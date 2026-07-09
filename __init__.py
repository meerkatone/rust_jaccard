#!/usr/bin/env python3

import os
import subprocess

import binaryninja as bn
from binaryninja import (
    BackgroundTaskThread,
    MessageBoxButtonSet,
    PluginCommand,
    log_error,
    log_info,
    show_message_box,
)
from binaryninja.interaction import get_directory_name_input, get_save_filename_input

# Plugin directory
plugin_dir = os.path.dirname(os.path.abspath(__file__))

# Check if the Rust binary exists
rust_binary_path = os.path.join(plugin_dir, "target", "release", "jaccard_analyzer")
if os.name == 'nt':  # Windows
    rust_binary_path += ".exe"

rust_available = os.path.exists(rust_binary_path)
if not rust_available:
    log_error(f"Rust binary not found at {rust_binary_path}. Please build with 'cargo build --release'")

def _show_message(title, message):
    bn.execute_on_main_thread(
        lambda title=title, message=message: show_message_box(
            title, message, MessageBoxButtonSet.OKButtonSet
        )
    )


class JaccardTask(BackgroundTaskThread):
    def __init__(self, command, success_message):
        super().__init__("Jaccard byte similarity analysis", False)
        self.command = command
        self.success_message = success_message

    def run(self):
        try:
            result = subprocess.run(
                self.command, capture_output=True, text=True, check=True
            )
            if result.stdout:
                log_info(f"Rust analyzer output: {result.stdout}")
            _show_message("Success", self.success_message)
        except subprocess.CalledProcessError as error:
            message = f"Analysis failed with exit code {error.returncode}"
            if error.stderr:
                message += f"\nError: {error.stderr}"
            log_error(message)
            _show_message("Error", message)
        except Exception as error:
            message = f"Analysis failed: {error}"
            log_error(message)
            _show_message("Error", message)


def _original_binary_path(bv):
    metadata = bv.file
    candidates = (
        getattr(metadata, "original_filename", ""),
        getattr(metadata, "filename", ""),
    )
    for path in candidates:
        if path and os.path.isfile(path) and not path.lower().endswith(".bndb"):
            return path
    return None


def analyze_folder_jaccard(bv):
    """Perform Jaccard similarity analysis against binaries in a folder"""
    if not rust_available:
        show_message_box("Error", "Rust binary not available. Please build the plugin first with 'cargo build --release'.", MessageBoxButtonSet.OKButtonSet)
        return

    # Get the current binary path
    current_path = _original_binary_path(bv)
    if not current_path:
        show_message_box(
            "Error",
            "The original binary is unavailable. Open the original executable or restore its path in the BNDB.",
            MessageBoxButtonSet.OKButtonSet,
        )
        return

    # Get folder to analyze
    folder_path = get_directory_name_input("Select folder with binaries", "Folder:")
    if not folder_path:
        return

    # Get output file path
    output_path = get_save_filename_input("Save results as", "parquet", "parquet")
    if not output_path:
        return

    command = [rust_binary_path, "-r", current_path, "-f", folder_path, "-o", output_path]
    JaccardTask(
        command, f"Analysis completed. Results saved to {output_path}"
    ).start()

def analyze_folder_pairwise_jaccard(bv):
    """Perform pairwise byte-chunk Jaccard analysis of binaries in a folder.

    Routes through the Rust `jaccard_analyzer` engine (byte-level Jaccard over
    real file content) so the menu command and the CLI share one implementation,
    rather than the prior separate pure-Python feature extraction.
    """
    if not rust_available:
        show_message_box("Error", "Rust binary not available. Please build the plugin first with 'cargo build --release'.", MessageBoxButtonSet.OKButtonSet)
        return

    # Get folder to analyze
    folder_path = get_directory_name_input("Select folder with binary files", "Folder:")
    if not folder_path:
        return

    # Get output file path
    output_path = get_save_filename_input("Save results as", "parquet", "parquet")
    if not output_path:
        return

    command = [rust_binary_path, "-p", "-f", folder_path, "-o", output_path]
    JaccardTask(
        command, f"Pairwise analysis completed. Results saved to {output_path}"
    ).start()

# Register the plugin commands
PluginCommand.register("Jaccard Similarity",
                      "Perform pairwise byte-chunk Jaccard similarity analysis of binaries in a folder",
                      analyze_folder_pairwise_jaccard)

PluginCommand.register("Jaccard Similarity (Reference)",
                      "Compare the current binary's raw bytes against binaries in a folder",
                      analyze_folder_jaccard)

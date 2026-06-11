#!/usr/bin/env python3

import os
import subprocess
from binaryninja import *
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

def analyze_folder_jaccard(bv):
    """Perform Jaccard similarity analysis against binaries in a folder"""
    if not rust_available:
        show_message_box("Error", "Rust binary not available. Please build the plugin first with 'cargo build --release'.", MessageBoxButtonSet.OKButtonSet)
        return

    # Get the current binary path
    current_path = bv.file.filename
    if not current_path:
        show_message_box("Error", "Please save the current binary first.", MessageBoxButtonSet.OKButtonSet)
        return

    # Get folder to analyze
    folder_path = get_directory_name_input("Select folder with binaries", "Folder:")
    if not folder_path:
        return

    # Get output file path
    output_path = get_save_filename_input("Save results as", "parquet", "parquet")
    if not output_path:
        return

    try:
        # Call the Rust binary
        cmd = [rust_binary_path, "-r", current_path, "-f", folder_path, "-o", output_path]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        
        show_message_box("Success", f"Analysis completed. Results saved to {output_path}", MessageBoxButtonSet.OKButtonSet)
        if result.stdout:
            log_info(f"Rust analyzer output: {result.stdout}")
            
    except subprocess.CalledProcessError as e:
        error_msg = f"Analysis failed with exit code {e.returncode}"
        if e.stderr:
            error_msg += f"\nError: {e.stderr}"
        show_message_box("Error", error_msg, MessageBoxButtonSet.OKButtonSet)
        log_error(error_msg)
    except Exception as e:
        show_message_box("Error", f"Analysis failed: {str(e)}", MessageBoxButtonSet.OKButtonSet)
        log_error(f"Plugin error: {str(e)}")

def analyze_folder_pairwise_jaccard(bv):
    """Perform pairwise Jaccard similarity analysis of all BNDB files in a folder.

    Routes through the Rust `jaccard_analyzer` engine (byte-level Jaccard over
    real file content) so the menu command and the CLI share one implementation,
    rather than the prior separate pure-Python feature extraction.
    """
    if not rust_available:
        show_message_box("Error", "Rust binary not available. Please build the plugin first with 'cargo build --release'.", MessageBoxButtonSet.OKButtonSet)
        return

    # Get folder to analyze
    folder_path = get_directory_name_input("Select folder with BNDB files", "Folder:")
    if not folder_path:
        return

    # Get output file path
    output_path = get_save_filename_input("Save results as", "parquet", "parquet")
    if not output_path:
        return

    try:
        # Call the Rust binary in pairwise mode.
        cmd = [rust_binary_path, "-p", "-f", folder_path, "-o", output_path]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)

        show_message_box("Success", f"Pairwise analysis completed. Results saved to {output_path}", MessageBoxButtonSet.OKButtonSet)
        if result.stdout:
            log_info(f"Rust analyzer output: {result.stdout}")

    except subprocess.CalledProcessError as e:
        error_msg = f"Pairwise analysis failed with exit code {e.returncode}"
        if e.stderr:
            error_msg += f"\nError: {e.stderr}"
        show_message_box("Error", error_msg, MessageBoxButtonSet.OKButtonSet)
        log_error(error_msg)
    except Exception as e:
        show_message_box("Error", f"Pairwise analysis failed: {str(e)}", MessageBoxButtonSet.OKButtonSet)
        log_error(f"Plugin error: {str(e)}")

# Register the plugin commands
PluginCommand.register("Jaccard Similarity",
                      "Perform pairwise Jaccard similarity analysis of all BNDB files in a folder",
                      analyze_folder_pairwise_jaccard)

PluginCommand.register("Jaccard Similarity (Reference)",
                      "Compare the current binary against a folder of binaries (Rust engine)",
                      analyze_folder_jaccard)
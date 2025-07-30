#!/usr/bin/env python3

import os
import sys
import subprocess
import hashlib
from binaryninja import *
from binaryninja.interaction import get_directory_name_input, get_save_filename_input
from binaryninja import load

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
    """Perform pairwise Jaccard similarity analysis of all BNDB files in a folder"""
    # Get folder to analyze
    folder_path = get_directory_name_input("Select folder with BNDB files", "Folder:")
    if not folder_path:
        return

    # Get output file path
    output_path = get_save_filename_input("Save results as", "parquet", "parquet")
    if not output_path:
        return

    try:
        from pathlib import Path
        
        folder = Path(folder_path)
        
        # Find BNDB files only
        bndb_files = list(folder.glob('*.bndb'))
        
        if len(bndb_files) < 2:
            show_message_box("Error", f"Need at least 2 BNDB files for pairwise analysis. Found {len(bndb_files)}", MessageBoxButtonSet.OKButtonSet)
            return
        
        log_info(f"Found {len(bndb_files)} BNDB files for pairwise analysis")
        
        # Load all binaries and extract features
        binary_features = []
        
        for bndb_path in bndb_files:
            try:
                # Load BNDB file in Binary Ninja
                target_bv = load(str(bndb_path))
                if target_bv is None:
                    log_warn(f"Could not load BNDB file: {bndb_path}")
                    continue
                
                # Wait for analysis to complete
                target_bv.update_analysis_and_wait()
                    
                features = extract_binja_features(target_bv)
                binary_features.append({
                    'name': bndb_path.name,
                    'path': str(bndb_path),
                    'features': features
                })
                
                target_bv.file.close()
                log_info(f"Loaded features for {bndb_path.name}: {len(features['instructions'])} instr, {len(features['functions'])} func, {len(features['basic_blocks'])} bb")
                
            except Exception as e:
                log_warn(f"Failed to analyze {bndb_path}: {e}")
                continue
        
        if len(binary_features) < 2:
            show_message_box("Error", f"Could only load {len(binary_features)} valid BNDB files", MessageBoxButtonSet.OKButtonSet)
            return
        
        # Calculate pairwise similarities
        results = []
        total_comparisons = 0
        identical_comparisons = 0
        
        log_info(f"Starting pairwise comparison of {len(binary_features)} binaries")
        
        for i in range(len(binary_features)):
            for j in range(len(binary_features)):
                binary1 = binary_features[i]
                binary2 = binary_features[j]
                
                is_identical = (i == j) or (binary1['path'] == binary2['path'])
                total_comparisons += 1
                
                similarity = calculate_jaccard_similarity(binary1['features'], binary2['features'])
                
                results.append({
                    'binary1': binary1['name'],
                    'binary2': binary2['name'],
                    'jaccard_index': similarity['overall_similarity'],
                    'instruction_similarity': similarity['instruction_similarity'],
                    'function_similarity': similarity['function_similarity'],
                    'basic_block_similarity': similarity['basic_block_similarity']
                })
                
                if is_identical:
                    identical_comparisons += 1
                    if similarity['overall_similarity'] < 1.0:
                        log_warn(f"ISSUE: Identical binary comparison {binary1['name']} vs {binary2['name']} got similarity {similarity['overall_similarity']:.4f} instead of 1.0")
                        log_warn(f"  Instruction similarity: {similarity['instruction_similarity']:.4f}")
                        log_warn(f"  Function similarity: {similarity['function_similarity']:.4f}")
                        log_warn(f"  Basic block similarity: {similarity['basic_block_similarity']:.4f}")
                    else:
                        log_info(f"✓ Identical binary comparison {binary1['name']} vs {binary2['name']}: {similarity['overall_similarity']:.4f}")
                else:
                    log_info(f"Compared {binary1['name']} vs {binary2['name']}: {similarity['overall_similarity']:.4f}")
        
        log_info(f"Completed {total_comparisons} total comparisons, {identical_comparisons} were identical binaries")
        
        # Export results to Parquet
        export_pairwise_results_to_parquet(results, output_path)
        show_message_box("Success", f"Pairwise analysis completed. Analyzed {len(results)} pairs. Results saved to {output_path}", MessageBoxButtonSet.OKButtonSet)
        
    except Exception as e:
        show_message_box("Error", f"Pairwise analysis failed: {str(e)}", MessageBoxButtonSet.OKButtonSet)
        log_error(f"Plugin error: {str(e)}")

def analyze_folder_jaccard_with_binja_features(bv):
    """Enhanced version using Binary Ninja's analysis features"""
    if not rust_available:
        show_message_box("Error", "Rust module not available. Please build the plugin first.", MessageBoxButtonSet.OKButtonSet)
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
        # Extract features from the current binary using Binary Ninja
        current_features = extract_binja_features(bv)
        
        # Process folder and compare
        from pathlib import Path
        import glob
        
        results = []
        folder = Path(folder_path)
        
        # Find BNDB files only
        binary_files = list(folder.glob('*.bndb'))
        
        # Analyze each BNDB file
        for binary_path in binary_files:
            try:
                # Load BNDB file in Binary Ninja
                target_bv = load(str(binary_path))
                if target_bv is None:
                    log_warn(f"Could not load BNDB file: {binary_path}")
                    continue
                
                # Wait for analysis to complete
                target_bv.update_analysis_and_wait()
                    
                target_features = extract_binja_features(target_bv)
                similarity = calculate_jaccard_similarity(current_features, target_features)
                
                results.append({
                    'name': binary_path.name,
                    'path': str(binary_path),
                    'similarity': similarity
                })
                
                target_bv.file.close()
                
            except Exception as e:
                log_warn(f"Failed to analyze {binary_path}: {e}")
                continue
        
        # Export results using the Rust parquet exporter
        export_results_to_parquet(results, output_path)
        show_message_box("Success", f"Analysis completed. Results saved to {output_path}", MessageBoxButtonSet.OKButtonSet)
        
    except Exception as e:
        show_message_box("Error", f"Analysis failed: {str(e)}", MessageBoxButtonSet.OKButtonSet)

def deterministic_hash(data):
    """Create a deterministic hash that's consistent across runs"""
    if isinstance(data, (tuple, list)):
        data = str(data)
    elif not isinstance(data, (str, bytes)):
        data = str(data)
    
    if isinstance(data, str):
        data = data.encode('utf-8')
    
    return int(hashlib.sha256(data).hexdigest()[:16], 16)

def extract_binja_features(bv):
    """Extract features from a Binary Ninja BinaryView"""
    features = {
        'instructions': set(),
        'functions': set(), 
        'basic_blocks': set()
    }
    
    if not bv.functions:
        log_warn("No functions found in binary - analysis may be incomplete")
        return features
    
    # Extract function features
    for func in bv.functions:
        # Hash function start address and size (end - start)
        func_size = func.highest_address - func.start
        func_hash = deterministic_hash((func.start, func_size))
        features['functions'].add(func_hash)
        
        # Extract basic block features
        for bb in func.basic_blocks:
            bb_hash = deterministic_hash((bb.start, bb.end))
            features['basic_blocks'].add(bb_hash)
            
            # Extract instruction features
            for instr in bb:
                # Hash the instruction mnemonic and operands
                instr_text = str(instr)
                if instr_text:
                    instr_hash = deterministic_hash(instr_text)
                    features['instructions'].add(instr_hash)
    
    log_info(f"Extracted {len(features['instructions'])} instructions, {len(features['functions'])} functions, {len(features['basic_blocks'])} basic blocks")
    return features

def calculate_jaccard_similarity(features1, features2):
    """Calculate Jaccard similarity between two feature sets"""
    similarities = {}
    
    for feature_type in ['instructions', 'functions', 'basic_blocks']:
        set1 = features1[feature_type]
        set2 = features2[feature_type]
        
        if len(set1) == 0 and len(set2) == 0:
            similarities[feature_type] = 1.0
        else:
            intersection = len(set1.intersection(set2))
            union = len(set1.union(set2))
            similarities[feature_type] = intersection / union if union > 0 else 0.0
    
    # Calculate weighted overall similarity
    overall = (similarities['instructions'] * 0.4 + 
              similarities['functions'] * 0.4 + 
              similarities['basic_blocks'] * 0.2)
    
    return {
        'instruction_similarity': similarities['instructions'],
        'function_similarity': similarities['functions'],
        'basic_block_similarity': similarities['basic_blocks'],
        'overall_similarity': overall
    }

def export_results_to_parquet(results, output_path):
    """Export results to Parquet format using pandas"""
    try:
        import pandas as pd
        
        # Convert results to DataFrame
        df_data = []
        for result in results:
            similarity = result['similarity']
            df_data.append({
                'binary_name': result['name'],
                'binary_path': result['path'],
                'instruction_similarity': similarity['instruction_similarity'],
                'function_similarity': similarity['function_similarity'],
                'basic_block_similarity': similarity['basic_block_similarity'],
                'overall_similarity': similarity['overall_similarity']
            })
        
        df = pd.DataFrame(df_data)
        df.to_parquet(output_path, index=False)
        
    except ImportError:
        # Fallback to CSV if pandas not available
        import csv
        csv_path = output_path.replace('.parquet', '.csv')
        
        with open(csv_path, 'w', newline='') as csvfile:
            fieldnames = ['binary_name', 'binary_path', 'instruction_similarity', 
                         'function_similarity', 'basic_block_similarity', 'overall_similarity']
            writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
            writer.writeheader()
            
            for result in results:
                similarity = result['similarity']
                writer.writerow({
                    'binary_name': result['name'],
                    'binary_path': result['path'],
                    'instruction_similarity': similarity['instruction_similarity'],
                    'function_similarity': similarity['function_similarity'],
                    'basic_block_similarity': similarity['basic_block_similarity'],
                    'overall_similarity': similarity['overall_similarity']
                })
        
        show_message_box("Info", f"Pandas not available. Results saved as CSV: {csv_path}", 
                        MessageBoxButtonSet.OKButtonSet)

def export_pairwise_results_to_parquet(results, output_path):
    """Export pairwise results to Parquet format using pandas"""
    try:
        import pandas as pd
        
        # Convert results to DataFrame
        df = pd.DataFrame(results)
        df.to_parquet(output_path, index=False)
        
    except ImportError:
        # Fallback to CSV if pandas not available
        import csv
        csv_path = output_path.replace('.parquet', '.csv')
        
        with open(csv_path, 'w', newline='') as csvfile:
            if results:
                fieldnames = ['binary1', 'binary2', 'jaccard_index', 'instruction_similarity', 'function_similarity', 'basic_block_similarity']
                writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
                writer.writeheader()
                writer.writerows(results)
        
        show_message_box("Info", f"Pandas not available. Results saved as CSV: {csv_path}", 
                        MessageBoxButtonSet.OKButtonSet)

# Register the plugin command
PluginCommand.register("Jaccard Similarity", 
                      "Perform pairwise Jaccard similarity analysis of all BNDB files in a folder", 
                      analyze_folder_pairwise_jaccard)
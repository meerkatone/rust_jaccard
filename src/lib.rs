// Rust Jaccard Similarity Library for Binary Analysis
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;
use anyhow::{Result, Context};
use log::info;
use rayon::prelude::*;

mod jaccard;
mod parquet_export;

pub use jaccard::{JaccardAnalyzer, JaccardSimilarity};
pub use parquet_export::ParquetExporter;

#[derive(Debug, Clone)]
pub struct BinaryFeatures {
    pub name: String,
    pub path: String,
    pub instruction_hashes: HashSet<u64>,
    pub function_hashes: HashSet<u64>,
    pub basic_block_hashes: HashSet<u64>,
}

impl BinaryFeatures {
    pub fn extract_from_bytes(bytes: &[u8], name: String, path: String) -> Result<Self> {
        let mut instruction_hashes = HashSet::new();
        let mut function_hashes = HashSet::new();
        let mut basic_block_hashes = HashSet::new();

        // Simple feature extraction from raw bytes
        // This is a simplified approach - in practice you'd want to use a disassembler
        for chunk in bytes.chunks(4) {
            let hash = Self::hash_bytes(chunk);
            instruction_hashes.insert(hash);
        }

        // Extract function-like patterns (simplified)
        for chunk in bytes.chunks(16) {
            let hash = Self::hash_bytes(chunk);
            function_hashes.insert(hash);
        }

        // Extract basic block-like patterns (simplified)
        for chunk in bytes.chunks(8) {
            let hash = Self::hash_bytes(chunk);
            basic_block_hashes.insert(hash);
        }

        Ok(BinaryFeatures {
            name,
            path,
            instruction_hashes,
            function_hashes,
            basic_block_hashes,
        })
    }

    fn hash_bytes(bytes: &[u8]) -> u64 {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        let mut hash_bytes = [0u8; 8];
        hash_bytes.copy_from_slice(&result[..8]);
        u64::from_le_bytes(hash_bytes)
    }
}

pub fn analyze_folder_jaccard(reference_path: &str, folder_path: &str, output_path: &str) -> Result<()> {
    run_jaccard_analysis(reference_path, folder_path, output_path)
}

pub fn analyze_folder_pairwise_jaccard(folder_path: &str, output_path: &str) -> Result<()> {
    run_pairwise_jaccard_analysis(folder_path, output_path)
}

fn run_jaccard_analysis(reference_path: &str, folder_path: &str, output_path: &str) -> Result<()> {
    info!("Starting Jaccard similarity analysis");
    
    // Load reference binary
    let reference_bytes = std::fs::read(reference_path)
        .context("Failed to read reference binary")?;
    
    let reference_name = Path::new(reference_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    let current_binary = BinaryFeatures::extract_from_bytes(
        &reference_bytes,
        reference_name,
        reference_path.to_string(),
    )?;

    // Find all binary files in the folder
    let binary_paths: Vec<_> = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_binary_file(e.path()))
        .collect();

    let analyzer = JaccardAnalyzer::new();
    let exporter = ParquetExporter::new();

    // Process binaries in parallel
    let results: Vec<_> = binary_paths
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            
            match load_and_analyze_binary(path, name.clone()) {
                Ok(features) => {
                    let similarity = analyzer.calculate_similarity(&current_binary, &features);
                    Some((name, path.to_string_lossy().to_string(), similarity))
                }
                Err(e) => {
                    info!("Failed to analyze {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    exporter.export_results(&results, output_path)?;
    info!("Results exported to {}", output_path);
    
    Ok(())
}

fn load_and_analyze_binary(path: &Path, name: String) -> Result<BinaryFeatures> {
    // For BNDB files, we can't directly parse them in Rust
    // This is a placeholder - in practice, this would need to be handled by the Python side
    // For now, let's use a simplified approach based on file metadata
    let metadata = std::fs::metadata(path).context("Failed to read file metadata")?;
    let file_size = metadata.len();
    
    // Create dummy features based on file characteristics
    let mut instruction_hashes = HashSet::new();
    let mut function_hashes = HashSet::new();
    let mut basic_block_hashes = HashSet::new();
    
    // Generate some deterministic hashes based on file properties
    let name_hash = BinaryFeatures::hash_bytes(name.as_bytes());
    let size_hash = BinaryFeatures::hash_bytes(&file_size.to_le_bytes());
    let path_hash = BinaryFeatures::hash_bytes(path.to_string_lossy().as_bytes());
    
    instruction_hashes.insert(name_hash);
    instruction_hashes.insert(size_hash);
    
    function_hashes.insert(path_hash);
    function_hashes.insert(name_hash ^ size_hash);
    
    basic_block_hashes.insert(size_hash ^ path_hash);
    basic_block_hashes.insert(name_hash ^ path_hash);
    
    Ok(BinaryFeatures {
        name,
        path: path.to_string_lossy().to_string(),
        instruction_hashes,
        function_hashes,
        basic_block_hashes,
    })
}

fn is_binary_file(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => ext.to_lowercase() == "bndb",
        None => false,
    }
}

fn run_pairwise_jaccard_analysis(folder_path: &str, output_path: &str) -> Result<()> {
    info!("Starting pairwise Jaccard similarity analysis");
    
    // Find all binary files in the folder
    let binary_paths: Vec<_> = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_binary_file(e.path()))
        .collect();

    info!("Found {} binary files", binary_paths.len());

    // Load all binaries into memory
    let binaries: Vec<BinaryFeatures> = binary_paths
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().to_string();
            
            match load_and_analyze_binary(path, name) {
                Ok(features) => Some(features),
                Err(e) => {
                    info!("Failed to analyze {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();

    info!("Successfully loaded {} binaries", binaries.len());

    let analyzer = JaccardAnalyzer::new();
    let exporter = ParquetExporter::new();

    // Calculate pairwise similarities
    let mut results = Vec::new();
    
    for (i, binary1) in binaries.iter().enumerate() {
        for (j, binary2) in binaries.iter().enumerate() {
            // Skip self-comparison and duplicate pairs (only calculate upper triangle)
            if i >= j {
                continue;
            }
            
            let similarity = analyzer.calculate_similarity(binary1, binary2);
            
            // Create a better pair identifier
            let pair_name = format!("{}|{}", binary1.name, binary2.name);
            let pair_path = format!("{} <-> {}", binary1.path, binary2.path);
            
            info!("Compared {} vs {}: overall similarity = {:.4}", 
                  binary1.name, binary2.name, similarity.overall_similarity);
            
            results.push((pair_name, pair_path, similarity));
        }
    }

    info!("Calculated {} pairwise similarities", results.len());
    
    if results.is_empty() {
        info!("Warning: No similarity results to export");
        return Ok(());
    }
    
    exporter.export_results(&results, output_path)?;
    info!("Results exported to {}", output_path);
    
    Ok(())
}

// Library exports for use by Binary Ninja plugin
// Rust Jaccard Similarity Library for Binary Analysis
use anyhow::{bail, Context, Result};
use log::info;
use rayon::prelude::*;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

mod jaccard;
mod parquet_export;

pub use jaccard::{JaccardAnalyzer, JaccardSimilarity};
pub use parquet_export::{ComparisonResult, ParquetExporter};

pub const DEFAULT_MAX_PAIRWISE_FILES: usize = 256;
pub const DEFAULT_MAX_PAIRWISE_INPUT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct BinaryFeatures {
    pub name: String,
    pub path: String,
    pub chunk_4_hashes: HashSet<u64>,
    pub chunk_16_hashes: HashSet<u64>,
    pub chunk_8_hashes: HashSet<u64>,
}

impl BinaryFeatures {
    pub fn extract_from_bytes(bytes: &[u8], name: String, path: String) -> Result<Self> {
        let mut chunk_4_hashes = HashSet::new();
        let mut chunk_16_hashes = HashSet::new();
        let mut chunk_8_hashes = HashSet::new();

        // These are deliberately raw-byte chunk features, not disassembled code.
        for chunk in bytes.chunks(4) {
            let hash = Self::hash_bytes(chunk);
            chunk_4_hashes.insert(hash);
        }

        // Coarser chunk sizes capture longer identical byte runs.
        for chunk in bytes.chunks(16) {
            let hash = Self::hash_bytes(chunk);
            chunk_16_hashes.insert(hash);
        }

        for chunk in bytes.chunks(8) {
            let hash = Self::hash_bytes(chunk);
            chunk_8_hashes.insert(hash);
        }

        Ok(BinaryFeatures {
            name,
            path,
            chunk_4_hashes,
            chunk_16_hashes,
            chunk_8_hashes,
        })
    }

    fn hash_bytes(bytes: &[u8]) -> u64 {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        let mut hash_bytes = [0u8; 8];
        hash_bytes.copy_from_slice(&result[..8]);
        u64::from_le_bytes(hash_bytes)
    }
}

pub fn analyze_folder_jaccard(
    reference_path: &str,
    folder_path: &str,
    output_path: &str,
) -> Result<()> {
    run_jaccard_analysis(reference_path, folder_path, output_path)
}

pub fn analyze_folder_pairwise_jaccard(folder_path: &str, output_path: &str) -> Result<()> {
    analyze_folder_pairwise_jaccard_with_limits(
        folder_path,
        output_path,
        DEFAULT_MAX_PAIRWISE_FILES,
        DEFAULT_MAX_PAIRWISE_INPUT_BYTES,
    )
}

pub fn analyze_folder_pairwise_jaccard_with_limits(
    folder_path: &str,
    output_path: &str,
    max_files: usize,
    max_total_bytes: u64,
) -> Result<()> {
    run_pairwise_jaccard_analysis(folder_path, output_path, max_files, max_total_bytes)
}

fn run_jaccard_analysis(reference_path: &str, folder_path: &str, output_path: &str) -> Result<()> {
    info!("Starting Jaccard similarity analysis");

    // Load reference binary
    let reference_bytes =
        std::fs::read(reference_path).context("Failed to read reference binary")?;

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
                    Some(ComparisonResult {
                        binary1: current_binary.name.clone(),
                        binary2: name,
                        pair_path: format!("{} <-> {}", current_binary.path, path.display()),
                        similarity,
                    })
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
    // This computes byte-level similarity over original binary files using the
    // same chunk-and-hash featurization on both sides. Database files are
    // excluded during discovery because their SQLite contents describe Binary
    // Ninja state rather than the program bytes being compared.
    let bytes = std::fs::read(path).context("Failed to read binary file")?;
    BinaryFeatures::extract_from_bytes(&bytes, name, path.to_string_lossy().to_string())
}

fn is_binary_file(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "bndb" {
        return false;
    }
    if matches!(
        extension.as_str(),
        "exe" | "dll" | "sys" | "bin" | "so" | "dylib" | "o" | "elf"
    ) {
        return true;
    }

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 4];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    header.starts_with(b"MZ")
        || header.starts_with(b"\x7fELF")
        || matches!(
            header,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
        )
}

fn validate_pairwise_limits(
    file_count: usize,
    total_bytes: u64,
    max_files: usize,
    max_total_bytes: u64,
) -> Result<()> {
    if max_files < 2 {
        bail!("max pairwise files must be at least 2");
    }
    if max_total_bytes == 0 {
        bail!("max pairwise input bytes must be greater than zero");
    }
    if file_count > max_files {
        let pair_count = file_count.saturating_mul(file_count.saturating_sub(1)) / 2;
        bail!(
            "pairwise input contains {file_count} binaries ({pair_count} comparisons); \
             the configured limit is {max_files}. Narrow the folder or raise \
             --max-pairwise-files explicitly"
        );
    }
    if total_bytes > max_total_bytes {
        bail!(
            "pairwise input totals {total_bytes} bytes; the configured limit is \
             {max_total_bytes} bytes. Narrow the folder or raise \
             --max-pairwise-input-mib explicitly"
        );
    }
    Ok(())
}

fn run_pairwise_jaccard_analysis(
    folder_path: &str,
    output_path: &str,
    max_files: usize,
    max_total_bytes: u64,
) -> Result<()> {
    info!("Starting pairwise Jaccard similarity analysis");

    // Find all binary files in the folder
    let binary_paths: Vec<_> = WalkDir::new(folder_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_binary_file(e.path()))
        .collect();

    info!("Found {} binary files", binary_paths.len());

    let total_bytes = binary_paths.iter().try_fold(0u64, |total, entry| {
        let length = entry
            .metadata()
            .with_context(|| format!("Failed to read metadata for {}", entry.path().display()))?
            .len();
        Ok::<u64, anyhow::Error>(total.saturating_add(length))
    })?;
    validate_pairwise_limits(binary_paths.len(), total_bytes, max_files, max_total_bytes)?;

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

            let pair_path = format!("{} <-> {}", binary1.path, binary2.path);

            info!(
                "Compared {} vs {}: overall similarity = {:.4}",
                binary1.name, binary2.name, similarity.overall_similarity
            );

            results.push(ComparisonResult {
                binary1: binary1.name.clone(),
                binary2: binary2.name.clone(),
                pair_path,
                similarity,
            });
        }
    }

    info!("Calculated {} pairwise similarities", results.len());

    if results.is_empty() {
        info!("No similarity pairs found; writing an empty Parquet result");
    }

    exporter.export_results(&results, output_path)?;
    info!("Results exported to {}", output_path);

    Ok(())
}

#[cfg(test)]
mod pairwise_limit_tests {
    use super::validate_pairwise_limits;

    #[test]
    fn accepts_limits_at_the_boundary() {
        assert!(validate_pairwise_limits(256, 512, 256, 512).is_ok());
    }

    #[test]
    fn rejects_excessive_pair_count_before_loading_files() {
        let error = validate_pairwise_limits(257, 1, 256, 512).unwrap_err();
        assert!(error.to_string().contains("32896 comparisons"));
    }

    #[test]
    fn rejects_excessive_total_input_size() {
        let error = validate_pairwise_limits(2, 513, 256, 512).unwrap_err();
        assert!(error.to_string().contains("totals 513 bytes"));
    }

    #[test]
    fn rejects_invalid_configured_limits() {
        assert!(validate_pairwise_limits(0, 0, 1, 512).is_err());
        assert!(validate_pairwise_limits(0, 0, 2, 0).is_err());
    }
}

// Library exports for use by Binary Ninja plugin

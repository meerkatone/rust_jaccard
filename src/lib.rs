mod atomic_output;
// Rust Jaccard Similarity Library for Binary Analysis
use anyhow::{bail, Context, Result};
use log::{debug, info};
use rayon::prelude::*;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

mod jaccard;
mod parquet_export;

pub use jaccard::{JaccardAnalyzer, JaccardSimilarity};
pub use parquet_export::{ComparisonResult, ParquetExporter};

pub const DEFAULT_MAX_PAIRWISE_FILES: usize = 256;
pub const DEFAULT_MAX_PAIRWISE_INPUT_BYTES: u64 = 512 * 1024 * 1024;

/// The distinct fixed-width chunks of one file.
///
/// Full chunks are packed into integers and kept sorted and deduplicated, so
/// two sets intersect with one linear merge and no hashing. Only the last
/// chunk of a file can be short; it is kept apart because its bytes cannot
/// equal a full chunk, which makes the set exact without a length tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChunkSet<T> {
    full: Vec<T>,
    tail: Option<Vec<u8>>,
}

impl<T: Copy + Ord + Send> ChunkSet<T> {
    fn from_bytes<const N: usize>(bytes: &[u8], pack: impl Fn([u8; N]) -> T) -> Self {
        let (chunks, tail) = bytes.as_chunks::<N>();
        let mut full: Vec<T> = chunks.iter().map(|chunk| pack(*chunk)).collect();
        full.par_sort_unstable();
        full.dedup();
        full.shrink_to_fit();
        Self {
            full,
            tail: (!tail.is_empty()).then(|| tail.to_vec()),
        }
    }

    /// Build a set from already packed full chunks.
    pub fn from_values(values: impl IntoIterator<Item = T>) -> Self {
        let mut full: Vec<T> = values.into_iter().collect();
        full.sort_unstable();
        full.dedup();
        Self { full, tail: None }
    }

    /// Number of distinct chunks.
    pub fn len(&self) -> usize {
        self.full.len() + usize::from(self.tail.is_some())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of chunks present in both sets.
    pub fn intersection_len(&self, other: &Self) -> usize {
        let (a, b) = (self.full.as_slice(), other.full.as_slice());
        let (mut i, mut j, mut shared) = (0, 0, 0);
        while i < a.len() && j < b.len() {
            let (x, y) = (a[i], b[j]);
            shared += usize::from(x == y);
            i += usize::from(x <= y);
            j += usize::from(y <= x);
        }
        shared + usize::from(self.tail.is_some() && self.tail == other.tail)
    }
}

#[derive(Debug, Clone)]
pub struct BinaryFeatures {
    pub name: String,
    pub path: String,
    pub chunk_4: ChunkSet<u32>,
    pub chunk_8: ChunkSet<u64>,
    pub chunk_16: ChunkSet<u128>,
}

impl BinaryFeatures {
    pub fn extract_from_bytes(bytes: &[u8], name: String, path: String) -> Result<Self> {
        // These are deliberately raw-byte chunk features, not disassembled
        // code. Coarser chunk sizes capture longer identical byte runs.
        Ok(BinaryFeatures {
            name,
            path,
            chunk_4: ChunkSet::from_bytes(bytes, u32::from_le_bytes),
            chunk_8: ChunkSet::from_bytes(bytes, u64::from_le_bytes),
            chunk_16: ChunkSet::from_bytes(bytes, u128::from_le_bytes),
        })
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

    reject_output_alias(output_path, Path::new(reference_path))?;

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

    let binary_paths = discover_binary_paths(folder_path)?;
    for entry in &binary_paths {
        reject_output_alias(output_path, entry.path())?;
    }
    let reference_path =
        std::fs::canonicalize(reference_path).context("Failed to resolve reference path")?;
    let reference_path = reference_path.to_string_lossy().into_owned();

    let analyzer = JaccardAnalyzer::new();
    let exporter = ParquetExporter::new();

    // Process binaries in parallel
    let results: Vec<_> = binary_paths
        .par_iter()
        .map(|entry| {
            let path = entry.path();
            let name = path
                .file_name()
                .context("Binary has no filename")?
                .to_string_lossy()
                .into_owned();
            let features = load_and_analyze_binary(path, name.clone())?;
            let similarity = analyzer.calculate_similarity(&current_binary, &features);
            Ok(ComparisonResult {
                binary1: current_binary.name.clone(),
                binary2: name,
                binary1_path: reference_path.clone(),
                binary2_path: features.path.clone(),
                pair_path: format!("{} <-> {}", reference_path, path.display()),
                similarity,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    exporter.export_results(&results, output_path)?;
    info!("Results exported to {}", output_path);

    Ok(())
}

/// Refuse to replace a binary with its own similarity report, including symlink aliases.
fn reject_output_alias(output_path: &str, input: &Path) -> Result<()> {
    if let Ok(output) = std::fs::canonicalize(output_path) {
        if output == std::fs::canonicalize(input)? {
            bail!(
                "Output report would overwrite input binary: {}",
                input.display()
            );
        }
    }
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

fn discover_binary_paths(folder_path: &str) -> Result<Vec<walkdir::DirEntry>> {
    let folder = std::fs::canonicalize(folder_path).context("Failed to resolve input directory")?;
    if !folder.is_dir() {
        bail!("Input is not a directory: {}", folder.display());
    }
    let mut paths = Vec::new();
    for entry in WalkDir::new(&folder) {
        let entry = entry.with_context(|| format!("Failed to walk {}", folder.display()))?;
        if entry.file_type().is_file() && is_binary_file(entry.path())? {
            paths.push(entry);
        }
    }
    paths.sort_by(|a, b| a.path().cmp(b.path()));
    Ok(paths)
}

fn is_binary_file(path: &Path) -> Result<bool> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "bndb" {
        return Ok(false);
    }
    if matches!(
        extension.as_str(),
        "exe" | "dll" | "sys" | "bin" | "so" | "dylib" | "o" | "elf"
    ) {
        return Ok(true);
    }

    let mut file =
        std::fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut header = [0u8; 4];
    if let Err(error) = file.read_exact(&mut header) {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            return Ok(false);
        }
        return Err(error).with_context(|| format!("Failed to read {}", path.display()));
    }
    Ok(header.starts_with(b"MZ")
        || header.starts_with(b"\x7fELF")
        || matches!(
            header,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xca, 0xfe, 0xba, 0xbf]
        ))
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

    let binary_paths = discover_binary_paths(folder_path)?;
    for entry in &binary_paths {
        reject_output_alias(output_path, entry.path())?;
    }

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
        .map(|entry| {
            let path = entry.path();
            let name = path
                .file_name()
                .context("Binary has no filename")?
                .to_string_lossy()
                .into_owned();
            load_and_analyze_binary(path, name)
        })
        .collect::<Result<Vec<_>>>()?;

    info!("Successfully loaded {} binaries", binaries.len());

    let analyzer = JaccardAnalyzer::new();
    let exporter = ParquetExporter::new();

    // Compare each unordered pair once (the upper triangle). Pairs are
    // independent, so they run in parallel; collect keeps them in order.
    let results: Vec<ComparisonResult> = (0..binaries.len())
        .into_par_iter()
        .flat_map_iter(|first| {
            let binary1 = &binaries[first];
            let analyzer = &analyzer;
            binaries[first + 1..].iter().map(move |binary2| {
                let similarity = analyzer.calculate_similarity(binary1, binary2);
                debug!(
                    "Compared {} vs {}: overall similarity = {:.4}",
                    binary1.name, binary2.name, similarity.overall_similarity
                );
                ComparisonResult {
                    binary1: binary1.name.clone(),
                    binary2: binary2.name.clone(),
                    binary1_path: binary1.path.clone(),
                    binary2_path: binary2.path.clone(),
                    pair_path: format!("{} <-> {}", binary1.path, binary2.path),
                    similarity,
                }
            })
        })
        .collect();

    info!("Calculated {} pairwise similarities", results.len());

    if results.is_empty() {
        info!("No similarity pairs found; writing an empty Parquet result");
    }

    exporter.export_results(&results, output_path)?;
    info!("Results exported to {}", output_path);

    Ok(())
}

#[cfg(test)]
mod feature_tests {
    use super::{BinaryFeatures, ChunkSet, JaccardAnalyzer};
    use std::collections::HashSet;

    fn raw_jaccard(a: &[u8], b: &[u8], width: usize) -> f64 {
        let a: HashSet<_> = a.chunks(width).collect();
        let b: HashSet<_> = b.chunks(width).collect();
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        a.intersection(&b).count() as f64 / a.union(&b).count() as f64
    }

    #[test]
    fn packed_features_match_raw_chunk_sets_including_partial_tails() {
        let inputs: Vec<Vec<u8>> = (0..=64)
            .flat_map(|length| {
                [
                    vec![0; length],
                    (0..length).map(|i| (i % 7) as u8).collect(),
                ]
            })
            .collect();
        for a in &inputs {
            let features_a = BinaryFeatures::extract_from_bytes(a, "a".into(), "a".into()).unwrap();
            for b in &inputs {
                let features_b =
                    BinaryFeatures::extract_from_bytes(b, "b".into(), "b".into()).unwrap();
                let result = JaccardAnalyzer::new().calculate_similarity(&features_a, &features_b);
                assert_eq!(result.chunk_4_similarity, raw_jaccard(a, b, 4));
                assert_eq!(result.chunk_8_similarity, raw_jaccard(a, b, 8));
                assert_eq!(result.chunk_16_similarity, raw_jaccard(a, b, 16));
            }
        }
    }

    #[test]
    fn short_tail_never_equals_a_full_chunk_with_the_same_leading_bytes() {
        // "AB" as a two-byte tail, and as a full chunk padded with zeros.
        let tail = BinaryFeatures::extract_from_bytes(b"AB", "t".into(), "t".into()).unwrap();
        let full = BinaryFeatures::extract_from_bytes(b"AB\0\0", "f".into(), "f".into()).unwrap();
        assert_eq!(tail.chunk_4.len(), 1);
        assert_eq!(full.chunk_4.len(), 1);
        assert_eq!(tail.chunk_4.intersection_len(&full.chunk_4), 0);
        assert_eq!(tail.chunk_4.intersection_len(&tail.chunk_4), 1);
    }

    #[test]
    fn chunk_sets_deduplicate_and_merge_in_either_order() {
        let a = ChunkSet::from_values([5u64, 1, 5, 3, 9]);
        let b = ChunkSet::from_values([9u64, 2, 3, 3]);
        assert_eq!((a.len(), b.len()), (4, 3));
        assert_eq!(a.intersection_len(&b), 2);
        assert_eq!(b.intersection_len(&a), 2);
        assert!(ChunkSet::<u64>::default().is_empty());
        assert_eq!(a.intersection_len(&ChunkSet::default()), 0);
    }
}

#[cfg(test)]
mod pairwise_limit_tests {
    use super::{discover_binary_paths, is_binary_file, validate_pairwise_limits};

    #[test]
    fn reports_cannot_overwrite_reference_or_pairwise_inputs() {
        let directory = tempfile::tempdir().unwrap();
        let a = directory.path().join("a.bin");
        let b = directory.path().join("b.bin");
        std::fs::write(&a, b"binary a").unwrap();
        std::fs::write(&b, b"binary b").unwrap();
        let folder = directory.path().to_str().unwrap();
        let output = a.to_str().unwrap();
        assert!(super::analyze_folder_jaccard(output, folder, output)
            .unwrap_err()
            .to_string()
            .contains("overwrite input"));
        assert!(super::analyze_folder_pairwise_jaccard(folder, output)
            .unwrap_err()
            .to_string()
            .contains("overwrite input"));
        assert_eq!(std::fs::read(&a).unwrap(), b"binary a");
        assert_eq!(std::fs::read(&b).unwrap(), b"binary b");
    }

    #[cfg(unix)]
    #[test]
    fn report_destination_symlink_cannot_alias_an_input() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("a.bin");
        let output = directory.path().join("report.parquet");
        std::fs::write(&input, b"binary").unwrap();
        std::os::unix::fs::symlink(&input, &output).unwrap();
        assert!(super::reject_output_alias(output.to_str().unwrap(), &input).is_err());
    }

    #[test]
    fn detects_universal_binaries_by_magic() {
        let directory = tempfile::tempdir().unwrap();
        for (name, magic) in [
            ("fat32", [0xca, 0xfe, 0xba, 0xbe]),
            ("fat64", [0xca, 0xfe, 0xba, 0xbf]),
        ] {
            let file = directory.path().join(name);
            std::fs::write(&file, magic).unwrap();
            assert!(is_binary_file(&file).unwrap(), "{name}");
        }
        let text = directory.path().join("notes");
        std::fs::write(&text, b"text").unwrap();
        assert!(!is_binary_file(&text).unwrap());
    }

    #[test]
    fn rejects_missing_and_non_directory_inputs() {
        let directory = tempfile::tempdir().unwrap();
        assert!(discover_binary_paths(directory.path().join("missing").to_str().unwrap()).is_err());
        let file = directory.path().join("file.bin");
        std::fs::write(&file, b"data").unwrap();
        assert!(discover_binary_paths(file.to_str().unwrap()).is_err());
    }

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

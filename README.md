# Rust Jaccard Similarity Analyzer

A Binary Ninja plugin for performing Jaccard similarity analysis on binary files with high-performance Rust backend and Parquet export capabilities.

## Overview

This plugin computes **byte-level** Jaccard similarity between binary files and exports the results to Parquet. It combines Python for Binary Ninja integration with Rust for efficient computation and data export.

> **Note on what "similarity" means here.** The analyzer does **not** disassemble the input or compare instructions/basic blocks/functions semantically. It hashes raw-byte windows of three sizes (see *Feature Extraction* below) and computes Jaccard over those hash sets. This makes it a fast structural/byte-overlap signal, not a code-aware diff. For true instruction/basic-block/function similarity the `.bndb` would need to be routed through Binary Ninja on the Python side (future work). Both sides of every comparison use the identical byte-window featurization, so scores are content-driven and comparable.

## Features

- **Binary Similarity Analysis**: Compare binaries using Jaccard similarity coefficients
- **Multiple Analysis Modes**:
  - Reference-based: Compare one binary against a folder of binaries
  - Pairwise: Compare all binaries in a folder against each other
- **Feature Extraction**: Byte-window hashing at three window sizes (not disassembly-based)
- **High Performance**: Rust backend with parallel processing using Rayon
- **Data Export**: Results exported to Parquet format for data analysis workflows
- **Binary Ninja Integration**: Native UI integration with file dialogs and progress feedback

## Installation

1. **Prerequisites**:
   - Binary Ninja (minimum version 3.0.0)
   - Rust toolchain (for building the analyzer)

2. **Build the Rust Components**:
   ```bash
   cd /path/to/plugin/directory
   cargo build --locked --release
   ```

   If Binary Ninja reports that the plugin was built for an outdated core ABI, update the lockfile deliberately and rebuild:

   ```bash
   cargo update
   cargo build --locked --release
   ```

3. **Install Plugin**:
   - Copy the plugin directory to your Binary Ninja plugins folder
   - The plugin will automatically register when Binary Ninja starts

## Usage

### From Binary Ninja UI

The plugin registers two commands (Plugins menu), both of which run through the
bundled Rust `jaccard_analyzer` engine and prompt for a Parquet output location:

- **Jaccard Similarity** — pairwise: select a folder of executable or library files; every pair is compared. `.bndb` database files are deliberately excluded.
- **Jaccard Similarity (Reference)** — compare the current view's original binary file against a folder of binaries.

### Command Line Interface

The plugin includes a standalone CLI tool:

```bash
# Pairwise analysis of all binaries in a folder
./target/release/jaccard_analyzer -f /path/to/binaries -o results.parquet -p

# Pairwise mode defaults to 256 files and 512 MiB total input. Raise either
# limit explicitly only when the machine has enough memory for all features.
./target/release/jaccard_analyzer -f /path/to/binaries -o results.parquet -p \
  --max-pairwise-files 512 --max-pairwise-input-mib 1024

# Compare reference binary against folder
./target/release/jaccard_analyzer -r reference.bin -f /path/to/binaries -o results.parquet
```

## Algorithm Details

### Feature Extraction

The analyzer reads each file's raw bytes and hashes aligned byte chunks of three sizes into three hash sets. They are **byte chunks, not disassembled code**:

1. **4-byte chunks**: SHA-256 hashes of aligned 4-byte chunks
2. **16-byte chunks**: SHA-256 hashes of aligned 16-byte chunks
3. **8-byte chunks**: SHA-256 hashes of aligned 8-byte chunks

### Similarity Calculation

Jaccard similarity is calculated for each byte-window set:

```
J(A,B) = |A ∩ B| / |A ∪ B|
```

Overall similarity uses a weighted combination of the three sets:
- 4-byte chunks: 40%
- 16-byte chunks: 40%
- 8-byte chunks: 20%

### Output Format

Results are exported in Parquet format with the following schema:

| Column | Type | Description |
|--------|------|-------------|
| binary1 | string | First binary name |
| binary2 | string | Second binary name |
| binary_pair | string | Pair identifier |
| jaccard_index | float64 | Overall similarity score |
| chunk_4_similarity | float64 | Similarity of aligned 4-byte chunk sets |
| chunk_16_similarity | float64 | Similarity of aligned 16-byte chunk sets |
| chunk_8_similarity | float64 | Similarity of aligned 8-byte chunk sets |

## Development

### Building

```bash
# Build release version
cargo build --locked --release

# Build debug version
cargo build --locked
```

### Testing

```bash
# Run all tests
cargo test --locked

# Run with verbose output
cargo test --locked -- --nocapture
```

### Linting

```bash
# Check code style
cargo clippy

# Format code
cargo fmt
```

### Dependencies

**Rust Dependencies**:
- `arrow` & `parquet`: Data export functionality
- `rayon`: Parallel processing
- `walkdir`: Directory traversal
- `sha2`: Cryptographic hashing
- `clap`: CLI argument parsing
- `serde`: Serialization

**Python Dependencies**:
- Binary Ninja API
- `pandas` (optional, for Parquet export fallback)

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Binary Ninja  │    │   Python Plugin  │    │  Rust Analyzer  │
│      UI         │◄──►│    (__init__.py) │◄──►│   (lib.rs)      │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                              │                          │
                              ▼                          ▼
                       ┌──────────────┐         ┌─────────────────┐
                       │   Feature    │         │   Similarity    │
                       │  Extraction  │         │  Calculation    │
                       └──────────────┘         └─────────────────┘
                              │                          │
                              └────────┬─────────────────┘
                                       │
                                       ▼
                               ┌──────────────┐
                               │   Parquet    │
                               │   Export     │
                               └──────────────┘
```

## Performance

- **Parallel Processing**: Utilizes all CPU cores for analysis
- **Bounded pairwise mode**: Rejects more than 256 files or 512 MiB of input by default because pairwise features are held in memory; CLI flags can raise either limit explicitly
- **Compressed output**: Results use Parquet compression; the reduction depends
  on the input corpus and comparison count.

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes and add tests
4. Run `cargo test --locked` and `cargo clippy --locked`
5. Submit a pull request

## Troubleshooting

### Common Issues

**"Rust binary not found" error**:
- Ensure you've run `cargo build --locked --release`
- Check that the binary exists at `target/release/jaccard_analyzer`

**Plugin not appearing in Binary Ninja**:
- Verify plugin is in the correct plugins directory
- Check Binary Ninja console for error messages
- Ensure Binary Ninja version is 3.0.0 or higher

**Analysis fails on large datasets**:
- Consider reducing dataset size or increasing system memory
- Check disk space for output files
- Monitor system resources during analysis

### Logging

Enable debug logging:
```bash
RUST_LOG=debug ./target/release/jaccard_analyzer [args...]
```

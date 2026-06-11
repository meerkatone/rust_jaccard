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
   cargo build --release

   If the plugin fails to load due to the following error message "This plugin was built for an outdated core ABI (XXX). Please rebuild the plugin with the latest API (XXX)." Please use the following to update the dependencies:
   cargo update && cargo build --release
   ```

3. **Install Plugin**:
   - Copy the plugin directory to your Binary Ninja plugins folder
   - The plugin will automatically register when Binary Ninja starts

## Usage

### From Binary Ninja UI

The plugin registers two commands (Plugins menu), both of which run through the
bundled Rust `jaccard_analyzer` engine and prompt for a Parquet output location:

- **Jaccard Similarity** — pairwise: select a folder of `.bndb` files; every pair is compared.
- **Jaccard Similarity (Reference)** — compare the current (saved) binary against a folder of binaries.

### Command Line Interface

The plugin includes a standalone CLI tool:

```bash
# Pairwise analysis of all binaries in a folder
./target/release/jaccard_analyzer -f /path/to/binaries -o results.parquet -p

# Compare reference binary against folder
./target/release/jaccard_analyzer -r reference.bin -f /path/to/binaries -o results.parquet
```

## Algorithm Details

### Feature Extraction

The analyzer reads each file's raw bytes and hashes overlapping/aligned byte windows of three sizes into three hash sets. The set names are kept for output-schema compatibility, but they are **byte windows, not disassembled code**:

1. **"Instructions"**: SHA-256 hashes of 4-byte windows
2. **"Functions"**: SHA-256 hashes of 16-byte windows
3. **"Basic Blocks"**: SHA-256 hashes of 8-byte windows

### Similarity Calculation

Jaccard similarity is calculated for each byte-window set:

```
J(A,B) = |A ∩ B| / |A ∪ B|
```

Overall similarity uses a weighted combination of the three sets:
- "Instructions" (4-byte windows): 40%
- "Functions" (16-byte windows): 40%
- "Basic Blocks" (8-byte windows): 20%

### Output Format

Results are exported in Parquet format with the following schema:

| Column | Type | Description |
|--------|------|-------------|
| binary1 | string | First binary name |
| binary2 | string | Second binary name |
| binary_pair | string | Pair identifier |
| jaccard_index | float64 | Overall similarity score |
| instruction_similarity | float64 | Instruction-level similarity |
| function_similarity | float64 | Function-level similarity |
| basic_block_similarity | float64 | Basic block-level similarity |

## Development

### Building

```bash
# Build release version
cargo build --release

# Build debug version
cargo build
```

### Testing

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture
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
- **Memory Efficient**: Streaming processing for large datasets
- **Optimized Storage**: Compressed Parquet format reduces file size by ~70%

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes and add tests
4. Run `cargo test` and `cargo clippy`
5. Submit a pull request

## Troubleshooting

### Common Issues

**"Rust binary not found" error**:
- Ensure you've run `cargo build --release`
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

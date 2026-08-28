use anyhow::{Context, Result};
use clap::{Arg, Command};
use rust_jaccard::{
    analyze_folder_jaccard, analyze_folder_pairwise_jaccard_with_limits,
    DEFAULT_MAX_PAIRWISE_FILES, DEFAULT_MAX_PAIRWISE_INPUT_BYTES,
};

fn main() -> Result<()> {
    env_logger::init();

    let matches = Command::new("Jaccard Binary Analyzer")
        .version("0.1.0")
        .about("Performs Jaccard similarity analysis on binary files")
        .arg(
            Arg::new("reference")
                .short('r')
                .long("reference")
                .value_name("FILE")
                .help("Reference binary file (not used in pairwise mode)")
                .required(false),
        )
        .arg(
            Arg::new("folder")
                .short('f')
                .long("folder")
                .value_name("DIR")
                .help("Folder containing binaries to compare")
                .required(true),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output Parquet file")
                .required(true),
        )
        .arg(
            Arg::new("pairwise")
                .short('p')
                .long("pairwise")
                .help("Perform pairwise comparison of all binaries (default mode)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("max-pairwise-files")
                .long("max-pairwise-files")
                .value_name("COUNT")
                .help("Maximum binaries accepted in pairwise mode")
                .value_parser(clap::value_parser!(usize))
                .default_value("256"),
        )
        .arg(
            Arg::new("max-pairwise-input-mib")
                .long("max-pairwise-input-mib")
                .value_name("MIB")
                .help("Maximum total input size accepted in pairwise mode")
                .value_parser(clap::value_parser!(u64))
                .default_value("512"),
        )
        .get_matches();

    let folder_path = matches.get_one::<String>("folder").unwrap();
    let output_path = matches.get_one::<String>("output").unwrap();
    let pairwise_mode = matches.get_flag("pairwise");

    if pairwise_mode || matches.get_one::<String>("reference").is_none() {
        // Pairwise mode (default)
        let max_files = matches
            .get_one::<usize>("max-pairwise-files")
            .copied()
            .unwrap_or(DEFAULT_MAX_PAIRWISE_FILES);
        let max_input_mib = matches
            .get_one::<u64>("max-pairwise-input-mib")
            .copied()
            .unwrap_or(DEFAULT_MAX_PAIRWISE_INPUT_BYTES / (1024 * 1024));
        let max_total_bytes = max_input_mib
            .checked_mul(1024 * 1024)
            .context("--max-pairwise-input-mib is too large")?;
        analyze_folder_pairwise_jaccard_with_limits(
            folder_path,
            output_path,
            max_files,
            max_total_bytes,
        )?;
        println!(
            "Pairwise analysis completed successfully. Results saved to {}",
            output_path
        );
    } else {
        // Reference mode
        let reference_path = matches.get_one::<String>("reference").unwrap();
        analyze_folder_jaccard(reference_path, folder_path, output_path)?;
        println!(
            "Reference-based analysis completed successfully. Results saved to {}",
            output_path
        );
    }

    Ok(())
}

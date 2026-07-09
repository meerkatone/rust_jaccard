use anyhow::Result;
use clap::{Arg, Command};
use rust_jaccard::{analyze_folder_jaccard, analyze_folder_pairwise_jaccard};

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
        .get_matches();

    let folder_path = matches.get_one::<String>("folder").unwrap();
    let output_path = matches.get_one::<String>("output").unwrap();
    let pairwise_mode = matches.get_flag("pairwise");

    if pairwise_mode || matches.get_one::<String>("reference").is_none() {
        // Pairwise mode (default)
        analyze_folder_pairwise_jaccard(folder_path, output_path)?;
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

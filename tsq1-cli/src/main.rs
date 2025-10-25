use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

/// Convert Standard MIDI Files into TSQ1 sequences.
#[derive(Parser, Debug)]
#[command(author, version, about = "TSQ1 toolkit", long_about = None)]
struct Cli {
    /// Path to the input SMF (.mid) file
    #[arg(value_hint = clap::ValueHint::FilePath)]
    input: PathBuf,
    /// Destination for the generated TSQ file (defaults to changing extension to .tsq)
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let midi_data = std::fs::read(&cli.input)
        .with_context(|| format!("failed to read MIDI file: {}", cli.input.display()))?;
    let tsq_data = tsq1::convert_midi_to_tsq_vec(&midi_data)
        .with_context(|| format!("failed to convert MIDI to TSQ: {}", cli.input.display()))?;
    let output_path = cli
        .output
        .clone()
        .unwrap_or_else(|| cli.input.with_extension("tsq"));
    std::fs::write(&output_path, tsq_data)
        .with_context(|| format!("failed to write TSQ file: {}", output_path.display()))?;
    println!("Wrote {}", output_path.display());
    Ok(())
}

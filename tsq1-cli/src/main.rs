use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

#[derive(Copy, Clone, Debug, ValueEnum, Eq, PartialEq)]
enum Direction {
    MidiToTsq,
    TsqToMidi,
}

/// Convert between Standard MIDI Files and TSQ1 sequences.
#[derive(Parser, Debug)]
#[command(author, version, about = "TSQ1 toolkit", long_about = None)]
struct Cli {
    /// Path to the input SMF (.mid) file
    #[arg(value_hint = clap::ValueHint::FilePath)]
    input: PathBuf,
    /// Destination for the generated TSQ file (defaults to changing extension to .tsq)
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    output: Option<PathBuf>,
    /// Conversion direction
    #[arg(short, long, value_enum, default_value_t = Direction::MidiToTsq)]
    direction: Direction,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let output_path = cli.output.clone().unwrap_or_else(|| match cli.direction {
        Direction::MidiToTsq => cli.input.with_extension("tsq"),
        Direction::TsqToMidi => cli.input.with_extension("mid"),
    });

    match cli.direction {
        Direction::MidiToTsq => {
            let midi_data = std::fs::read(&cli.input)
                .with_context(|| format!("failed to read MIDI file: {}", cli.input.display()))?;
            let tsq_data = tsq1::convert_midi_to_tsq_vec(&midi_data).with_context(|| {
                format!("failed to convert MIDI to TSQ: {}", cli.input.display())
            })?;
            std::fs::write(&output_path, tsq_data)
                .with_context(|| format!("failed to write TSQ file: {}", output_path.display()))?;
        }
        Direction::TsqToMidi => {
            let tsq_data = std::fs::read(&cli.input)
                .with_context(|| format!("failed to read TSQ file: {}", cli.input.display()))?;
            let midi_data = tsq1::convert_tsq_to_midi_vec(&tsq_data).with_context(|| {
                format!("failed to convert TSQ to MIDI: {}", cli.input.display())
            })?;
            std::fs::write(&output_path, midi_data)
                .with_context(|| format!("failed to write MIDI file: {}", output_path.display()))?;
        }
    }

    println!("Wrote {}", output_path.display());
    Ok(())
}

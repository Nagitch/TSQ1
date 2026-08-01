//! Command-line conversion between Standard MIDI Files and TSQ1 sequences.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Copy, Clone, Debug, ValueEnum, Eq, PartialEq)]
enum Direction {
    MidiToTsq,
    TsqToMidi,
}

#[derive(Args, Debug)]
struct ConvertArgs {
    /// Path to the input SMF (.mid) or TSQ1 (.tsq) file.
    #[arg(value_hint = clap::ValueHint::FilePath)]
    input: PathBuf,
    /// Destination path (defaults to changing the extension).
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    output: Option<PathBuf>,
    /// Conversion direction.
    #[arg(short, long, value_enum, default_value_t = Direction::MidiToTsq)]
    direction: Direction,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Convert between Standard MIDI Files and TSQ1 sequences.
    Convert(ConvertArgs),
    /// Decode a TSQ1 file and print its complete JSON model.
    Inspect {
        /// TSQ1 input file.
        #[arg(value_hint = clap::ValueHint::FilePath)]
        input: PathBuf,
        /// Emit compact JSON instead of pretty-printed JSON.
        #[arg(long)]
        compact: bool,
    },
    /// Validate a TSQ1 file without modifying it.
    Validate {
        /// TSQ1 input file.
        #[arg(value_hint = clap::ValueHint::FilePath)]
        input: PathBuf,
    },
}

/// Convert, inspect, and validate TSQ1 sequences.
#[derive(Parser, Debug)]
#[command(author, version, about = "TSQ1 toolkit", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Legacy conversion input. Prefer `tsq1-cli convert`.
    #[arg(value_hint = clap::ValueHint::FilePath)]
    input: Option<PathBuf>,
    /// Legacy conversion output.
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    output: Option<PathBuf>,
    /// Legacy conversion direction.
    #[arg(short, long, value_enum, default_value_t = Direction::MidiToTsq)]
    direction: Direction,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Convert(args)) => convert(args),
        Some(Command::Inspect { input, compact }) => inspect(&input, compact),
        Some(Command::Validate { input }) => validate(&input),
        None => {
            let input = cli
                .input
                .ok_or_else(|| anyhow!("an input file or subcommand is required; use --help"))?;
            convert(ConvertArgs {
                input,
                output: cli.output,
                direction: cli.direction,
            })
        }
    }
}

fn convert(args: ConvertArgs) -> Result<()> {
    let output_path = args.output.clone().unwrap_or_else(|| match args.direction {
        Direction::MidiToTsq => args.input.with_extension("tsq"),
        Direction::TsqToMidi => args.input.with_extension("mid"),
    });

    match args.direction {
        Direction::MidiToTsq => {
            let midi_data = std::fs::read(&args.input)
                .with_context(|| format!("failed to read MIDI file: {}", args.input.display()))?;
            let tsq_data = tsq1::convert_midi_to_tsq_vec(&midi_data).with_context(|| {
                format!("failed to convert MIDI to TSQ: {}", args.input.display())
            })?;
            std::fs::write(&output_path, tsq_data)
                .with_context(|| format!("failed to write TSQ file: {}", output_path.display()))?;
        }
        Direction::TsqToMidi => {
            let tsq_data = std::fs::read(&args.input)
                .with_context(|| format!("failed to read TSQ file: {}", args.input.display()))?;
            let midi_data = tsq1::convert_tsq_to_midi_vec(&tsq_data).with_context(|| {
                format!("failed to convert TSQ to MIDI: {}", args.input.display())
            })?;
            std::fs::write(&output_path, midi_data)
                .with_context(|| format!("failed to write MIDI file: {}", output_path.display()))?;
        }
    }

    println!("Wrote {}", output_path.display());
    Ok(())
}

fn inspect(input: &PathBuf, compact: bool) -> Result<()> {
    let bytes = std::fs::read(input)
        .with_context(|| format!("failed to read TSQ1 file: {}", input.display()))?;
    let sequence = tsq1::Sequence::decode(&bytes)
        .with_context(|| format!("invalid TSQ1: {}", input.display()))?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    if compact {
        serde_json::to_writer(&mut output, &sequence)?;
    } else {
        serde_json::to_writer_pretty(&mut output, &sequence)?;
    }
    use std::io::Write;
    writeln!(output)?;
    Ok(())
}

fn validate(input: &PathBuf) -> Result<()> {
    let bytes = std::fs::read(input)
        .with_context(|| format!("failed to read TSQ1 file: {}", input.display()))?;
    let sequence = tsq1::Sequence::decode(&bytes)
        .with_context(|| format!("invalid TSQ1: {}", input.display()))?;
    let event_count: usize = sequence.tracks.iter().map(|track| track.events.len()).sum();
    println!(
        "Valid TSQ1: {} track(s), {event_count} event(s), {} marker(s)",
        sequence.tracks.len(),
        sequence.markers.len()
    );
    Ok(())
}

# TSQ1

[![Rust CI](https://github.com/Nagitch/TSQ1/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Nagitch/TSQ1/actions/workflows/rust-ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.96](https://img.shields.io/badge/rust-1.96.0-orange.svg)](rust-toolchain.toml)

**TSQ1 (Time Sequence Quantized)** is a compact binary format for discrete
events on musical and absolute timelines. The repository contains the draft
format specification, a Rust conversion library, and a command-line tool.

The current implementation converts Standard MIDI Files (SMF) to and from the
musical-time subset of TSQ1. The draft format is broader than the implemented
subset; see [Implementation status](#implementation-status) before integrating
it.

## Format overview

| Item | Description |
| --- | --- |
| Extension | `.tsq` |
| Magic | `TSQ1` |
| Endianness | Little endian |
| Intended events | MIDI, OSC, lighting, synchronization, and custom events |
| Time axes | Musical ticks/PPQ and absolute microseconds or nanoseconds |
| Container | Chunked tracks (`TRK `), inspired by SMF |

Read the detailed draft specification in
[English](TSQ1_SPEC_v1.0_Draft.md) or
[日本語](TSQ1_SPEC_v1.0_Draft_JA.md).

## Implementation status

Implemented:

- SMF-to-TSQ1 and TSQ1-to-SMF conversion for PPQ-based musical timing
- MIDI channel, meta, SysEx, and escape events covered by the test suite
- `no_std` library builds with allocation support
- C-compatible SMF-to-TSQ1 conversion and buffer release functions

Not yet implemented:

- Absolute-time events
- Custom events
- General OSC event conversion
- SMPTE timecode input

The specification remains a draft. Compatibility-sensitive consumers should
pin a commit until a stable format version is released.

## Repository layout

```text
.
├── crates/tsq1/       # reusable Rust library
├── crates/tsq1-ffi/   # C-compatible dynamic library
├── tests/no-std/      # compile-only no-std integration check
├── tools/tsq1-cli/    # SMF/TSQ1 command-line converter
├── .github/           # CI and contribution automation
└── TSQ1_SPEC_*.md     # English and Japanese draft specifications
```

## Getting started

The repository pins Rust in `rust-toolchain.toml`. With
[rustup](https://rustup.rs/) installed, Cargo selects it automatically.

Convert a MIDI file to TSQ1:

```console
cargo run -p tsq1-cli -- input.mid
```

Convert TSQ1 back to MIDI:

```console
cargo run -p tsq1-cli -- input.tsq --direction tsq-to-midi
```

Use an explicit output path with `--output path/to/output.tsq`. Run
`cargo run -p tsq1-cli -- --help` for the complete CLI reference.

Library consumers can call the byte-oriented API directly:

```rust
let midi = std::fs::read("input.mid")?;
let tsq = tsq1::convert_midi_to_tsq_vec(&midi)?;
let roundtrip_midi = tsq1::convert_tsq_to_midi_vec(&tsq)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [library crate](crates/tsq1/README.md) and
[CLI tool](tools/tsq1-cli/README.md) documentation for component-specific
details.

## Development

Run the same checks enforced by CI:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p tsq1-no-std-check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Dependency update PRs are created by Dependabot. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and
[SECURITY.md](SECURITY.md) for private vulnerability reporting.

## License

Licensed under the [MIT License](LICENSE).

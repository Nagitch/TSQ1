# TSQ1

[![Rust CI](https://github.com/Nagitch/TSQ1/actions/workflows/rust-ci.yml/badge.svg)](https://github.com/Nagitch/TSQ1/actions/workflows/rust-ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust 1.96](https://img.shields.io/badge/rust-1.96.0-orange.svg)](rust-toolchain.toml)

**TSQ1 (Time Sequence Quantized)** is a compact binary format for discrete
events on musical and absolute timelines. The repository contains the draft
format specification, Rust libraries, a command-line tool, and a VS Code
editor.

The repository implements the complete v1 draft data model in Rust and
TypeScript, SMF interoperability, OSC helpers, a command-line toolkit, and a
VS Code binary custom editor.

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
The rationale for cross-cutting format and implementation choices is recorded
in the [architecture decision records](docs/adr/README.md).

## Implementation status

Implemented and covered by shared compatibility tests:

- Musical and absolute event domains with checked `SYNC` interpolation
- MIDI, meta, SysEx/escape, RAW OSC, MessagePack OSC, CBOR payload storage,
  and custom events
- `TMAP`, `SYNC`, `MARK`, `SMPF`, and byte-preserving unknown chunks
- SMF import/export for metrical and SMPTE divisions (24, 25, 29.97
  drop-frame, and 30 fps)
- Owned `Sequence` decode, validate, edit, and canonical encode APIs
- `osc-ir` MessagePack interoperability through the `tsq1-osc` crate
- CLI conversion, JSON inspection, and byte-offset-aware validation
- VS Code binary custom editor with undo/redo, save, save-as, revert, backup,
  diagnostics, all event kinds, and installable VSIX packaging
- Allocation-backed `no_std` core and C-compatible conversion functions
- Version-pinned, `no_std` shared calculation-kernel adapter for runtime queries

SMF cannot represent OSC or custom events. Export therefore reports an error
instead of silently dropping them. Absolute events require either `SYNC`
anchors or retained SMPTE source timing when exported to SMF.

The specification remains a draft. Compatibility-sensitive consumers should
pin a commit until a stable format version is released.

## Repository layout

```text
.
├── crates/tsq1/         # reusable Rust library
├── crates/tsq1-ffi/     # C-compatible dynamic library
├── crates/tsq1-osc/     # osc-ir / MessagePack interoperability
├── editors/vscode-tsq1/ # VS Code binary custom editor
├── tests/no-std/        # compile-only no-std integration check
├── tests/fixtures/      # shared Rust/TypeScript canonical files
├── tools/tsq1-cli/      # SMF/TSQ1 command-line converter
├── .github/             # CI and contribution automation
└── TSQ1_SPEC_*.md       # English and Japanese draft specifications
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

Inspect or validate the complete binary model:

```console
cargo run -p tsq1-cli -- inspect input.tsq
cargo run -p tsq1-cli -- validate input.tsq
```

Library consumers can call the byte-oriented API directly:

```rust
let midi = std::fs::read("input.mid")?;
let tsq = tsq1::convert_midi_to_tsq_vec(&midi)?;
let roundtrip_midi = tsq1::convert_tsq_to_midi_vec(&tsq)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

See the [library crate](crates/tsq1/README.md) and
[CLI tool](tools/tsq1-cli/README.md) documentation for component-specific
details. To build the editor:

```console
cd editors/vscode-tsq1
npm ci
npm run vsix
```

Install `editors/vscode-tsq1/dist/tsq1-editor.vsix` from VS Code's
**Extensions: Install from VSIX...** command.

## Development

Run the same checks enforced by CI:

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p tsq1-no-std-check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
cd editors/vscode-tsq1
npm ci
npm run package
```

Dependency update PRs are created by Dependabot. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the contribution workflow and
[SECURITY.md](SECURITY.md) for private vulnerability reporting.

## License

Licensed under the [MIT License](LICENSE).

# 0003: Edit an owned model and encode canonically

- Status: Accepted
- Decision date: 2026-07-31

## Context

Library, CLI, FFI, and editor consumers need more than streaming playback: they
must inspect, validate, modify, undo, serialize, and compare a complete
sequence. Editing raw byte ranges couples every operation to chunk positions
and can leave partially valid state. At the same time, equivalent models should
not produce indefinitely varying binary layouts.

## Decision

Decode a TSQ1 file into a complete owned `Sequence` containing tracks, events,
tempo entries, sync anchors, markers, optional SMPTE timing, flags, and unknown
chunks. Validate structural and semantic invariants before returning the model
and again before encoding it.

Encode valid models canonically: write the fixed header, derive known header
flags from the model, emit known chunks in a stable order, then emit retained
unknown chunks. Treat known data semantically rather than promising to preserve
its original byte layout. Return offset-aware errors for malformed binary
input.

## Consequences

- Consumers can perform transactional model edits and use ordinary snapshots
  for undo, redo, save, revert, and backup.
- Canonical output enables stable fixtures and cross-language conformance
  checks.
- A decode/encode cycle may normalize known flags, chunk order, and equivalent
  legacy event encodings; it is not a byte-preserving editor for known data.
- The complete model requires allocation proportional to the sequence, so the
  core supports `no_std + alloc` rather than allocation-free operation.
- Very large sequences may eventually need a separate indexed or streaming API
  that preserves the same validation contract.

## Alternatives considered

- **Expose only conversion functions**: small API, but insufficient for an
  editor or format-aware tooling.
- **Edit raw chunk byte ranges**: can preserve layout exactly, but makes
  invariant-preserving edits and cross-domain timing changes fragile.
- **Streaming decode and encode only**: lowers peak memory but complicates
  validation, undo/redo, unknown-chunk retention, and random editing.

## References

- [`tsq1` library model](../../crates/tsq1/README.md)
- [`Sequence` implementation](../../crates/tsq1/src/model.rs)
- Pull request [#11](https://github.com/Nagitch/TSQ1/pull/11)

# 0006: Lock Rust and TypeScript codecs with shared fixtures

- Status: Accepted
- Decision date: 2026-08-01

## Context

The VS Code custom editor needs to open, diagnose, edit, undo, and save a binary
document within the extension host. Shipping a native executable or addon for
every extension platform would add packaging and process boundaries. A native
TypeScript codec gives the editor direct model access, but creates a second
implementation that can drift from the Rust library.

JavaScript also cannot exactly represent every `u64` as a number, while TSQ1
uses `u64` deltas and positions.

## Decision

Maintain a complete TypeScript model and codec in the VS Code extension, with
the same validation and canonical encoding contract as Rust. Lock the two
implementations together using one canonical full-featured binary fixture that
is generated and decoded by Rust and decoded/re-encoded by TypeScript. The
fixture covers every event kind, both domains, timing chunks, markers, SMPTE
metadata, flags, and an unknown chunk.

Represent `u64` in human-readable models as a JavaScript number only within the
safe-integer range and otherwise as a canonical decimal string. Preserve exact
values across Rust serde, TypeScript editing, and binary encoding. Keep invalid
input bytes open for diagnosis and never overwrite them until a valid model has
been encoded. Reject stale webview mutations using document revisions.

## Consequences

- The editor has immediate binary access without a native runtime dependency.
- Rust and TypeScript can evolve independently only when their shared fixture,
  paired tests, and draft specifications continue to agree.
- Two codecs increase review and maintenance cost; a change to binary semantics
  is incomplete until both implementations and compatibility tests change.
- The fixture proves canonical interoperability for covered features, while
  malformed-input and edge-case tests remain necessary in each language.
- Decimal strings become part of JSON-facing compatibility for large timing
  values.

## Alternatives considered

- **Invoke the Rust CLI from the editor**: keeps one codec, but requires native
  binaries and a process protocol on every extension platform.
- **Compile the Rust codec to WASM**: keeps one implementation, but adds a WASM
  build and packaging pipeline and may constrain integration APIs.
- **Use a Node native addon or C FFI**: reuses Rust but creates ABI and
  platform-specific packaging obligations.
- **Use only ordinary JavaScript numbers**: simple, but corrupts valid values
  above `2^53 - 1`.

## References

- [Shared Rust fixture test](../../crates/tsq1/tests/full_format.rs)
- [TypeScript codec tests](../../editors/vscode-tsq1/src/codec.test.ts)
- [VS Code editor](../../editors/vscode-tsq1/README.md)
- Pull request [#11](https://github.com/Nagitch/TSQ1/pull/11)

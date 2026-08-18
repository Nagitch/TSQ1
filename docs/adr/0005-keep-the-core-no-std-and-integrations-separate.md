# 0005: Keep the core `no_std` and integrations separate

- Status: Accepted
- Decision date: 2026-07-31

## Context

TSQ1 is intended for desktop tooling as well as embedded, engine, and foreign
language consumers. Format decoding, validation, canonical encoding, and time
mapping do not inherently require filesystem, process, or network APIs. OSC
MessagePack integration, command-line behavior, and dynamic-library packaging
do require additional dependencies and platform facilities.

## Decision

Keep `crates/tsq1` usable with `no_std + alloc`. The core owns the complete
format model, codec, validation, timing, and SMF conversion. Standard-library
error integration and C allocation helpers are feature-gated.

Place delivery and ecosystem boundaries in separate workspace packages:

- `tsq1-ffi` builds the C-compatible dynamic library while retaining the core
  Rust `rlib`;
- `tsq1-osc` owns `osc-ir` and MessagePack interoperability; and
- `tsq1-cli` owns filesystem paths, terminal behavior, JSON inspection, and
  command-line conversion.

Continuously compile a dedicated `no_std` consumer in CI.

## Consequences

- Embedded and engine integrations can reuse one codec without linking CLI or
  OSC tooling.
- Optional integrations cannot accidentally expand the core dependency and
  platform contract.
- APIs use owned allocation-backed data and core-compatible error types rather
  than relying on `std` collections or I/O traits.
- Cross-crate public types and features require coordination, and FFI ownership
  rules remain a separate compatibility surface.
- `no_std` support does not mean allocation-free or suitable for unbounded
  inputs; callers still need resource limits appropriate to their environment.

## Alternatives considered

- **Require `std` in the core**: simplifies implementation but excludes target
  environments that only provide allocation.
- **Put CLI, FFI, and OSC adapters in one crate**: fewer packages, but feature
  selection and dependencies become coupled.
- **Maintain separate embedded and desktop codecs**: allows specialization but
  risks format divergence.

## References

- [`tsq1` crate](../../crates/tsq1/README.md)
- [`tsq1-ffi`](../../crates/tsq1-ffi/README.md)
- [`tsq1-osc`](../../crates/tsq1-osc/README.md)
- [Workspace CI](../../.github/workflows/rust-ci.yml)

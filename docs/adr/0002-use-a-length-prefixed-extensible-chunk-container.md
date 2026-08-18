# 0002: Use a length-prefixed extensible chunk container

- Status: Accepted
- Decision date: 2026-08-01

## Context

The format needs independent tracks, timing maps, synchronization anchors,
markers, source timing, and future extensions. Readers must be able to skip
features they do not understand without guessing their size. Lossless editors
must also avoid deleting extension data merely because their current version
cannot interpret it.

## Decision

After the fixed `TSQ1` header, encode sections as four-byte chunk IDs, a
little-endian `u32` payload length, and payload bytes. Known v1 chunks are
`TRK `, `TMAP`, `SYNC`, `MARK`, and `SMPF`.

Readers skip unknown chunks by their declared lengths. An editor that rewrites
a sequence retains each unknown ID and payload byte-for-byte and preserves
unknown header flag bits. Canonical encoding emits known chunks in the defined
order followed by retained unknown chunks in their relative order. Unknown
event kinds inside `TRK ` are rejected because individual track events do not
have a general enclosing payload length.

## Consequences

- New top-level chunks can be added without preventing older readers from
  traversing the rest of the file.
- Lossless editors preserve extension payloads and unknown header capabilities.
- Canonical rewrites preserve unknown content, but not its original position
  among known chunks.
- Lengths and cursor arithmetic must be checked before allocation or slicing,
  and malformed inputs should report their byte offsets.
- Extending the event-kind space compatibly requires a separate framing or
  version decision; the chunk rule alone does not solve that case.

## Alternatives considered

- **One fixed monolithic structure**: compact, but every added section changes
  how older readers locate later data.
- **Require readers to reject every unknown chunk**: simpler validation, but
  prevents forward-compatible tools and safe metadata extensions.
- **Make every field self-describing**: more flexible, but adds size and parsing
  overhead to a compact event format.

## References

- [TSQ1 v1 draft, chunk container](../../TSQ1_SPEC_v1.0_Draft.md#2-chunk-container)
- [`UnknownChunk` model and canonical encoder](../../crates/tsq1/src/model.rs)
- Pull request [#11](https://github.com/Nagitch/TSQ1/pull/11)

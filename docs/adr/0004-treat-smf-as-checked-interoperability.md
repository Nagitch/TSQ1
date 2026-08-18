# 0004: Treat SMF as checked interoperability

- Status: Accepted
- Decision date: 2025-10-29

## Context

Standard MIDI File is an important interchange format, but it cannot represent
the complete TSQ1 model. OSC and custom events have no SMF equivalent, mixed
time domains need an explicit mapping, sequential SMF tracks can carry
independent tempo timelines, and SMPTE divisions encode source timing that a
plain PPQ conversion would lose.

## Decision

Implement SMF import and export as checked adapters around the TSQ1 `Sequence`,
not as the TSQ1 data model.

- Import metrical SMF into musical-domain events and a canonical `TMAP`.
- Import SMPTE SMF into absolute-domain events and retain its division in
  `SMPF` for round-trip export.
- Reject sequential SMF input because one TSQ1 tempo map cannot faithfully
  represent independent per-track tempo timelines.
- On export, use retained SMPTE timing only when all events are absolute;
  otherwise map into metrical time using `SYNC` anchors.
- Reject OSC, custom events, missing mappings, and out-of-range values instead
  of dropping or approximating them silently.
- Treat `TMAP` as authoritative for metrical export and synthesize one terminal
  end-of-track event per SMF track.

## Consequences

- MIDI-compatible material has predictable round trips, including SysEx escape
  status and supported SMPTE divisions.
- Conversion failure communicates real information loss to callers.
- TSQ1 remains free to model non-MIDI events and dual-domain time without being
  constrained by SMF.
- Applications that want a lossy export policy must make that policy explicit
  above the core conversion API.
- Tempo canonicalization and event ordering are part of the adapter contract
  and require dedicated regression tests.

## Alternatives considered

- **Make TSQ1 an SMF wrapper**: maximizes MIDI compatibility but cannot express
  the intended OSC, control, and dual-domain model cleanly.
- **Silently omit unsupported events**: produces a file but can create unsafe
  or materially different playback.
- **Flatten every sequence to microseconds**: avoids tempo conversion but loses
  musical structure and editable PPQ placement.

## References

- [TSQ1 conversion behavior](../../crates/tsq1/README.md)
- [CLI conversion interface](../../tools/tsq1-cli/README.md)
- Pull request [#2](https://github.com/Nagitch/TSQ1/pull/2) and the compatibility
  hardening merged in [#11](https://github.com/Nagitch/TSQ1/pull/11)

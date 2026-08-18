# 0001: Represent musical and absolute time explicitly

- Status: Accepted
- Decision date: 2025-10-25

## Context

TSQ1 sequences coordinate events whose natural clocks are different. Notes and
arrangement events should follow PPQ and tempo changes, while lighting,
automation, OSC, and synchronization events may need elapsed real time. A
single musical clock makes non-musical timing tempo-dependent; a single real
time clock discards musical intent. Floating-point seconds would also make
cross-platform binary results and boundary behavior difficult to reproduce.

## Decision

Give every track event an explicit `Musical` or `Absolute` domain bit. Its VLQ
delta advances the previous event in the same domain. Musical positions use
integer PPQ ticks. Absolute positions use integer microseconds or nanoseconds,
selected once in the file header.

Represent tempo independently in `TMAP`. Relate the domains through strictly
increasing `SYNC` anchors. Checked tick/time conversion uses integer linear
interpolation only within the covered anchor range; it does not silently
extrapolate. Markers likewise carry an explicit domain.

## Consequences

- Musical events retain score-relative intent while absolute events retain
  elapsed-time intent in the same sequence and track.
- Consumers of a mixed-domain track must maintain independent accumulated
  positions and map them before producing one playback order.
- Exact integer storage avoids architecture-dependent floating-point encoding;
  interpolation still needs checked wide arithmetic and a defined rounding
  direction.
- Cross-domain conversion requires at least two valid anchors unless retained
  SMPTE timing supplies the relevant export mapping.

## Alternatives considered

- **Musical ticks only**: simple and MIDI-like, but unsuitable for fixed-time
  control data across tempo changes.
- **Absolute time only**: general for playback, but loses editable musical
  placement and tempo-relative meaning.
- **Floating-point seconds**: convenient for applications, but not a stable,
  exact interchange representation.
- **Derive absolute time from tempo only**: works for musical material but does
  not express externally measured or synchronized anchors.

## References

- [TSQ1 v1 draft, overview and event layout](../../TSQ1_SPEC_v1.0_Draft.md)
- [`Sequence` time mapping](../../crates/tsq1/src/model.rs)

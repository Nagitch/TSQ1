export type U64 = number | string;

export type AbsoluteUnit = "microseconds" | "nanoseconds";
export type TimeDomain = "musical" | "absolute";
export type OscFormat = "raw" | "messagePack" | "cbor";
export type SmpteFps = "fps24" | "fps25" | "fps29Drop" | "fps30";

export interface OscEvent {
  format: OscFormat;
  data: number[];
}

export interface SysexEvent {
  status: number;
  data: number[];
}

export type EventKind =
  | { kind: "osc"; value: OscEvent }
  | { kind: "midi"; value: [number, number, number] }
  | { kind: "meta"; value: { type_id: number; data: number[] } }
  | { kind: "sysex"; value: SysexEvent }
  | { kind: "custom"; value: { type_id: number; data: number[] } };

export interface Event {
  delta: U64;
  domain: TimeDomain;
  kind: EventKind;
}

export interface Track {
  events: Event[];
}

export interface TempoEntry {
  tick: U64;
  microseconds_per_quarter: number;
}

export interface SyncAnchor {
  tick: U64;
  time: U64;
}

export interface Marker {
  domain: TimeDomain;
  position: U64;
  name: string;
  class: number;
  color_rgba: number | null;
}

export interface SmpteTiming {
  fps: SmpteFps;
  subframes: number;
}

export interface UnknownChunk {
  id: [number, number, number, number];
  data: number[];
}

export interface Sequence {
  ppq: number;
  absolute_unit: AbsoluteUnit;
  flags: number;
  tracks: Track[];
  tempo_map: TempoEntry[];
  sync_anchors: SyncAnchor[];
  markers: Marker[];
  smpte_timing: SmpteTiming | null;
  unknown_chunks: UnknownChunk[];
}

export interface DocumentState {
  model: Sequence | null;
  error: string | null;
}

const MAX_U64 = (1n << 64n) - 1n;
const MAX_SAFE_U64 = BigInt(Number.MAX_SAFE_INTEGER);

/** Remove an event without changing later absolute positions in its time domain. */
export function removeEventPreservingTimeline(track: Track, eventIndex: number): Event {
  if (!Number.isInteger(eventIndex) || eventIndex < 0 || eventIndex >= track.events.length) {
    throw new RangeError("event index is outside the track");
  }
  const removed = track.events[eventIndex]!;
  const next = track.events
    .slice(eventIndex + 1)
    .find((event) => event.domain === removed.domain);
  const combinedDelta =
    next === undefined ? undefined : addU64(removed.delta, next.delta, "combined event delta");

  track.events.splice(eventIndex, 1);
  if (next !== undefined && combinedDelta !== undefined) {
    next.delta = combinedDelta;
  }
  return removed;
}

function addU64(left: U64, right: U64, label: string): U64 {
  const result = parseU64(left, label) + parseU64(right, label);
  if (result > MAX_U64) {
    throw new RangeError(`${label} exceeds u64`);
  }
  return result <= MAX_SAFE_U64 ? Number(result) : result.toString();
}

function parseU64(value: U64, label: string): bigint {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new RangeError(`${label} must be a non-negative safe integer or decimal string`);
    }
    return BigInt(value);
  }
  if (!/^(0|[1-9][0-9]*)$/.test(value)) {
    throw new RangeError(`${label} must be an unsigned decimal value`);
  }
  const parsed = BigInt(value);
  if (parsed > MAX_U64) {
    throw new RangeError(`${label} exceeds u64`);
  }
  return parsed;
}

export function emptySequence(): Sequence {
  return {
    ppq: 480,
    absolute_unit: "microseconds",
    flags: 2,
    tracks: [{ events: [] }],
    tempo_map: [],
    sync_anchors: [],
    markers: [],
    smpte_timing: null,
    unknown_chunks: [],
  };
}

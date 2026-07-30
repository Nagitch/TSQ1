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

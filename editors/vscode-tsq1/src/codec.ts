import type {
  Event,
  EventKind,
  Marker,
  OscFormat,
  Sequence,
  SmpteFps,
  TimeDomain,
  U64,
} from "./model.js";

const HEADER_SIZE = 14;
const MAX_U64 = (1n << 64n) - 1n;
const MAX_SAFE = BigInt(Number.MAX_SAFE_INTEGER);
const FLAG_SYSEX_STATUS = 0x0001;
const FLAG_FIXED_MIDI_WIDTH = 0x0002;
const KNOWN_CHUNKS = new Set(["TRK ", "TMAP", "SYNC", "MARK", "SMPF"]);

export class FormatError extends Error {
  constructor(
    message: string,
    readonly offset?: number,
  ) {
    super(offset === undefined ? message : `${message} at byte ${offset}`);
    this.name = "FormatError";
  }
}

class Reader {
  private readonly view: DataView;
  offset = 0;

  constructor(
    private readonly bytes: Uint8Array,
    private readonly baseOffset = 0,
  ) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  get done(): boolean {
    return this.offset === this.bytes.length;
  }

  get position(): number {
    return this.baseOffset + this.offset;
  }

  fail(message: string, offset = this.position): never {
    throw new FormatError(message, offset);
  }

  take(length: number): Uint8Array {
    if (!Number.isSafeInteger(length) || length < 0 || this.offset + length > this.bytes.length) {
      this.fail("payload exceeds the remaining bytes");
    }
    const value = this.bytes.subarray(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }

  u8(): number {
    return this.take(1)[0]!;
  }

  u16(): number {
    this.take(2);
    const value = this.view.getUint16(this.offset - 2, true);
    return value;
  }

  u32(): number {
    this.take(4);
    return this.view.getUint32(this.offset - 4, true);
  }

  u64(): U64 {
    this.take(8);
    return fromBigInt(this.view.getBigUint64(this.offset - 8, true));
  }

  vlq(): U64 {
    let value = 0n;
    for (let count = 0; count < 10; count += 1) {
      const byte = this.u8();
      value = (value << 7n) | BigInt(byte & 0x7f);
      if (value > MAX_U64) {
        this.fail("VLQ exceeds u64");
      }
      if ((byte & 0x80) === 0) {
        return fromBigInt(value);
      }
    }
    this.fail("VLQ exceeds ten bytes");
  }
}

class Writer {
  private readonly bytes: number[] = [];

  u8(value: number): void {
    assertInteger(value, 0xff, "byte");
    this.bytes.push(value);
  }

  u16(value: number): void {
    assertInteger(value, 0xffff, "u16");
    this.bytes.push(value & 0xff, (value >>> 8) & 0xff);
  }

  u32(value: number): void {
    assertInteger(value, 0xffff_ffff, "u32");
    this.bytes.push(value & 0xff, (value >>> 8) & 0xff, (value >>> 16) & 0xff, (value >>> 24) & 0xff);
  }

  u64(value: U64): void {
    let remaining = toBigInt(value, "u64");
    for (let index = 0; index < 8; index += 1) {
      this.bytes.push(Number(remaining & 0xffn));
      remaining >>= 8n;
    }
  }

  vlq(value: U64): void {
    let remaining = toBigInt(value, "VLQ");
    const encoded = [Number(remaining & 0x7fn)];
    remaining >>= 7n;
    while (remaining > 0n) {
      encoded.push(Number(remaining & 0x7fn) | 0x80);
      remaining >>= 7n;
    }
    encoded.reverse();
    this.bytes.push(...encoded);
  }

  raw(data: ArrayLike<number>): void {
    for (let index = 0; index < data.length; index += 1) {
      this.u8(data[index]!);
    }
  }

  result(): Uint8Array {
    return Uint8Array.from(this.bytes);
  }
}

export function decodeSequence(bytes: Uint8Array): Sequence {
  const reader = new Reader(bytes);
  if (bytes.length < HEADER_SIZE) {
    reader.fail("TSQ1 header is truncated", 0);
  }
  if (ascii(reader.take(4)) !== "TSQ1") {
    reader.fail("TSQ1 magic is missing", 0);
  }
  if (reader.u16() !== 1) {
    reader.fail("unsupported TSQ1 version", 4);
  }
  const ppq = reader.u16();
  const absoluteUnitByte = reader.u8();
  const absolute_unit =
    absoluteUnitByte === 0
      ? "microseconds"
      : absoluteUnitByte === 1
        ? "nanoseconds"
        : reader.fail("unsupported absolute time unit", 8);
  if (reader.u8() !== 0) {
    reader.fail("header reserved byte must be zero", 9);
  }
  reader.u16();
  const flags = reader.u16();
  const sequence: Sequence = {
    ppq,
    absolute_unit,
    flags,
    tracks: [],
    tempo_map: [],
    sync_anchors: [],
    markers: [],
    smpte_timing: null,
    unknown_chunks: [],
  };

  while (!reader.done) {
    const chunkOffset = reader.position;
    const idBytes = reader.take(4);
    const id = ascii(idBytes);
    const length = reader.u32();
    const payloadOffset = reader.position;
    const payload = reader.take(length);
    const chunk = new Reader(payload, payloadOffset);
    switch (id) {
      case "TRK ":
        sequence.tracks.push(decodeTrack(chunk, flags));
        break;
      case "TMAP":
        if (length % 12 !== 0) {
          reader.fail("TMAP length is not a multiple of 12", chunkOffset);
        }
        while (!chunk.done) {
          sequence.tempo_map.push({
            tick: chunk.u64(),
            microseconds_per_quarter: chunk.u32(),
          });
        }
        break;
      case "SYNC":
        if (length % 16 !== 0) {
          reader.fail("SYNC length is not a multiple of 16", chunkOffset);
        }
        while (!chunk.done) {
          sequence.sync_anchors.push({ tick: chunk.u64(), time: chunk.u64() });
        }
        break;
      case "MARK":
        while (!chunk.done) {
          const domain = decodeDomain(chunk.u8(), chunk);
          const position = chunk.u64();
          const nameLength = toSafeNumber(chunk.vlq(), "marker name length");
          const nameOffset = chunk.position;
          let name = "";
          try {
            name = new TextDecoder("utf-8", { fatal: true }).decode(chunk.take(nameLength));
          } catch {
            chunk.fail("marker name is not UTF-8", nameOffset);
          }
          const markerClass = chunk.u8();
          const markerFlags = chunk.u8();
          if ((markerFlags & ~1) !== 0) {
            chunk.fail("unsupported MARK flags");
          }
          sequence.markers.push({
            domain,
            position,
            name,
            class: markerClass,
            color_rgba: (markerFlags & 1) === 0 ? null : chunk.u32(),
          });
        }
        break;
      case "SMPF": {
        if (length !== 2 || sequence.smpte_timing !== null) {
          reader.fail("SMPF must occur once with a two-byte payload", chunkOffset);
        }
        const fps = decodeFps(chunk.u8(), chunk);
        sequence.smpte_timing = { fps, subframes: chunk.u8() };
        break;
      }
      default:
        sequence.unknown_chunks.push({
          id: [idBytes[0]!, idBytes[1]!, idBytes[2]!, idBytes[3]!],
          data: [...payload],
        });
    }
  }
  try {
    validateSequence(sequence);
  } catch (error) {
    if (error instanceof FormatError && error.offset === undefined) {
      throw new FormatError(error.message, 0);
    }
    throw error;
  }
  return sequence;
}

export function encodeSequence(sequence: Sequence): Uint8Array {
  validateSequence(sequence);
  const hasSysex = sequence.tracks.some((track) =>
    track.events.some((event) => event.kind.kind === "sysex"),
  );
  let flags = sequence.flags | FLAG_FIXED_MIDI_WIDTH;
  flags = hasSysex ? flags | FLAG_SYSEX_STATUS : flags & ~FLAG_SYSEX_STATUS;

  const output = new Writer();
  output.raw(new TextEncoder().encode("TSQ1"));
  output.u16(1);
  output.u16(sequence.ppq);
  output.u8(sequence.absolute_unit === "microseconds" ? 0 : 1);
  output.u8(0);
  output.u16(sequence.tracks.length);
  output.u16(flags);

  for (const track of sequence.tracks) {
    const payload = new Writer();
    for (const event of track.events) {
      encodeEvent(event, payload, hasSysex);
    }
    writeChunk(output, "TRK ", payload.result());
  }
  if (sequence.tempo_map.length > 0) {
    const payload = new Writer();
    for (const entry of sequence.tempo_map) {
      payload.u64(entry.tick);
      payload.u32(entry.microseconds_per_quarter);
    }
    writeChunk(output, "TMAP", payload.result());
  }
  if (sequence.sync_anchors.length > 0) {
    const payload = new Writer();
    for (const anchor of sequence.sync_anchors) {
      payload.u64(anchor.tick);
      payload.u64(anchor.time);
    }
    writeChunk(output, "SYNC", payload.result());
  }
  if (sequence.markers.length > 0) {
    const payload = new Writer();
    for (const marker of sequence.markers) {
      payload.u8(encodeDomain(marker.domain));
      payload.u64(marker.position);
      const name = new TextEncoder().encode(marker.name);
      payload.vlq(name.length);
      payload.raw(name);
      payload.u8(marker.class);
      if (marker.color_rgba === null) {
        payload.u8(0);
      } else {
        payload.u8(1);
        payload.u32(marker.color_rgba);
      }
    }
    writeChunk(output, "MARK", payload.result());
  }
  if (sequence.smpte_timing !== null) {
    const payload = new Writer();
    payload.u8(encodeFps(sequence.smpte_timing.fps));
    payload.u8(sequence.smpte_timing.subframes);
    writeChunk(output, "SMPF", payload.result());
  }
  for (const chunk of sequence.unknown_chunks) {
    writeChunk(output, ascii(Uint8Array.from(chunk.id)), Uint8Array.from(chunk.data));
  }
  return output.result();
}

export function validateSequence(sequence: Sequence): void {
  assertInteger(sequence.ppq, 0xffff, "PPQ");
  if (sequence.ppq === 0) {
    throw new FormatError("PPQ must be greater than zero");
  }
  assertInteger(sequence.flags, 0xffff, "flags");
  if (sequence.absolute_unit !== "microseconds" && sequence.absolute_unit !== "nanoseconds") {
    throw new FormatError("unsupported absolute time unit");
  }
  if (sequence.tracks.length > 0xffff) {
    throw new FormatError("too many tracks");
  }
  ensureIncreasing(
    sequence.tempo_map.map((entry) => entry.tick),
    "tempo map ticks must be strictly increasing",
  );
  ensureIncreasing(
    sequence.sync_anchors.map((anchor) => anchor.tick),
    "sync ticks must be strictly increasing",
  );
  ensureIncreasing(
    sequence.sync_anchors.map((anchor) => anchor.time),
    "sync absolute times must be strictly increasing",
  );
  for (const entry of sequence.tempo_map) {
    assertInteger(entry.microseconds_per_quarter, 0xffff_ffff, "tempo");
  }
  const lastMarker = new Map<TimeDomain, bigint>();
  for (const marker of sequence.markers) {
    validateMarker(marker);
    const position = toBigInt(marker.position, "marker position");
    const previous = lastMarker.get(marker.domain);
    if (previous !== undefined && position < previous) {
      throw new FormatError("markers must be sorted within each domain");
    }
    lastMarker.set(marker.domain, position);
  }
  if (sequence.smpte_timing !== null) {
    encodeFps(sequence.smpte_timing.fps);
    assertInteger(sequence.smpte_timing.subframes, 0xff, "SMPTE subframes");
    if (sequence.smpte_timing.subframes === 0) {
      throw new FormatError("SMPTE subframes must be greater than zero");
    }
  }
  for (const track of sequence.tracks) {
    for (const event of track.events) {
      validateEvent(event);
    }
  }
  for (const chunk of sequence.unknown_chunks) {
    validateBytes(chunk.id, "unknown chunk id");
    const id = ascii(Uint8Array.from(chunk.id));
    if (KNOWN_CHUNKS.has(id)) {
      throw new FormatError("known chunk cannot be stored as unknown");
    }
    validateBytes(chunk.data, "unknown chunk data");
  }
}

function decodeTrack(reader: Reader, flags: number): { events: Event[] } {
  const events: Event[] = [];
  while (!reader.done) {
    const header = reader.u8();
    const domain: TimeDomain = (header & 0x80) === 0 ? "musical" : "absolute";
    const kind = header & 0x7f;
    const delta = reader.vlq();
    let eventKind: EventKind;
    switch (kind) {
      case 0: {
        const format = decodeOscFormat(reader.u8(), reader);
        const length = toSafeNumber(reader.vlq(), "OSC payload length");
        eventKind = { kind: "osc", value: { format, data: [...reader.take(length)] } };
        break;
      }
      case 1: {
        const status = reader.u8();
        const data1 = reader.u8();
        const data2 =
          (flags & FLAG_FIXED_MIDI_WIDTH) !== 0 || ![0xc, 0xd].includes(status >> 4)
            ? reader.u8()
            : 0;
        eventKind = { kind: "midi", value: [status, data1, data2] };
        break;
      }
      case 2: {
        const type_id = reader.u8();
        const length = toSafeNumber(reader.vlq(), "meta payload length");
        eventKind = { kind: "meta", value: { type_id, data: [...reader.take(length)] } };
        break;
      }
      case 3: {
        const length = toSafeNumber(reader.vlq(), "SysEx payload length");
        const payload = reader.take(length);
        if ((flags & FLAG_SYSEX_STATUS) !== 0) {
          if (payload.length === 0) {
            reader.fail("SysEx status is missing");
          }
          eventKind = {
            kind: "sysex",
            value: { status: payload[0]!, data: [...payload.subarray(1)] },
          };
        } else {
          eventKind = { kind: "sysex", value: { status: 0xf0, data: [...payload] } };
        }
        break;
      }
      case 0x7e: {
        const type_id = reader.u8();
        const length = toSafeNumber(reader.vlq(), "custom payload length");
        eventKind = { kind: "custom", value: { type_id, data: [...reader.take(length)] } };
        break;
      }
      default:
        reader.fail("unsupported track event kind");
    }
    events.push({ delta, domain, kind: eventKind });
  }
  return { events };
}

function encodeEvent(event: Event, output: Writer, sysexStatus: boolean): void {
  const kindCode = { osc: 0, midi: 1, meta: 2, sysex: 3, custom: 0x7e }[event.kind.kind];
  output.u8((encodeDomain(event.domain) << 7) | kindCode);
  output.vlq(event.delta);
  switch (event.kind.kind) {
    case "osc":
      output.u8(encodeOscFormat(event.kind.value.format));
      output.vlq(event.kind.value.data.length);
      output.raw(event.kind.value.data);
      break;
    case "midi":
      output.raw(event.kind.value);
      break;
    case "meta":
      output.u8(event.kind.value.type_id);
      output.vlq(event.kind.value.data.length);
      output.raw(event.kind.value.data);
      break;
    case "sysex":
      output.vlq(event.kind.value.data.length + Number(sysexStatus));
      if (sysexStatus) {
        output.u8(event.kind.value.status);
      }
      output.raw(event.kind.value.data);
      break;
    case "custom":
      output.u8(event.kind.value.type_id);
      output.vlq(event.kind.value.data.length);
      output.raw(event.kind.value.data);
      break;
  }
}

function validateEvent(event: Event): void {
  toBigInt(event.delta, "event delta");
  encodeDomain(event.domain);
  switch (event.kind.kind) {
    case "osc": {
      validateBytes(event.kind.value.data, "OSC payload");
      encodeOscFormat(event.kind.value.format);
      if (event.kind.value.format === "raw") {
        const first = event.kind.value.data[0];
        if (first !== 0x2f && first !== 0x23) {
          throw new FormatError("RAW OSC payload must start with '/' or '#'");
        }
        if (event.kind.value.data.length % 4 !== 0) {
          throw new FormatError("RAW OSC payload must be four-byte aligned");
        }
      }
      break;
    }
    case "midi": {
      if (event.kind.value.length !== 3) {
        throw new FormatError("MIDI events require three bytes");
      }
      validateBytes(event.kind.value, "MIDI event");
      const [status, data1, data2] = event.kind.value;
      if (status < 0x80 || status > 0xef) {
        throw new FormatError("MIDI status must be a channel message");
      }
      if (data1 > 0x7f || data2 > 0x7f) {
        throw new FormatError("MIDI data bytes must be seven-bit values");
      }
      if ([0xc, 0xd].includes(status >> 4) && data2 !== 0) {
        throw new FormatError("single-data-byte MIDI messages require zero padding");
      }
      break;
    }
    case "meta":
    case "custom":
      assertInteger(event.kind.value.type_id, 0xff, "event type");
      validateBytes(event.kind.value.data, "event payload");
      break;
    case "sysex":
      if (event.kind.value.status !== 0xf0 && event.kind.value.status !== 0xf7) {
        throw new FormatError("SysEx status must be 0xF0 or 0xF7");
      }
      validateBytes(event.kind.value.data, "SysEx payload");
      break;
  }
}

function validateMarker(marker: Marker): void {
  encodeDomain(marker.domain);
  toBigInt(marker.position, "marker position");
  assertInteger(marker.class, 0xff, "marker class");
  if (marker.color_rgba !== null) {
    assertInteger(marker.color_rgba, 0xffff_ffff, "marker color");
  }
}

function writeChunk(output: Writer, id: string, payload: Uint8Array): void {
  const idBytes = new TextEncoder().encode(id);
  if (idBytes.length !== 4) {
    throw new FormatError("chunk id must be four ASCII bytes");
  }
  output.raw(idBytes);
  output.u32(payload.length);
  output.raw(payload);
}

function decodeDomain(value: number, reader: Reader): TimeDomain {
  if (value === 0) return "musical";
  if (value === 1) return "absolute";
  return reader.fail("unsupported time domain");
}

function encodeDomain(domain: TimeDomain): number {
  if (domain === "musical") return 0;
  if (domain === "absolute") return 1;
  throw new FormatError("unsupported time domain");
}

function decodeOscFormat(value: number, reader: Reader): OscFormat {
  if (value === 0) return "raw";
  if (value === 1) return "messagePack";
  if (value === 2) return "cbor";
  return reader.fail("unsupported OSC payload format");
}

function encodeOscFormat(format: OscFormat): number {
  const value = { raw: 0, messagePack: 1, cbor: 2 }[format];
  if (value === undefined) {
    throw new FormatError("unsupported OSC payload format");
  }
  return value;
}

function decodeFps(value: number, reader: Reader): SmpteFps {
  const fps = { 24: "fps24", 25: "fps25", 29: "fps29Drop", 30: "fps30" }[value] as
    | SmpteFps
    | undefined;
  return fps ?? reader.fail("unsupported SMPTE frame rate");
}

function encodeFps(fps: SmpteFps): number {
  const value = { fps24: 24, fps25: 25, fps29Drop: 29, fps30: 30 }[fps];
  if (value === undefined) {
    throw new FormatError("unsupported SMPTE frame rate");
  }
  return value;
}

function ensureIncreasing(values: U64[], message: string): void {
  let previous: bigint | undefined;
  for (const value of values) {
    const current = toBigInt(value, "position");
    if (previous !== undefined && current <= previous) {
      throw new FormatError(message);
    }
    previous = current;
  }
}

function fromBigInt(value: bigint): U64 {
  return value <= MAX_SAFE ? Number(value) : value.toString();
}

function toBigInt(value: U64, label: string): bigint {
  let result: bigint;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < 0) {
      throw new FormatError(`${label} must be a non-negative safe integer or decimal string`);
    }
    result = BigInt(value);
  } else if (/^(0|[1-9][0-9]*)$/.test(value)) {
    result = BigInt(value);
  } else {
    throw new FormatError(`${label} must be an unsigned decimal value`);
  }
  if (result > MAX_U64) {
    throw new FormatError(`${label} exceeds u64`);
  }
  return result;
}

function toSafeNumber(value: U64, label: string): number {
  const result = toBigInt(value, label);
  if (result > MAX_SAFE) {
    throw new FormatError(`${label} exceeds JavaScript collection limits`);
  }
  return Number(result);
}

function assertInteger(value: number, maximum: number, label: string): void {
  if (!Number.isInteger(value) || value < 0 || value > maximum) {
    throw new FormatError(`${label} is outside its valid range`);
  }
}

function validateBytes(values: ArrayLike<number>, label: string): void {
  for (let index = 0; index < values.length; index += 1) {
    assertInteger(values[index]!, 0xff, `${label}[${index}]`);
  }
}

function ascii(bytes: Uint8Array): string {
  return String.fromCharCode(...bytes);
}

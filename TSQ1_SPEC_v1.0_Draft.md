# TSQ1 File Format Specification — v1.0 Draft (EN)

The TSQ1 (Time Sequence Quantized) format stores ordered musical and control events on
both musical and absolute time axes. This document describes the full structure of the
v1.0 draft without historical notes so that implementers can focus on the latest rules.

---

## 0. Overview
- Dual time domains: **Musical (Δticks, PPQ)** and **Absolute (Δtime, AbsUnit = μs/ns)**
- Little Endian for all multibyte values
- Variable-length quantity (VLQ) encoding for delta times (compatible with SMF)
- Chunk-based container with dedicated chunk identifiers (`"TRK "`, `"TMAP"`, `"SYNC"`, ...)
- Synchronisation through tempo maps and absolute anchors
 - Optional locator metadata via `"MARK"` chunk (sections, drops, etc.)

---

## 1. Header
| Off | Size | Type | Name | Description |
|---:|---:|---|---|---|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | `1` |
| 0x06 | 2 | `u16` | PPQ | Ticks per quarter note |
| 0x08 | 1 | `u8` | AbsUnit | 0 = Microseconds, 1 = Nanoseconds |
| 0x09 | 1 | - | Reserved | Must be 0 |
| 0x0A | 2 | `u16` | TrackCount | Advisory track count |
| 0x0C | 2 | `u16` | Flags | Encoding flags described below |

Header flags:

- `bit 0` (`0x0001`): SysEx payloads include their leading `0xF0` or `0xF7`
  status byte.
- `bit 1` (`0x0002`): MIDI messages use the canonical fixed three-byte
  payload. Writers of this draft must set this bit.
- Other bits are reserved. Readers must preserve unknown bits when rewriting a
  document and otherwise ignore them.

---

## 2. Chunk Container
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: Event stream chunk (musical and absolute events)
- `"TMAP"`: Tempo map entries `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: Absolute anchors `(tick:u64, time_abs:u64)*` where `time_abs` uses `AbsUnit`
- `"MARK"`: Locators/markers for arrangement sections and cues
- `"SMPF"`: Original SMF SMPTE division `[fps:u8][subframes:u8]`

Implementations may introduce additional chunks; unknown chunk IDs must be skipped by
using the declared length. Lossless editors should retain unknown chunks byte-for-byte.

---

## 3. TRK Chunk
### 3.1 Event Layout
```
[Header:1][ΔTime:VLQ][Payload...]
```
- `Header.bit7 = Domain` (`0` = Musical / `1` = Absolute)
- `Header.bit6..0 = EventKind`
- `ΔTime`: VLQ encoding (`Δtick` for musical events, `Δabs` for absolute events in `AbsUnit`)

### 3.2 EventKind Assignments
| Value | Constant | Description |
|---:|---|---|
| 0x00 | EK_OSC | OSC event (canonical) |
| 0x01 | EK_MIDI | MIDI message (3 bytes) |
| 0x02 | EK_META | Meta event (SMF-like) |
| 0x03 | EK_SYSEX | System Exclusive payload |
| 0x7E | EK_CUSTOM | Custom / vendor extensions |

---

## 4. Payload Definitions
### 4.1 OSC (`EventKind = 0x00`)
```
[OscFormat:u8][Length:VLQ][Data:N]
```
- `OscFormat`
  - `0x00 = RAW`: Byte-accurate OSC 1.0/1.1 datagram (`/path...` or `#bundle...`)
  - `0x01 = MSGPACK`: `{ "k": "msg"|"bun", "p": "/foo", "t": ",ifs", "a": [...], "ntp": u64? }`
  - `0x02 = CBOR`: Same schema encoded in CBOR
  - `0x20–0x7F`: Reserved
- RAW validation: the first byte is `'/'` or `'#'` and the packet length is
  four-byte aligned
- Emission time derives from `Header.Domain` and `ΔTime`; payload timetags remain untouched
- No fragmentation: one TSQ1 event encapsulates one OSC message or bundle

### 4.2 MIDI (`EventKind = 0x01`)
```
[Status:1][Data1:1][Data2:1]
```
- No running status; every canonical MIDI event stores all three bytes.
- Program Change (`0xCn`) and Channel Pressure (`0xDn`) set `Data2` to zero.
- For backward compatibility, readers may accept two-byte `0xCn`/`0xDn`
  messages when header flag `0x0002` is clear.

### 4.3 Meta (`EventKind = 0x02`)
```
[MetaType:1][Length:VLQ][Data:N]
```
- Mirrors SMF meta events (e.g., Tempo `0x51` uses 3-byte μs per quarter note)

### 4.4 SysEx (`EventKind = 0x03`)
```
[Length:VLQ][Data:N]
```
- With header flag `0x0001`, `Data` begins with the `0xF0` or `0xF7` status
  byte and `Length` includes it. Canonical writers use this form so escape and
  continuation events round-trip.
- With the flag clear, the legacy form excludes framing and readers assume
  `0xF0`.

### 4.5 Custom (`EventKind = 0x7E`)
```
[TypeID:1][Length:VLQ][Data:N]
```
- Reserved for vendor-specific or experimental extensions

---

## 5. Timing Chunks

### 5.1 TMAP
```
"TMAP"[len:u32] { [tick:u64][us_per_qn:u32] }*
```
- Entries must be strictly increasing by `tick`.
- `us_per_qn` is microseconds per quarter note, matching SMF tempo metadata.

### 5.2 SYNC
```
"SYNC"[len:u32] { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical position (PPQ-based)
- `time_abs`: Absolute position expressed in `AbsUnit`
- Provides tick ↔ time conversion via linear interpolation between anchors
- `time_abs` expresses elapsed sequence time, not wall-clock timestamps
- Both `tick` and `time_abs` must be strictly increasing.

### 5.3 SMPF
```
"SMPF"[len=2:u32][fps:u8][subframes:u8]
```
- Retains the source SMF SMPTE division for round-trip export.
- `fps` is `24`, `25`, `29` (29.97 drop-frame), or `30`.
- `subframes` must be non-zero.
- SMPTE-imported track deltas are represented in the Absolute domain; `SMPF`
  records how to reconstruct the original division.

---

## 6. MARK Chunk (Locators)
```
"MARK"[len:u32] { [pos_kind:u8][pos:u64][name_len:VLQ][name:N][class:u8][marker_flags:u8][color_rgba:u32]? }*
```
- Purpose: store non-audio, non-control metadata for arrangement navigation such as Ableton Live-style locators (Intro, Breakdown, Drop, etc.).
- Positioning:
  - `pos_kind`: `0 = Musical(tick)`, `1 = Absolute(time_abs in AbsUnit)`
  - `pos`: when `pos_kind=0`, PPQ ticks from start; when `1`, absolute time from start in `AbsUnit`.
- Label:
  - `name_len`: VLQ length of UTF-8 `name`
  - `name`: UTF-8 string; implementers should preserve casing and emoji if present
- Classification:
  - `class` (u8) focuses on SMF-compatible semantics to support non-musical timelines as well:
    - `0x00 = Generic`
    - `0x20 = Cue`
    - `0x7F = Custom`
- Color (optional):
  - `marker_flags.bit0 = 1` means `color_rgba` is present. Other marker flag
    bits are reserved and must be zero.
  - Encoded as little-endian `u32` RGBA (`0xAARRGGBB`). Consumers SHOULD ignore if unsupported.
- Ordering: entries must be sorted by `pos` within each `pos_kind`.
- Uniqueness: multiple locators may share the same position; consumers should handle duplicates.
- Extensibility: unknown `class` values must be accepted and treated as `Generic`.

### 6.1 Rationale
Locators are intentionally separated from the `TRK` event stream to avoid timing and playback side effects. They provide human-readable navigation and interoperability for general time-series sequences (not limited to music) without constraining controller/event semantics.

### 6.2 Examples
```
// Musical locator labeled "Generic"
pos_kind = 0  // Musical
pos      = 1024  // ticks from start
name_len = VLQ(len("Generic Marker"))
name     = "Generic Marker"
class    = 0x00  // Generic
marker_flags = 0x00

// Absolute locator at 90s, labeled "Cue", with optional color
pos_kind   = 1  // Absolute
pos        = 90_000_000  // assuming AbsUnit=μs
name_len   = VLQ(len("Cue"))
name       = "Cue"
class      = 0x20  // Cue
marker_flags = 0x01
color_rgba = 0xFF00FF00  // optional opaque green
```

## 7. Implementation Notes
- Use little endian encoding consistently
- VLQs are limited to ten bytes and the `u64` range
- Practical PPQ values: 480 or 960; absolute μs is common, ns is optional for high precision
- Maintaining per-bar or per-second indexes improves seek performance

---

## 8. Examples
### 8.1 Musical event after 240 ticks (OSC RAW)
```
Header = 0b0_0000000 (Domain = Musical, Kind = OSC)
ΔTime  = 0x81 0x70  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x18
  Data      = OSC packet for address "/light/flash", type tag ",i", argument 1
```

### 8.2 Absolute event after 150,000 μs (MIDI)
```
Header = 0b1_0000001 (Domain = Absolute, Kind = MIDI)
ΔTime  = 0x89 0x93 0x70  // 150,000 (μs)
Payload: [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

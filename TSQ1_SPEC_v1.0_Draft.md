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
| 0x0C | 2 | `u16` | Flags | Reserved for future use (0) |

---

## 2. Chunk Container
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: Event stream chunk (musical and absolute events)
- `"TMAP"`: Tempo map entries `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: Absolute anchors `(tick:u64, time_abs:u64)*` where `time_abs` uses `AbsUnit`

Implementations may introduce additional chunks; unknown chunk IDs must be skipped by
using the declared length.

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
- Validation guidelines for RAW: first byte `'/'` or `'#'`, maintain 4-byte alignment where applicable
- Emission time derives from `Header.Domain` and `ΔTime`; payload timetags remain untouched
- No fragmentation: one TSQ1 event encapsulates one OSC message or bundle

### 4.2 MIDI (`EventKind = 0x01`)
```
[Status:1][Data1:1][Data2:1]
```
- No running status; every MIDI event stores all three bytes

### 4.3 Meta (`EventKind = 0x02`)
```
[MetaType:1][Length:VLQ][Data:N]
```
- Mirrors SMF meta events (e.g., Tempo `0x51` uses 3-byte μs per quarter note)

### 4.4 SysEx (`EventKind = 0x03`)
```
[Length:VLQ][Data:N]  // excludes 0xF0/0xF7 framing
```

### 4.5 Custom (`EventKind = 0x7E`)
```
[TypeID:1][Length:VLQ][Data:N]
```
- Reserved for vendor-specific or experimental extensions

---

## 5. SYNC Chunk
```
"SYNC"[len:u32] { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical position (PPQ-based)
- `time_abs`: Absolute position expressed in `AbsUnit`
- Provides tick ↔ time conversion via linear interpolation between anchors
- `time_abs` expresses elapsed sequence time, not wall-clock timestamps

---

## 6. Implementation Notes
- Use little endian encoding consistently
- VLQ supports up to 10 bytes (u64 range)
- Practical PPQ values: 480 or 960; absolute μs is common, ns is optional for high precision
- Maintaining per-bar or per-second indexes improves seek performance

---

## 7. Examples
### 7.1 Musical event after 240 ticks (OSC RAW)
```
Header = 0b0_0000000 (Domain = Musical, Kind = OSC)
ΔTime  = 0x81 0x10  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x15
  Data      = "/light/flash\0\0\0,i\0\0\0\0\0\1"
```

### 7.2 Absolute event after 150,000 μs (MIDI)
```
Header = 0b1_0000001 (Domain = Absolute, Kind = MIDI)
ΔTime  = 0x83 0x58  // 150,000 (μs)
Payload: [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

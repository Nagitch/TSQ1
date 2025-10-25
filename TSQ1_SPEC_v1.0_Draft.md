
# TSQ1 File Format Specification — v1.0 Draft (EN)

**This draft reassigns EventKind as follows (2025-10-25):**  
- `0x00 = OSC` (first-class / canonical)
- `0x01 = MIDI`
- `0x02 = Meta`
- `0x03 = SysEx`
- `0x7E = Custom` (vendor / extensions)

> Rationale: Treat **OSC as a primary event** in TSQ1, while keeping
> seamless bridging between musical time and absolute time workflows.

---

## 0. Overview (unchanged essentials)
- Dual time domains: **Musical (Δticks, PPQ)** and **Absolute (Δtime, AbsUnit=μs/ns)**
- Little Endian, chunk-based (`"TRK "`, `"TMAP"`, `"SYNC"`)
- ΔTime encoded as SMF-style VLQ
- SYNC provides tick↔time anchors (linear interpolation by default)
- **Δtick is independent of AbsUnit** (it follows PPQ). Absolute uses AbsUnit.

---

## 1. Header (recap)
| Off | Size | Type | Name | Description |
|---:|---:|---|---|---|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | `1` (draft) |
| 0x06 | 2 | `u16` | PPQ | ticks per quarter |
| 0x08 | 1 | `u8` | AbsUnit | 0=Microseconds, 1=Nanoseconds |
| 0x09 | 1 | - | Reserved | 0 |
| 0x0A | 2 | `u16` | TrackCount | advisory |
| 0x0C | 2 | `u16` | Flags | reserved (0) |

---

## 2. Chunks
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: event stream
- `"TMAP"`: `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: `(tick:u64, time_abs:u64)*` (time_abs uses AbsUnit)

---

## 3. TRK Chunk (event stream)
### 3.1 Layout
```
[Header:1][ΔTime:VLQ][Payload...]
```
- `Header.bit7 = Domain` (0=Musical / 1=Absolute)
- `Header.bit6..0 = EventKind` (see below)
- `ΔTime`: VLQ (Musical→Δtick, Absolute→Δabs[μs/ns])

### 3.2 EventKind (**after reassignment**)
| Val | Const | Description |
|---:|---|---|
| **0x00** | **EK_OSC** | **OSC event (canonical)** |
| 0x01 | EK_MIDI | 3-byte MIDI message |
| 0x02 | EK_META | Meta event (SMF-like) |
| 0x03 | EK_SYSEX | SysEx (length-prefixed) |
| 0x7E | EK_CUSTOM | Custom / extensions |

---

## 4. Payload Specification

### 4.1 OSC (`EventKind=0x00`)
```
[OscFormat: u8][Length: VLQ][Data: N]
```
- **OscFormat**
  - `0x00 = RAW` (**canonical**): Byte-accurate OSC 1.0/1.1 datagram (`/path...` or `#bundle...`)
  - `0x01 = MSGPACK` (optional): `{"k":"msg"|"bun","p":"/foo","t":",ifs","a":[...],"ntp":u64?}`
  - `0x02 = CBOR` (optional): Same schema encoded in CBOR
  - `0x20–0x7F` reserved
- **Validation (RAW)**: First byte `'/'` or `'#'`, 4-byte alignment where applicable
- **Emission time**: Determined by `Header.Domain` + `ΔTime`. OSC payload timetag is preserved (player policy may honor it).
- **No fragmentation**: One event = one message/bundle

### 4.2 MIDI (`EventKind=0x01`)
```
[Status:1][Data1:1][Data2:1]
```
- No running status (always 3 bytes).

### 4.3 Meta (`EventKind=0x02`)
```
[MetaType:1][Length:VLQ][Data:N]
```
- e.g., Tempo (0x51): `Data(3) = μs per quarter`
- TimeSig (0x58), Key (0x59), End of Track (0x2F), etc.

### 4.4 SysEx (`EventKind=0x03`)
```
[Length:VLQ][Data:N]  // excludes 0xF0/0xF7 framing
```

### 4.5 Custom (`EventKind=0x7E`)
```
[TypeID:1][Length:VLQ][Data:N]
```
- For vendor/experimental extensions decoupled from the TSQ1 core.

---

## 5. SYNC Chunk (clarification)
```
"SYNC"[len:u32]  { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical (PPQ-based)
- `time_abs`: Absolute (AbsUnit = μs/ns)
- Purpose: tick↔time conversion via linear interpolation (bar-head or 1s anchors are typically enough)
- **Note**: `time_abs` is an **absolute elapsed time within the sequence**, **not a wall-clock timestamp**.

---

## 6. Implementation Notes
- LE everywhere; VLQ supports u64 (max 10 bytes).
- Suggested PPQ: 480 / 960. Absolute μs is practical; ns for high-end sync.
- Indexing: Keep per-bar or per-second offsets for fast seeks.

---

## 7. Examples

### 7.1 Musical: Raw OSC after 240 ticks
```
Header = 0b0_0000000 (Domain=Musical, Kind=OSC=0x00)
ΔTime  = 0x81 0x10  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x15
  Data      = "/light/flash\0\0\0,i\0\0\0\0\0\1"
```

### 7.2 Absolute: MIDI Note On after 150,000 μs
```
Header = 0b1_0000001 (Domain=Absolute, Kind=MIDI=0x01)
ΔTime  = 0x83 0x58  // 150,000 (μs)
Payload:  [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

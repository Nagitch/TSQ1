
# TSQ1 File Format Specification (v1.0, English Edition)

**TSQ1 (Time Sequence Quantized)** is a compact, portable binary format designed to store
**time-ordered event sequences** such as MIDI, OSC, lighting, and synchronization data.

It inherits the musical timing model of **SMF (Standard MIDI File)** using tick-based timing and tempo maps,
while extending it to support **absolute time** (in microseconds or nanoseconds).

---

## Overview

| Field | Description |
|--------|--------------|
| **Extension** | `.tsq` |
| **Magic** | `"TSQ1"` |
| **Endianness** | Little Endian (fixed) |
| **Purpose** | Storage of MIDI / OSC / lighting / timeline events |
| **Time Domains** | Musical (ticks, PPQ) and Absolute (μs/ns) |
| **Atomic Unit** | `Event` |
| **Structure** | Chunk-based, similar to SMF; multiple `"TRK "` chunks allowed |

---

## 1. File Layout

```
┌──────────────────────────────────────────────┐
│ Header (fixed size)                          │
├──────────────────────────────────────────────┤
│ Chunk[0] : "TRK " + event stream             │
│ Chunk[1] : "TRK " + event stream             │
│ ...                                          │
├──────────────────────────────────────────────┤
│ (optional) "TMAP" Tempo Map Chunk            │
│ (optional) "SYNC" Tick↔Time Mapping Chunk    │
└──────────────────────────────────────────────┘
```

---

## 2. Header Structure

| Offset | Size | Type | Name | Description |
|---------|------|------|------|-------------|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | Currently `1` |
| 0x06 | 2 | `u16` | PPQ | Ticks per quarter note |
| 0x08 | 1 | `u8` | AbsUnit | 0 = Microseconds, 1 = Nanoseconds |
| 0x09 | 1 | - | Reserved | Always 0 |
| 0x0A | 2 | `u16` | TrackCount | Expected track count (not strict) |
| 0x0C | 2 | `u16` | Flags | Reserved for future use |

**Total: 16 bytes**

---

## 3. Chunk Layout

| Field | Size | Type | Description |
|--------|------|------|-------------|
| ChunkID | 4 | `char[4]` | `"TRK "`, `"TMAP"`, `"SYNC"`, etc. |
| ChunkLength | 4 | `u32` | Length of following data in bytes |
| ChunkData | N | bytes | Chunk content |

---

## 4. `"TRK "` Chunk (Event Stream)

### 4.1 Overview
Each track consists of a **chronologically ordered list of events**.  
Each event = `Header (1B)` + `ΔTime (VLQ)` + `Payload`.

```
┌─────────────┬────────────────────┬────────────┐
│ Header (1B) │ ΔTime (VLQ, 1–10B) │ Payload (N)│
└─────────────┴────────────────────┴────────────┘
```

### 4.2 Header (1 byte)

| bit | Name | Meaning |
|------|------|---------|
| 7 | `Domain` | 0 = Musical (ticks), 1 = Absolute (μs/ns) |
| 6..0 | `EventKind` | Event type ID |

### 4.3 EventKind Table

| Kind | Const | Description | Notes |
|------|--------|-------------|-------|
| 0x00 | `EK_MIDI` | 3-byte MIDI message | Status + 2 data bytes |
| 0x01 | `EK_META` | Meta event | SMF-style structure |
| 0x02 | `EK_SYSEX` | System Exclusive data | Length-prefixed |
| 0x7E | `EK_CUSTOM` | Custom / extended events | For OSC or vendor data |

### 4.4 ΔTime (VLQ)
Variable Length Quantity (SMF-style).  
Each byte uses the MSB as continuation flag.

### 4.5 EventBody (Payload)

#### (1) MIDI (0x00)
| Field | Size | Description |
|--------|------|-------------|
| Status | 1 | 0x8n–0xEn |
| Data1 | 1 | data |
| Data2 | 1 | data |

*Running status is not used.*

#### (2) Meta (0x01)
| Field | Size | Description |
|--------|------|-------------|
| MetaType | 1 | e.g. 0x51 = Tempo, 0x58 = TimeSig |
| Length | VLQ | length of data |
| Data | N | meta payload |

Example: Tempo (0x51) → 3 bytes: `μs_per_quarter_note`.

#### (3) SysEx (0x02)
| Field | Size | Description |
|--------|------|-------------|
| Length | VLQ | length of SysEx data |
| Data | N | raw SysEx payload (without 0xF0/0xF7) |

#### (4) Custom (0x7E)
| Field | Size | Description |
|--------|------|-------------|
| TypeID | 1 | subtype |
| Length | VLQ | data length |
| Data | N | payload bytes |

##### Recommended TypeID Values

| TypeID | Name | Description |
|---------|------|-------------|
| 0x10 | `OSC_RAW` | Raw OSC packet (`/path`, `#bundle`, etc.) |
| 0x11 | `OSC_MSGPACK` | MessagePack object: `{"k":"msg","p":"/path","t":",ifs","a":[…]}` |
| 0x12 | `OSC_CBOR` | CBOR encoded equivalent |
| 0x7F | `VENDOR_DEFINED` | Vendor-specific extension |

---

## 5. `"TMAP"` Chunk (Tempo Map)

| Field | Type | Description |
|--------|------|-------------|
| tick | `u64` | Tick position |
| us_per_qn | `u32` | Microseconds per quarter note |

Multiple entries allowed.  
Provides quick tempo lookup without parsing meta events.

---

## 6. `"SYNC"` Chunk (Tick ↔ Time Mapping)

| Field | Type | Description |
|--------|------|-------------|
| tick | `u64` | Tick position |
| time_abs | `u64` | Absolute time (μs or ns) |

Defines anchor points for converting between musical and absolute domains.

---

## 7. Time Domains and Synchronization

| Item | Description |
|-------|-------------|
| **Musical Domain** | Expressed in Δticks; converted to real time via TMAP/meta events. |
| **Absolute Domain** | Expressed in Δtime (μs/ns) as defined by header. |
| **Synchronization** | SYNC chunk provides tick↔time anchors. |
| **Rule** | Each event belongs to only one domain; conversions are external. |

---

## 8. Reserved Extensions

| Range | Purpose |
|--------|----------|
| Chunk `"TEM2"` | Extended tempo curve (linear/exponential/Bezier) |
| EventKind `0x03–0x7D` | Reserved for future use |
| TypeID `0x20–0x6F` | Vendor / experimental types |
| Flags bitfield | Compression, versioning, etc. |

---

## 9. Compatibility and Safety

- **Backward compatible:** Unknown chunks must be skipped.  
- **Forward compatible:** Unknown event kinds can be skipped using length.  
- **Compression:** Per-chunk compression allowed (e.g., `"TRKZ"` = zstd/deflate).  
- **Integrity:** CRC32/64 checksum may be appended at end of file.

---

## 10. Implementation Notes

- VLQ: up to 10 bytes (u64 safe).  
- Running status: not used.  
- Numeric fields: Little Endian.  
- Recommended PPQ: 480 or 960.  
- Microsecond precision is sufficient for most cases; nanosecond for high-end sync.

---

© 2025 TSQ1 Working Group (Draft)

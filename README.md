# TSQ1 File Format Specification (v1.0)

**TSQ1 (Time Sequence Quantized)** is a general-purpose binary format for efficiently and portably storing sequences of discrete events arranged along a time axis, such as MIDI sequences and OSC events.

This specification inherits the advantages of **SMF (Standard MIDI File)** (musical time management via tempo maps) while providing a structure that also allows for **absolute time**.

---

## Overview

| Item | Description |
|------|------|
| **Extension** | `.tsq` |
| **Identifier (magic)** | `"TSQ1"` |
| **Endianness** | Fixed Little Endian |
| **Purpose** | Storing timeline sequences for MIDI, OSC, lighting, synchronization events, etc. |
| **Primary Time Axes** | Musical (ticks / PPQ) + Absolute (μs or ns) |
| **Minimum Unit** | Event (`Event`) |
| **Chunk Structure** | Similar to SMF. Can contain multiple tracks (`TRK `) |

---

See the detailed specification in [TSQ1_SPEC_v1.0_Draft.md](TSQ1_SPEC_v1.0_Draft.md) for English or [TSQ1_SPEC_v1.0_Draft_JA.md](TSQ1_SPEC_v1.0_Draft_JA.md) for 日本語.

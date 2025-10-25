
# TSQ1 File Format Specification (v1.0)

**TSQ1 (Time Sequence Quantized)** は、
MIDIシーケンスやOSCイベントなど、**時間軸上に離散的に並ぶイベント列**を
効率的かつ可搬性の高い形で格納するための汎用バイナリフォーマットである。

本仕様は **SMF (Standard MIDI File)** の利点（テンポマップによるミュージカルタイム管理）を継承しつつ、
**実時間 (Absolute time)** も併用できる構造を提供する。

---

## 概要

| 項目 | 内容 |
|------|------|
| **拡張子** | `.tsq` |
| **識別子 (magic)** | `"TSQ1"` |
| **エンディアン** | Little Endian 固定 |
| **目的** | MIDI・OSC・照明・同期イベントなどのタイムラインシーケンスの格納 |
| **主時間軸** | Musical（ticks / PPQ） + Absolute（μs または ns） |
| **最小単位** | イベント（`Event`） |
| **チャンク構造** | SMFに類似。複数トラック(`TRK `)を保持可能 |

---

## 1. ファイル全体構造

```
┌──────────────────────────────────────────────┐
│ Header (固定長ヘッダ)                       │
├──────────────────────────────────────────────┤
│ Chunk[0] : "TRK " + events                   │
│ Chunk[1] : "TRK " + events                   │
│ …                                            │
├──────────────────────────────────────────────┤
│ (optional) "TMAP" TempoMap Chunk             │
│ (optional) "SYNC" Tick↔Time mapping Chunk    │
└──────────────────────────────────────────────┘
```

---

## 2. ヘッダ構造

| オフセット | サイズ | 型 | 名称 | 説明 |
|-------------|--------|----|------|------|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` 固定 |
| 0x04 | 2 | `u16` | Version | 現行 `1` |
| 0x06 | 2 | `u16` | PPQ | Ticks per quarter note |
| 0x08 | 1 | `u8` | AbsUnit | 0=Microseconds, 1=Nanoseconds |
| 0x09 | 1 | - | Reserved | 常に `0` |
| 0x0A | 2 | `u16` | TrackCount | 想定トラック数（実際と異なってもよい） |
| 0x0C | 2 | `u16` | Flags | 将来拡張用（現行=0） |

**合計: 16 bytes**

---

## 3. チャンク構造

| フィールド | サイズ | 型 | 説明 |
|-------------|--------|----|------|
| ChunkID | 4 | `char[4]` | `"TRK "`, `"TMAP"`, `"SYNC"` など |
| ChunkLength | 4 | `u32` | 後続データのバイト長 |
| ChunkData | N | バイト列 | 内容（可変長） |

---

## 4. "TRK " チャンク（イベント列）

### 4.1 概要
1トラック内のイベント列は**時間順（昇順）**に並ぶ。  
各イベントは「ヘッダ1バイト + Δ時間(VLQ) + ペイロード」構成。

```
┌─────────────┬────────────────────┬────────────┐
│ Header (1B) │ ΔTime (VLQ, 1–10B) │ Payload (N)│
└─────────────┴────────────────────┴────────────┘
```

### 4.2 Header (1 byte)

| bit | 名称 | 意味 |
|------|------|------|
| 7 | `Domain` | 0=Musical（ticks）, 1=Absolute（μs/ns） |
| 6..0 | `EventKind` | 種別ID（下表参照） |

### 4.3 EventKind 一覧

| Kind | 定数 | 内容 | 備考 |
|------|-------|------|------|
| 0x00 | `EK_MIDI` | MIDIメッセージ（3バイト固定） | Status + 2 data bytes |
| 0x01 | `EK_META` | Metaイベント | SMFメタと同構造 |
| 0x02 | `EK_SYSEX` | SysExデータ | 長さ付き可変 |
| 0x7E | `EK_CUSTOM` | カスタムイベント | OSCや独自拡張 |

### 4.4 ΔTime（VLQ）
可変長整数 (Variable Length Quantity)。  
SMF同様、7bit単位で連結。上位ビット1が継続フラグ。

### 4.5 EventBody（Payload）
- MIDI: Status + 2 Data bytes
- Meta: MetaType(1) + Length(VLQ) + Data(N)
- SysEx: Length(VLQ) + Data(N)
- Custom: TypeID(1) + Length(VLQ) + Data(N)

---

## 5. "TMAP" チャンク（TempoMap）
| フィールド | 型 | 説明 |
|-------------|----|------|
| tick | `u64` | Tick位置 |
| us_per_qn | `u32` | 1拍あたりのμs |

---

## 6. "SYNC" チャンク（Tick↔Time対応表）
| フィールド | 型 | 説明 |
|-------------|----|------|
| tick | `u64` | Tick位置 |
| time_abs | `u64` | 絶対時間（μs or ns） |

---

## 7. 拡張仕様（予約）
| 範囲 | 用途 |
|-------|------|
| Chunk `"TEM2"` | 拡張テンポカーブ |
| EventKind `0x03–0x7D` | 予約 |
| TypeID `0x20–0x6F` | 独自拡張 |
| Flags bitfield | 圧縮やバージョン指定 |

---

© 2025 TSQ1 Working Group (Draft)


# TSQ1 File Format Specification — v1.0 Draft (JA)

**このドラフトでは EventKind を以下の通り再割当てします（2025-10-25 更新）**  
- `0x00 = OSC`（一級イベント／正準）
- `0x01 = MIDI`
- `0x02 = Meta`
- `0x03 = SysEx`
- `0x7E = Custom`（ベンダ拡張等）

> 目的：本フォーマットにおける **OSC を最重要イベント**として位置づけ、
> SMF的ワークフロー（Musical）と実時間同期（Absolute）の橋渡しをより明確にする。

---

## 0. 概要（変更なしの要点）
- 2軸時間をサポート：**Musical（Δtick, PPQ）** と **Absolute（Δtime, AbsUnit=μs/ns）**
- Little Endian 固定、チャンクベース（`"TRK "`, `"TMAP"`, `"SYNC"`）
- VLQ（SMF互換）でΔ時間を表現
- SYNC で tick↔time のアンカー列を提供（線形補間が標準）
- **Δtick は AbsUnit と無関係**（PPQに従う）。Absolute は AbsUnit（μs/ns）。

---

## 1. ヘッダ（再掲）
| オフセット | サイズ | 型 | 名称 | 説明 |
|---|---:|---|---|---|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | 現行 `1`（ドラフト） |
| 0x06 | 2 | `u16` | PPQ | ticks per quarter note |
| 0x08 | 1 | `u8` | AbsUnit | 0=Microseconds, 1=Nanoseconds |
| 0x09 | 1 | - | Reserved | 0 固定 |
| 0x0A | 2 | `u16` | TrackCount | 想定値（参考） |
| 0x0C | 2 | `u16` | Flags | 予約（0） |

---

## 2. チャンク
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: イベント列
- `"TMAP"`: `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: `(tick:u64, time_abs:u64)*`（time_abs は AbsUnit）

---

## 3. TRK チャンク（イベント列）
### 3.1 フォーマット
```
[Header:1] [ΔTime:VLQ] [Payload...]
```
- `Header.bit7 = Domain`（0=Musical / 1=Absolute）
- `Header.bit6..0 = EventKind`（下表）
- `ΔTime`: VLQ（Musical→Δtick, Absolute→Δabs[μs/ns]）

### 3.2 EventKind（**再割当後**）
| 値 | 定数 | 説明 |
|---:|---|---|
| **0x00** | **EK_OSC** | **OSC イベント（正準）** |
| 0x01 | EK_MIDI | MIDI 3バイトメッセージ |
| 0x02 | EK_META | メタイベント（SMF準拠） |
| 0x03 | EK_SYSEX | SysEx（長さ付き） |
| 0x7E | EK_CUSTOM | カスタム（拡張用） |

---

## 4. Payload 仕様

### 4.1 OSC（`EventKind=0x00`）
```
[OscFormat: u8][Length: VLQ][Data: N]
```
- **OscFormat**
  - `0x00 = RAW`（**正準**）: `/path...` または `#bundle...` を先頭とする **OSC 1.0/1.1 バイト列そのまま**
  - `0x01 = MSGPACK`（任意）: `{"k":"msg"|"bun","p":"/foo","t":",ifs","a":[...],"ntp":u64?}`
  - `0x02 = CBOR`（任意）: 上記と同スキーマを CBOR で
  - `0x20–0x7F` 予約
- **バリデーション推奨**（RAW）: 先頭が `'/'` or `'#'`、アライメント（4B）整合
- **発火時間**: `Header.Domain` と `ΔTime` に従って決定。OSC payload の timetag は**保存**（尊重はプレイヤポリシー）。
- **断片化禁止**: 1イベント = 1メッセージ/バンドル

### 4.2 MIDI（`EventKind=0x01`）
```
[Status:1][Data1:1][Data2:1]
```
- Running Status 非採用（常に3バイト）

### 4.3 Meta（`EventKind=0x02`）
```
[MetaType:1][Length:VLQ][Data:N]
```
- 例: テンポ（0x51）= `Data(3)` = μs/quarter
- TimeSig（0x58）、Key（0x59）、End of Track（0x2F）等

### 4.4 SysEx（`EventKind=0x03`）
```
[Length:VLQ][Data:N]  // 0xF0/0xF7 は含まない
```

### 4.5 Custom（`EventKind=0x7E`）
```
[TypeID:1][Length:VLQ][Data:N]
```
- ベンダ拡張／将来拡張用（TSQ1コアと独立）

---

## 5. SYNC チャンク（再掲：意味付け明確化）
```
"SYNC"[len:u32]  { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical（PPQ基準）
- `time_abs`: Absolute（AbsUnit = μs/ns）
- 用途: tick↔time を線形補間で相互変換（小節頭/秒単位などで十分高精度）
- **注意**: `time_abs` は **シーケンス内の絶対経過時間**。**システム日時ではない**。

---

## 6. 実装メモ
- すべて LE。VLQ は u64 対応（最大10B）。
- PPQ 推奨: 480 / 960。Absolute は μs が実用的、ns は高精度用途。
- インデックス: 小節頭 or 1秒ごとに TRK オフセットを併記すると高速シーク。

---

## 7. 例

### 7.1 Musical で 240tick 後に Raw OSC 送出
```
Header = 0b0_0000000 (Domain=Musical, Kind=OSC=0x00)
ΔTime  = 0x81 0x10  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x15
  Data      = "/light/flash\0\0\0,i\0\0\0\0\0\1"
```

### 7.2 Absolute で 150,000μs 後に MIDI Note On
```
Header = 0b1_0000001 (Domain=Absolute, Kind=MIDI=0x01)
ΔTime  = 0x83 0x58  // 150,000 (μs)
Payload:  [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

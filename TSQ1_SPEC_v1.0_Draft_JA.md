# TSQ1 File Format Specification — v1.0 Draft (JA)

TSQ1（Time Sequence Quantized）は、音楽系イベントと制御イベントを時間軸上に配置して格納するためのバイナリ形式です。
本書は履歴ではなく最新版の仕様そのものを記載し、実装者が参照しやすいように整理しています。

---

## 0. 概要
- 2軸時間：**Musical（Δtick, PPQ）** と **Absolute（Δtime, AbsUnit = μs/ns）**
- すべての多バイト値はリトルエンディアン
- Δ時間は SMF 互換の VLQ（可変長数量）で表現
- チャンクベース構造（`"TRK "`, `"TMAP"`, `"SYNC"` など）
- テンポマップと絶対時間アンカーで同期を提供

---

## 1. ヘッダ
| オフセット | サイズ | 型 | 名称 | 説明 |
|---:|---:|---|---|---|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | `1` |
| 0x06 | 2 | `u16` | PPQ | クォーターノートあたりの tick 数 |
| 0x08 | 1 | `u8` | AbsUnit | 0 = Microseconds, 1 = Nanoseconds |
| 0x09 | 1 | - | Reserved | 0 固定 |
| 0x0A | 2 | `u16` | TrackCount | トラック数の目安 |
| 0x0C | 2 | `u16` | Flags | 将来予約（0） |

---

## 2. チャンク構造
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: イベント列（Musical / Absolute 両対応）
- `"TMAP"`: テンポマップ `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: 絶対時間アンカー `(tick:u64, time_abs:u64)*` （`time_abs` は `AbsUnit` に従う）

未知のチャンク ID は `ChunkLength` 分をスキップして処理を継続します。

---

## 3. TRK チャンク
### 3.1 イベントレイアウト
```
[Header:1][ΔTime:VLQ][Payload...]
```
- `Header.bit7 = Domain`（`0` = Musical / `1` = Absolute）
- `Header.bit6..0 = EventKind`
- `ΔTime`: VLQ（Musical は Δtick、Absolute は AbsUnit による Δabs）

### 3.2 EventKind 割当
| 値 | 定数 | 説明 |
|---:|---|---|
| 0x00 | EK_OSC | OSC イベント（正準） |
| 0x01 | EK_MIDI | MIDI メッセージ（3 バイト固定） |
| 0x02 | EK_META | メタイベント（SMF 相当） |
| 0x03 | EK_SYSEX | SysEx ペイロード |
| 0x7E | EK_CUSTOM | カスタム／拡張用 |

---

## 4. ペイロード定義
### 4.1 OSC（`EventKind = 0x00`）
```
[OscFormat:u8][Length:VLQ][Data:N]
```
- `OscFormat`
  - `0x00 = RAW`: OSC 1.0/1.1 のバイト列（`/path...` または `#bundle...`）をそのまま格納
  - `0x01 = MSGPACK`: `{ "k": "msg"|"bun", "p": "/foo", "t": ",ifs", "a": [...], "ntp": u64? }`
  - `0x02 = CBOR`: 上記スキーマを CBOR で表現
  - `0x20–0x7F`: 予約領域
- RAW の検証ガイドライン：先頭バイト `'/'` または `'#'`、4 バイトアライメント維持
- 発火時間は `Header.Domain` と `ΔTime` から決定。ペイロード内の timetag は変更しない
- 断片化不可：1 TSQ1 イベント = 1 OSC メッセージ／バンドル

### 4.2 MIDI（`EventKind = 0x01`）
```
[Status:1][Data1:1][Data2:1]
```
- Running Status は使用せず、常に 3 バイトを格納

### 4.3 Meta（`EventKind = 0x02`）
```
[MetaType:1][Length:VLQ][Data:N]
```
- SMF メタイベント相当（例：Tempo `0x51` は μs/quarter の 3 バイト）

### 4.4 SysEx（`EventKind = 0x03`）
```
[Length:VLQ][Data:N]  // 0xF0/0xF7 を含めない
```

### 4.5 Custom（`EventKind = 0x7E`）
```
[TypeID:1][Length:VLQ][Data:N]
```
- ベンダ固有または実験的拡張用

---

## 5. SYNC チャンク
```
"SYNC"[len:u32] { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical（PPQ 基準）
- `time_abs`: `AbsUnit` に基づく絶対時間位置
- アンカー間を線形補間することで tick ↔ time の変換を提供
- `time_abs` はシーケンス内の経過時間であり、実世界の時計時刻ではない

---

## 6. 実装メモ
- リトルエンディアンを徹底
- VLQ は最大 10 バイト（u64 範囲）をサポート
- PPQ 推奨値：480 または 960。Absolute は μs が標準、ns は高精度用途
- 小節頭や 1 秒ごとのオフセット索引を保持するとシーク性能が向上

---

## 7. 例
### 7.1 Musical で 240 tick 後に OSC RAW を送出
```
Header = 0b0_0000000 (Domain = Musical, Kind = OSC)
ΔTime  = 0x81 0x10  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x15
  Data      = "/light/flash\0\0\0,i\0\0\0\0\0\1"
```

### 7.2 Absolute で 150,000 μs 後に MIDI Note On
```
Header = 0b1_0000001 (Domain = Absolute, Kind = MIDI)
ΔTime  = 0x83 0x58  // 150,000 (μs)
Payload: [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

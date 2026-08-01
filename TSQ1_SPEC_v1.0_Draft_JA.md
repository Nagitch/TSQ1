# TSQ1 ファイル形式仕様 — v1.0 ドラフト（日本語）

TSQ1（Time Sequence Quantized）は、音楽イベントと制御イベントを、音楽時間軸と絶対時間軸の両方で順序付けて格納するためのバイナリ形式です。本書は履歴的な注記を省き、最新の規則に集中できるように仕様の全体構造を記述します。

---

## 0. 概要
- 二種類の時間軸：**Musical（Δticks, PPQ）** と **Absolute（Δtime, AbsUnit = μs/ns）**
- すべての多バイト値はリトルエンディアン
- Δ時間は SMF 互換の VLQ（可変長数量）を使用
- チャンクベースのコンテナ（`"TRK "`, `"TMAP"`, `"SYNC"` など）
- テンポマップと絶対アンカーによる同期
- `"MARK"` チャンクによりロケーター（セクション、ドロップ等）のメタデータを任意で付与可能

---

## 1. ヘッダ
| オフセット | サイズ | 型 | 名称 | 説明 |
|---:|---:|---|---|---|
| 0x00 | 4 | `char[4]` | Magic | `"TSQ1"` |
| 0x04 | 2 | `u16` | Version | `1` |
| 0x06 | 2 | `u16` | PPQ | 四分音符あたりの tick 数 |
| 0x08 | 1 | `u8` | AbsUnit | 0 = Microseconds, 1 = Nanoseconds |
| 0x09 | 1 | - | Reserved | 0 固定 |
| 0x0A | 2 | `u16` | TrackCount | トラック数の目安（参考値） |
| 0x0C | 2 | `u16` | Flags | 下記のエンコードフラグ |

ヘッダフラグ：

- `bit 0`（`0x0001`）：SysEx ペイロードに先頭の `0xF0` または
  `0xF7` ステータスバイトを含む。
- `bit 1`（`0x0002`）：MIDI メッセージが正準の 3 バイト固定形式。
  本ドラフトの Writer はこのビットをセットする。
- その他のビットは予約。Reader は未知ビットを無視し、ファイルを
  再書き込みする場合は保持する。

---

## 2. チャンクコンテナ
```
[ChunkID:4][ChunkLength:u32][ChunkData...]
```
- `"TRK "`: イベントストリーム（Musical / Absolute の両ドメイン対応）
- `"TMAP"`: テンポマップエントリ群 `(tick:u64, us_per_qn:u32)*`
- `"SYNC"`: 絶対アンカー群 `(tick:u64, time_abs:u64)*`（`time_abs` は `AbsUnit` に従う）
- `"MARK"`: アレンジ用ロケーター／マーカー
- `"SMPF"`: 元の SMF SMPTE division `[fps:u8][subframes:u8]`

実装は追加チャンクを導入可能です。未知のチャンク ID は
`ChunkLength` に基づいてスキップしてください。可逆エディタは未知
チャンクをバイト単位で保持するべきです。

---

## 3. TRK チャンク
### 3.1 イベントレイアウト
```
[Header:1][ΔTime:VLQ][Payload...]
```
- `Header.bit7 = Domain`（`0` = Musical / `1` = Absolute）
- `Header.bit6..0 = EventKind`
- `ΔTime`: VLQ（Musical は Δtick、Absolute は AbsUnit による Δabs）

### 3.2 EventKind の割当
| 値 | 定数 | 説明 |
|---:|---|---|
| 0x00 | EK_OSC | OSC イベント（正準） |
| 0x01 | EK_MIDI | MIDI メッセージ（3 バイト固定） |
| 0x02 | EK_META | メタイベント（SMF 相当） |
| 0x03 | EK_SYSEX | SysEx ペイロード |
| 0x7E | EK_CUSTOM | カスタム／ベンダ拡張 |

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
  - `0x20–0x7F`: 予約
- RAW の検証：先頭バイトは `'/'` または `'#'`、packet 長は 4 バイト
  アラインメント
- 発火時間は `Header.Domain` と `ΔTime` から導出。ペイロード内 timetag は変更しない
- 断片化不可：1 TSQ1 イベント = 1 OSC メッセージ／バンドル

### 4.2 MIDI（`EventKind = 0x01`）
```
[Status:1][Data1:1][Data2:1]
```
- Running Status は使用せず、正準形式では常に 3 バイトを格納。
- Program Change（`0xCn`）と Channel Pressure（`0xDn`）の `Data2` は 0。
- 後方互換のため、ヘッダフラグ `0x0002` がない場合は `0xCn` / `0xDn`
  の 2 バイト形式を受理してよい。

### 4.3 Meta（`EventKind = 0x02`）
```
[MetaType:1][Length:VLQ][Data:N]
```
- SMF メタイベント相当（例：Tempo `0x51` は 四分音符当たり μs の 3 バイト）

### 4.4 SysEx（`EventKind = 0x03`）
```
[Length:VLQ][Data:N]
```
- ヘッダフラグ `0x0001` がある場合、`Data` の先頭に `0xF0` または
  `0xF7` を含め、`Length` にも算入する。正準 Writer は escape /
  continuation を可逆にするためこの形式を使う。
- フラグがない従来形式では framing を含まず、Reader は `0xF0` と
  みなす。

### 4.5 Custom（`EventKind = 0x7E`）
```
[TypeID:1][Length:VLQ][Data:N]
```
- ベンダ固有／実験的拡張用

---

## 5. タイミングチャンク

### 5.1 TMAP
```
"TMAP"[len:u32] { [tick:u64][us_per_qn:u32] }*
```
- `tick` は厳密な昇順。
- `us_per_qn` は SMF tempo metadata と同じ四分音符あたりの μs。

### 5.2 SYNC
```
"SYNC"[len:u32] { [tick:u64][time_abs:u64] }*
```
- `tick`: Musical（PPQ 基準）の位置
- `time_abs`: `AbsUnit` に基づく絶対位置
- アンカー間の線形補間により tick ↔ time 変換を提供
- `time_abs` はシーケンス経過時間であり、実世界の時計時刻ではない
- `tick` と `time_abs` はどちらも厳密な昇順。

### 5.3 SMPF
```
"SMPF"[len=2:u32][fps:u8][subframes:u8]
```
- SMF へ可逆出力するため、入力元の SMPTE division を保持。
- `fps` は `24`、`25`、`29`（29.97 drop-frame）、`30`。
- `subframes` は 0 より大きい値。
- SMPTE 入力のトラックdeltaは Absolute ドメインで表現し、`SMPF` が
  元の division の復元方法を記録する。

---

## 6. MARK チャンク（ロケーター）
```
"MARK"[len:u32] { [pos_kind:u8][pos:u64][name_len:VLQ][name:N][class:u8][marker_flags:u8][color_rgba:u32]? }*
```
- 目的：演奏や制御に影響しないナビゲーション用メタデータを格納（Ableton Live のロケーター等）。
- 位置指定：
  - `pos_kind`: `0 = Musical(tick)`, `1 = Absolute(time_abs; AbsUnit に従う)`
  - `pos`: `pos_kind=0` は開始からの PPQ tick、`pos_kind=1` は開始からの絶対時間
- ラベル：
  - `name_len`: UTF-8 `name` の VLQ 長
  - `name`: UTF-8 文字列（大文字／小文字や絵文字はそのまま保持）
- 区分（分類）：
  - `class` (u8) は SMF 互換の最小集合に絞り、非音楽系も含む時系列に対応：
    - `0x00 = Generic`
    - `0x20 = Cue`
    - `0x7F = Custom`
- 色（任意）：
  - `marker_flags.bit0 = 1` のとき `color_rgba` が存在する。その他の
    marker flag は予約であり 0 とする。
  - リトルエンディアン `u32` RGBA（`0xAARRGGBB`）。未対応の実装は無視してよい。
- 並び順：同一 `pos_kind` 内では `pos` 昇順。
- 一意性：同一位置の複数ロケーターを許容。
- 拡張性：未知の `class` は受理し、`Generic` と同様に扱う。

### 6.1 目的の補足
ロケーターは `TRK` イベントストリームから分離され、タイミングや再生への副作用を避けます。音楽に限定されない一般的な時系列シーケンスにおける人間可読なナビゲーション／相互運用性を提供します。

### 6.2 例
```
// Musical：任意位置に "Generic Marker" を配置
pos_kind = 0  // Musical
pos      = 1024  // 開始からの tick
name_len = VLQ(len("Generic Marker"))
name     = "Generic Marker"
class    = 0x00  // Generic
marker_flags = 0x00

// Absolute：90 秒に "Cue" を配置（色は任意）
pos_kind   = 1  // Absolute
pos        = 90_000_000  // AbsUnit=μs の例
name_len   = VLQ(len("Cue"))
name       = "Cue"
class      = 0x20  // Cue
marker_flags = 0x01
color_rgba = 0xFF00FF00  // 任意：不透明グリーン
```

---

## 7. 実装メモ
- リトルエンディアンを徹底
- VLQ は最大 10 バイト（u64 範囲）
- PPQ の実用値：480 / 960。Absolute は μs が一般的、ns は高精度用途
- 小節頭や 1 秒ごとのインデックスを持つとシーク性能が向上

---

## 8. 例
### 8.1 Musical で 240 tick 後に OSC RAW を送出
```
Header = 0b0_0000000 (Domain = Musical, Kind = OSC)
ΔTime  = 0x81 0x70  // 240
Payload:
  OscFormat = 0x00  // RAW
  Length    = 0x18
  Data      = address "/light/flash"、type tag ",i"、引数 1 の OSC packet
```

### 8.2 Absolute で 150,000 μs 後に MIDI Note On
```
Header = 0b1_0000001 (Domain = Absolute, Kind = MIDI)
ΔTime  = 0x89 0x93 0x70  // 150,000 (μs)
Payload: [0x90, 0x3C, 0x64]
```

---

© 2025 TSQ1 Working Group — v1.0 Draft

# BMS-IR Arena（接続の土台・段階 B1）

このフォークは、BMS-IR の対戦サービス Arena に接続できます。
仕様の全体は [SPEC_ARENA_FOUNDATION.md](SPEC_ARENA_FOUNDATION.md)、運用上の決定は [README](README.md) にあります。
ここでは、段階 B1（接続の土台）で実装した範囲と、手元での確かめ方をまとめます。

## 範囲

B1 に入るもの:

- プロファイルの設定（既定 OFF）、`hello`、`ping` と時計合わせ、`state` の送信。
- 20 秒の無通信での切断、再接続、拒否・`replaced` の扱い、最小の表示。

B1 に入らないもの（後の段階）:

- ルーム、推薦、曲の受け渡し、同時スタート、`live` / `final`、結果、棄権（B2 以降）。
- レート戦、`arena_status` の中身（レートなど）の表示。サーバーの項目名を仕様書で確認できていないためです。

ゲームプレイのスレッド、スコアの保存、リプレイ、IR の provider、G-BATTLE の区分には手を入れていません。

## 設定

「BMZ メニュー → IR」の「BMS-IR Arena」に、接続のスイッチと接続の状態を出します。

| 項目 | 値 |
|---|---|
| 保存先 | プロファイルの `profile.toml` の `[arena]` の `enabled` |
| 既定 | `false`（OFF）。OFF の間は何も接続しません |
| 認証 | IR 設定の BMS-IR のログイン（プレイヤー ID とゲームトークン）を読むだけです。Arena 用の保存場所はありません |
| ルール | プロファイルのルール。`Lr2Oraja` は `lr2`、`Beatoraja` は `oraja`。`Dx` は Arena では使えません |

設定を変える、プロファイルを切り替える、BMS-IR にログインし直す、のどれでも、接続は自動で作り直します。

## 接続

- 接続先は `fork_policy::ensure_arena_url_allowed` が確認します。BMS-IR のホスト以外へは接続しません。
  本番は `wss://www.bms-ir.org/new/arena/ws/client` です。ビルド時の `BMZ_BMS_IR_BASE_URL` で
  通常の IR を手元のサーバーへ向けたビルドでは、同じホストの `ws://…/new/arena/ws/client` になります。
  Arena 専用の上書きはありません。
- `hello` の `passmd5` は、ゲームトークンを UTF-8 にした MD5（小文字 16 進）です。トークンそのものは送りません。
- `client_flavor` は `bmz-player`、`client_version` は `bmz-bmsir-0.1.0`（Arena のプロトコルに関わる変更のたびに上げます）、
  `build_hash` はビルドのコミット（`BMZ_BUILD_COMMIT`）です。
- `server_cpu_v1` は送りません。

| 場面 | 動き |
|---|---|
| `hello_ok` | 接続済みにして、`state` を送り、すぐに `ping` を送る |
| `ping` | 5 秒ごと。`pong` の往復の中間から、サーバー時刻との差を求める |
| 20 秒の無通信 | 切断して再接続（`hello_ok` の前も同じ） |
| 切断 | 5 秒後に再接続。最初の切断から 30 秒を過ぎたら 30 秒おきに再接続（サーバーが試合を保つのは 30 秒） |
| `protocol_mismatch` など版の拒否 | 再接続しない。「更新が必要」と表示 |
| `authentication_failed` | 再接続しない。IR 設定の確認を表示 |
| `client_flavor_conflict` | 再接続しない。理由を表示 |
| `replaced` | 自動では再接続しない。「再接続」ボタンで手動の再接続 |
| そのほかの `error` | 接続を保ったまま、`code` を 1 度だけ表示してログに残す |
| 知らないメッセージ | 無視してログに残す |

`state` は画面が変わったとき（選曲 / 決定 / プレイ / 結果）と `hello_ok` の直後に送ります。
練習・AUTOPLAY・リプレイ・コースは、B1 では `play` として送ります（後の段階で区別します）。
`ready` は部屋がない B1 では常に `false` です。

## 構成

| ファイル | 責務 |
|---|---|
| `crates/bmz-player/src/arena/protocol.rs` | メッセージの型と JSON の読み書き、`passmd5` |
| `crates/bmz-player/src/arena/session.rs` | 入出力を持たない状態機械（時刻は呼び出し側から渡す） |
| `crates/bmz-player/src/arena/connection.rs` | 専用スレッド（tokio の current-thread + tokio-tungstenite）と window スレッド側のハンドル |
| `crates/bmz-player/src/arena/mod.rs` | 接続先 URL、`hello` の組み立て、起動 |
| `crates/bmz-player/src/app/integrations.rs` | 設定・ログインの変化で接続を作り直し、画面を伝える |
| `crates/bmz-player/src/ui/profile_panel/sections/arena.rs` | 設定のスイッチと接続状態の表示 |

## 手元で確かめる

1. 設定が OFF のままで、BMZ を起動して選曲まで進め、Arena のサーバーへ何もつながらないこと（ログに接続の記録がないこと）。
2. BMS-IR にログインしていない状態で ON にすると、「BMS-IR にログインしていません」と出ること。
3. BMS-IR にログインして ON にし、サーバー側が BMZ を許可していれば「接続済み」になること。
   許可前なら「拒否されました。このビルドは更新が必要です」になり、再接続を繰り返さないこと。
4. 選曲 → 曲を開始 → 結果、と進んでも、プレイとスコア保存が普通に動くこと。
5. OFF に戻すと、接続が切れて「オフ」になること。

## 段階 B2-1: ルームと推薦

B2 のうち、ルームの作成・入室、準備 OK、推薦、`chart` と `chart_check` までを入れました。
曲の読み込み・`option_ready`・`prepare`・`play_ready`・`start_at` までの待機は B2-2（別の PR）です。

### 画面

選曲画面の右上に、接続済みのときだけ「Arena」のパネル（egui、既定は折りたたみ）を出します。
プレイ中・決定画面・結果画面には出ません。パネルはゲームのキー入力を取りません（マウス操作）。

| 操作 | 送るもの |
|---|---|
| ルームを作る | `room_entry`（`match_mode="private"`、`series_format="single"`） |
| コードで入る | `room_entry`（`match_mode="private"`、`room_code`、あれば `room_password`） |
| 準備 OK | `room_ready`（`ready`）と `state`（選曲画面で、ルームにいて、準備 OK のときだけ `ready=true`） |
| 選択中の譜面を推薦 | `chart_nominate`（`match_id`、`chart_hash`=MD5）。推薦の時間中に 1 回だけ |
| スキップ | `chart_nomination_skip`（`match_id`） |
| （自動）候補の確認 | `chart_check`。手元の曲 DB の MD5 で引く。7KEY の譜面だけ `available=true` |

試合の進み具合（`match_reserved` → `fill_started` → `nomination_started` → `nominations_revealed` → `chart`）と
残り秒数（サーバー時刻に合わせた値）、取り消し・外された理由を出します。

### 状態の扱い

- 試合 ID と段階で古い応答を捨てます。`chart_check` の答えは、その時点の候補（`match_id` と MD5）と同じときだけ送ります。
- `match_cancelled` / `match_released` で試合の状態を消し、準備 OK を外します（ルームは残ります）。
- 切断・`replaced`・拒否でルームと試合の状態を消します。再接続後はサーバーが写しを送り直します。
  再接続後に自動でルームへ戻る処理はありません（B3 で `active_match_id` と合わせて決めます）。
- `hello_ok` の `active_match_id` が空なら、手元の試合の状態を消します。
- 試合中は新しいルームへ入れません。

### 仕様書に項目名が無いため、送っていない・読んでいないもの

推測で埋めていません。サーバーの実際の項目名を教えてもらえれば足します。

| 場面 | 不明な点 |
|---|---|
| `room_entry`（作る） | 土台の固定設定（公開、勝ち負け=EX SCORE、ゲージ=自由、選曲範囲=`free`、推薦=全員、LN、ルール、キー数=7KEY）の項目名と値。今は `match_mode` と `series_format` だけを送ります |
| ルームから抜ける | メッセージ名（パネルに「抜ける」ボタンがありません） |
| `room_status` | 参加者・準備の状態の項目名。`room_code` だけ読みます |
| `nomination_status` | 自分の推薦の有無・締め切りの項目名。読んでいません（`nomination_started.deadline` と `nomination_accepted` で代用） |
| `nomination_started` | `deadline` を読みます（項目名が仕様書にあるのは `fill_started` だけで、同じ名前と仮定）。違えば残り秒数が出ないだけです |
| `players_updated` | 項目名。読んでいません（人数は `fill_started` のみ） |
| `nominations_revealed` | 候補と選ばれた候補の項目名。読んでいません |
| `ln_mode` | JSON の型（文字列か数）。どちらでも文字列にして保持します |
| `match_reserved.rules` / `chart.rules` | 中身。読んでいません |
| `chart` | `chart.md5` / `title` / `artist` / `totalnotes` は仕様書にあります。`match_id` を含むかは不明なので、あれば現在の試合と照合し、無ければ現在の試合のものとして扱います |

## 練習・AUTOPLAY・リプレイ・コースの `state` と `ready`（案）

決定 5 の案です（B2-1 では実装していません。`state` は今のまま `play` です）。

| BMZ の場面 | `state` | 理由 |
|---|---|---|
| 通常の手動プレイ（NORMAL） | `play` | 仕様書 3.3 |
| G-BATTLE | `play` | 仕様書 3.3（NORMAL と G-BATTLE は `play`） |
| 結果（単曲） | `result` | 同上 |
| コース・段位（プレイと結果） | `course` | 仕様書 3.3 |
| PRACTICE | `practice` | 同上 |
| AUTOPLAY・AUTOPLAY BATTLE | `autoplay` | 同上 |
| リプレイ再生 | `replay` | 同上 |
| 決定画面 | `decide` | 同上 |

`ready` は仕様書 3.3 のとおり、`select` のときと、`result` で通常の IR 送信が終わったときだけ `true` にできます。
B2-1 は `select` のみ実装済みです。`result` の側は、IR 送信の完了を知る入口が要るため B3（結果の扱い）で足す案です。
`practice` / `autoplay` / `replay` / `course` の間は `ready=false` 固定で、試合の `match_reserved` が来たら
「通常のプレイ中なら終わるまで待つ」（`finish_current_song`）扱いにして選曲に戻った時点で再び `ready` を許す案です。
これらの区別の実装には、`AppSceneKind` が Play のまま練習・AUTOPLAY・リプレイ・コースを含むため、
play の種類を app 側から取り出す入口（`app/play_flow`）が要ります。

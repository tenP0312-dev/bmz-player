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
曲の読み込み・`option_ready`・`prepare`・`play_ready`・`start_at` までの待機は、次の「段階 B2-2」にまとめています。

### 画面

選曲画面の右上に、接続済みのときだけ「Arena」のパネル（egui、既定は折りたたみ）を出します。
プレイ中・決定画面・結果画面には出ません。パネルはゲームのキー入力を取りません（マウス操作）。

| 操作 | 送るもの |
|---|---|
| ルームを作る | `room_entry`（`match_mode="private"` と土台の固定設定の全項目） |
| ルームから抜ける | `queue_cancel` |
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

### サーバーの項目名（`ARENA_SERVER_MESSAGES.md` に合わせた部分）

項目名と型は [ARENA_SERVER_MESSAGES.md](ARENA_SERVER_MESSAGES.md) を正とします。

| 場面 | BMZ の扱い |
|---|---|
| `room_entry`（作る） | 全項目を送る：`score_rule=exscore`、`forced_gauge=free`、`chart_scope=free`、`ruleset_profile`（`hello` と同じ）、`ln_mode=LN`、`nomination_policy=all`、`nomination_seconds=60`、`option_seconds=10`、`intermission_seconds=0`、`series_format=single`、`first_to_wins=2`、`spectator_public=false`、`force_host_option=false`、`allowed_play_modes=[7]`。`public` / `seconds` / `password` という名前の項目はサーバーにありません |
| `room_entry`（入る） | `room_code` と、あれば `room_password` だけ（ルールは部屋のものになる） |
| `room_settings`（ホスト） | 省略した時間・形式の項目はサーバーが既定に戻すため、`ln_mode` 以外の全項目を送る。パネルのボタン「ルームの設定を送り直す」（ホストのみ） |
| ルームから抜ける | `queue_cancel`。試合前は即時に部屋を消す。試合中は `match_released` / `forfeit_accepted` まで待つ |
| `room_status` | `players[]`（`player_id`、`name`、`host`、`participating`、`pending`、`ready`）、`rules.room_host_id`、`rules.ln_mode`。自分の準備 OK は手元の要求を優先し、サーバーの古い `room_status` で上書きしない |
| `nomination_started` / `nomination_status` | `deadline`、`can_nominate`、`required_count`、`submitted_count`、`retry_reason`、`players[].submitted`。「推薦済み」はサーバーの `submitted` に従う。`retry_reason` があれば再推薦を促す |
| `players_updated` | `player_ids` の数を人数にする（現在の試合の `match_id` のときだけ） |
| `nominations_revealed` | `nominations[]`、`selected_player_id`、`chart.md5`、`reroll_count` |
| `ln_mode` | 文字列（`LN` / `CN` / `HCN`、大文字にそろえる） |
| `ready` | `state` が `select` のときだけ有効（サーバーは `state` のたびに上書きする）。BMZ は `select` 以外では `ready=false` を送る |

まだ読んでいない・使っていないもの：`match_reserved.rules`、`chart.rules`、`chat_history`、`result`（B3）。
`chat_send` の応答や `chart` の追加キーは資料でも「未確認」です。

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

## 段階 B2-2: オプションの確定と読み込み・開始

B2 の残りです。`option_select` から `start_at` まで、自分で曲を始めて READY で待つところまでを入れました。
`live` / `final` / `forfeit` の送信、ESC 等をふさぐ処理、結果の表示、再接続後にルームへ戻る処理は B3 です。

### 流れ

| サーバー | BMZ の動き |
|---|---|
| `option_select` | パネルに「READY（今のオプションで送る）」を出す。選曲画面のオプション（1P / 2P の配置、FLIP）を `play_option` にして、変わるたびに保っておく |
| （READY を押す、または締め切りの 0.25 秒前） | `option_ready`（`play_option`、`play_mode`、`ln_mode`）。1 試合に 1 回 |
| `prepare` | 確定した `play_option` と `random_seed` で、自分で曲を始める（下記）。始められないときは `forfeit` |
| （読み込み完了） | `play_ready`（`totalnotes` は読み込んだセッションの採点対象ノーツ数）。READY の演出を待たずに送る |
| `start` | READY の最後で止まり、サーバー時刻が `start_at` に着いたフレームで音声の時計を始める |

`play_option` は `1P + 10 × 2P + 100 × FLIP`（`options.rs`）。H-RANDOM、ALL-SCR、RANDOM-EX、S-RANDOM-EX、F-RANDOM、MF-RANDOM は NORMAL、
BATTLE と BATTLE AS は Off として送ります。SP（7KEY）は 1 の位だけです。

### 曲を始める入口（`app/arena_flow.rs`）

`prepare` を受けたフレームで、選曲または結果画面なら `start_chart_with_options` を呼びます。
リトライ・直接起動と同じ入口なので、プレロード、READY の演出、音声の開始、`gameplay` スレッドへの引き渡しは変えていません。
決定画面は通りません。

Arena の 1 曲で当てはめるものは、次の 2 か所だけに置きます。選曲画面のオプションやプロファイルは書き換えないので、
終わったときに戻す処理はありません（`arena_play` を捨てるだけです）。

1. **`PlayStartOptions`**（`arena/options.rs` の `apply_arena_overrides`）：NORMAL セッション、AUTOPLAY・リプレイ・G-BATTLE なし、
   アシスト・キー数変換・Random Trainer・LN の変更なし、配置と FLIP は `play_option`、RANDOM の側の seed はサーバーの `random_seed`。
   ゲージ、HS、レーンカバー、表示は利用者の設定のままです。
2. **ルールと LN の方針**（`WinitApp::effective_ln_policy_and_rule`）：これまでコースだけが上書きしていた
   `(ln_policy_setting, rule_mode)` の 4 か所の重複を 1 つの関数にまとめ、コース → Arena → プロファイルの順にしました。
   Arena は `ruleset_profile` から `Lr2Oraja` / `Beatoraja`、LN は `ForceLn` です。スコアの identity はこの値から作られます。

`maybe_start_ready_phase`（`play_loop_flow.rs`）に 2 つの入口を足しています。

- 関数の先頭で `arena_report_loaded`：読み込みが終わったら `play_ready` を 1 回報告する（サーバーの `start_at` は最後の `play_ready` の 5 秒後なので、READY の演出の後ではなく先に送る）。
- 音声の開始（`running.start`）の直前で `arena_start_gate`：Arena の曲なら、`start` が来て、サーバー時刻（時計合わせの差を足した現在時刻）が `start_at` に着くまで待つ。
  待っている間は `practice_chart_zero_time` を消費しません。

そのほか、`match_reserved` から試合の終わりまでは、通常の曲の開始（決定画面へ進む入口 `begin_decide_for_chart`）をふさぎ、トーストを出します
（`block_next_selection`）。

### 限界と未確認

- `start_at` の精度は 1 フレーム分です（待ちの判定を描画フレームごとに行うため）。実測は B4 で行います。
- `start_at` に合わせるのは**曲の 0 秒**です（オペレーター決定 1、Arena oraja と同じ）。BMZ はスキンの `playstart` の分だけ
  曲の 0 秒より前に音声の時計を始めるので、READY を抜ける条件は次の3つがそろったときです：
  READY の演出が終わっている、`play_ready` を送信済み、サーバー時計 ≥ `start_at − playstart`（`arena::launch::may_leave_ready`）。
  READY の演出が `start_at` に間に合わなかったときは、終わった時点で始めます（その分、0 秒が遅れる）。
- `play_ready.totalnotes` は、読み込んだ譜面をその部屋の `ln_mode` で数えた値です（オペレーター決定 2）。LN の部屋は LN 1 本を 1、
  CN / HCN の部屋は始点と終点で 2 と数えます（`arena::options::arena_total_notes`）。`chart_check` の `totalnotes` はライブラリの値
  （LN 1 本 = 1）で、サーバーが照合するのは LN の部屋だけなので、そのままにしています。
- CN / HCN の部屋（`ln_mode` が `LN` 以外）と、7KEY 以外の試合は始められず、`forfeit` します。
- `prepare` の `rules.ruleset_profile` が無い・知らない値のときも始めません（推測で補いません）。
- 試合の途中で ESC 等で抜ける操作は、READY 中のものだけ `forfeit` に結びつけています。プレイ中の ESC は B3 です。
- 練習・AUTOPLAY・リプレイ・コースの `state` の区別と、`result` での `ready` は B3 です。

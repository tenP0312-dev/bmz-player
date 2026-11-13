# BMS-IR Arena（B1〜B4 の全体）

このフォークは、BMS-IR の対戦サービス Arena に、プライベートルームの 1 曲戦として参加できます。
仕様の全体は [SPEC_ARENA_FOUNDATION.md](SPEC_ARENA_FOUNDATION.md)、運用上の決定は [README](README.md)、
サーバーの実際のメッセージは [ARENA_SERVER_MESSAGES.md](ARENA_SERVER_MESSAGES.md)（仕様書と食い違うときはこちらが正）、
つないだ確認の手順は [ARENA_MANUAL_TEST.md](ARENA_MANUAL_TEST.md) にあります。

## 今できること

| 段階 | できること |
|---|---|
| B1 接続 | プロファイルごとの ON/OFF（既定 OFF）、`hello`、`ping` と時計合わせ、`state` の送信、切断後の再接続、版の拒否・`replaced` の扱い、レートと対戦数の表示 |
| B2-1 ルームと推薦 | ルームを作る・コードで入る・抜ける、準備 OK、ホストの設定の送り直し、譜面の推薦とスキップ、決まった譜面の `chart_check`（7KEY のみ） |
| B2-2 開始 | オプション（1P / 2P の配置、FLIP）の確定、`prepare` を受けて自分で曲を読み込み、`play_ready`、`start_at` に曲の 0 秒を合わせた同時スタート |
| B3 試合の後半 | `live`（1 秒に 1 回まで）と `final`、プレイ中に抜ける操作のロック、棄権、結果・取り消し・外された・ルーム解散の表示、再接続後の復帰、`state` の区別（`play` / `course` / `practice` / `autoplay` / `replay`） |
| B4 仕上げ | 取り消し理由 9 件の 6 言語の文、再接続と古いメッセージまわりの直し（[B4 の見直し](#b4-の見直し)）、確認の手順 |

ゲームプレイのスレッド、スコアの保存、リプレイ、IR の provider、G-BATTLE の区分には手を入れていません。
Arena の 1 曲で当てはめるのは、その曲の `PlayStartOptions` とルール / LN の方針だけで、プロファイルは書き換えません。

## できないこと

- **CN / HCN の部屋**：`ln_mode` が `LN` 以外の部屋の試合は始められず、`forfeit` します（ノーツ数の数え方だけ合わせてあります）。
- **レート戦**：`queue_entry`、レートの計算・順位。表示するのは `arena_status.player` の `rating`（整数）と `matches_played` だけです。
- **5/9/10/14KEY**：7KEY だけです。それ以外の譜面は `chart_check` で `available=false`、試合も始めません。
- 作れる部屋は、土台の固定設定（`exscore`、ゲージ自由、`chart_scope=free`、`LN`、`nomination_policy=all`、1 曲戦）だけです。
  シリーズ戦（`all_picks` / `first_to` / `bo2`）、`rotate` / `host` の推薦、`custom` の譜面範囲、パスワードの変更は扱いません。
- 観戦、観戦者の表示、公開ルームの一覧、ロビーのチャット、試合中のチャット（`chat_history` / `chat_send`）。
- ホストの操作のうち、キック・解散・ホストの移譲・選曲者の指定（BMZ からは送りません。受け取ったときの表示だけ）。
- 強制終了の投票（`force_end_vote`）、`result.series`、`return_to_select_at`。
- Arena oraja との対戦（BMZ 同士だけ）。`server_cpu_v1` も送りません。
- BMZ を終了して開き直したあとの試合への復帰（曲に戻れず、サーバーが切断として棄権にします）。
- `forfeit` の `reason`（資料に項目が無いので送りません）。
- `chart` の追加キー、`chat_*` の応答の形は資料でも未確認で、読んでいません。
- 本番での確認。BMS-IR 側の許可リストへの追加（オペレーターが行う）と、実機・実サーバーの確認はこれからです（[手順](ARENA_MANUAL_TEST.md)）。

## 設定

「BMZ メニュー → IR」の「BMS-IR Arena」に、接続のスイッチと接続の状態を出します。選曲画面と結果画面の「Arena」の窓は、接続済みのときだけ出ます。

| 項目 | 値 |
|---|---|
| 保存先 | プロファイルの `profile.toml` の `[arena]` の `enabled` |
| 既定 | `false`（OFF）。OFF の間は何も接続しません |
| 認証 | IR 設定の BMS-IR のログイン（プレイヤー ID とゲームトークン）を読むだけです。Arena 用の保存場所はありません |
| ルール | プロファイルのルール。`Lr2Oraja` は `lr2`、`Beatoraja` は `oraja`。`Dx` は Arena では使えません |
| 接続先 | BMS-IR のホストだけ（下の「接続」）。ビルド時の `BMZ_BMS_IR_BASE_URL` で手元のサーバーへ向けられます |
| 画面の言語 | BMZ の言語設定（日本語、English、한국어、简体中文、繁體中文（台湾 / 香港））に従います |

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

手順と合格の基準は [ARENA_MANUAL_TEST.md](ARENA_MANUAL_TEST.md) に 1 本にまとめています。

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
  再接続後のルームへの復帰は B3 で入れました（下の「段階 B3」）。
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

## 段階 B2-2: オプションの確定と読み込み・開始

B2 の残りです。`option_select` から `start_at` まで、自分で曲を始めて READY で待つところまでを入れました。
`live` / `final` / `forfeit` の送信、ESC 等をふさぐ処理、結果の表示、再接続後にルームへ戻る処理は B3（次の節）です。

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

- `start_at` の精度は 1 フレーム分です（待ちの判定を描画フレームごとに行うため）。実測は [手順](ARENA_MANUAL_TEST.md)の 5-7 で行います。
- `start_at` に合わせるのは**曲の 0 秒**です（オペレーター決定 1、Arena oraja と同じ）。BMZ はスキンの `playstart` の分だけ
  曲の 0 秒より前に音声の時計を始めるので、READY を抜ける条件は次の3つがそろったときです：
  READY の演出が終わっている、`play_ready` を送信済み、サーバー時計 ≥ `start_at − playstart`（`arena::launch::may_leave_ready`）。
  READY の演出が `start_at` に間に合わなかったときは、終わった時点で始めます（その分、0 秒が遅れる）。
- `play_ready.totalnotes` は、読み込んだ譜面をその部屋の `ln_mode` で数えた値です（オペレーター決定 2）。LN の部屋は LN 1 本を 1、
  CN / HCN の部屋は始点と終点で 2 と数えます（`arena::options::arena_total_notes`）。`chart_check` の `totalnotes` はライブラリの値
  （LN 1 本 = 1）で、サーバーが照合するのは LN の部屋だけなので、そのままにしています。
- CN / HCN の部屋（`ln_mode` が `LN` 以外）と、7KEY 以外の試合は始められず、`forfeit` します。
- `prepare` の `rules.ruleset_profile` が無い・知らない値のときも始めません（推測で補いません）。
- 試合の途中で ESC 等で抜ける操作は、READY 中のものだけ `forfeit` に結びつけています。プレイ中の ESC は B3 でふさぎました。

## 段階 B3: 試合の後半

`live` / `final`、送り直し、ロック、`forfeit`、結果・取り消し・棄権の表示、再接続後の復帰、`state` の区別を入れました。
項目名は [ARENA_SERVER_MESSAGES.md](ARENA_SERVER_MESSAGES.md) を正とします。

### `live` と `final`

| 項目 | 内容 |
|---|---|
| 値の出どころ | プレイ中は window スレッドが持つ描画スナップショット（`ex_score`、`past_notes`、判定の数、`max_combo`）。`final` は保存済みの `FinishedPlaySession.result`（クリア種別と採点）。ゲームプレイのスレッドには手を入れていません |
| `live` | 音声の時計が始まってから、`seq` を 1 から増やしながら 1 秒に 1 回まで。`final` の後は送らない |
| 保護 | 値は減らさない。`processed_notes` は `play_ready` の `totalnotes` まで。EX は処理数の 2 倍まで、新しいノーツ 1 つにつき 2 まで。`max_combo` は処理数まで（サーバーが `invalid_live` で捨てる条件。`arena/session.rs` の `PlayValues::clamped`） |
| `minbp` / `combo_break` | `minbp` は BAD + POOR + 空 POOR、`combo_break` は BAD + POOR（サーバーはこちらを BP に使う） |
| `play_option` / `play_mode` / `ln_mode` | `prepare` で確定した値（途中で変えない） |
| `final.state` | 最後まで遊んでクリア種別が 2 以上なら `complete`（`processed_notes` はノーツ数そのもの）。FAILED 状態で終わった、またはゲージが足りず FAILED のクリアで終わったら `hard_fail` と `clear_type=1`（サーバーはこの組み合わせ以外を拒否する） |
| `clear_type` | BMZ の `ClearType` の番号が beatoraja の番号と同じなので、そのまま送る |
| 結果の保存が失敗したとき | 本当のことが言えないので、`final` を作らず `forfeit` する |

### 送り直しと再接続

- `final` は `result`（または取り消し・棄権の確定）が来るまで覚えておく。接続が切れている間に曲が終わっても、再接続して `hello_ok` の
  `active_match_id` が同じ試合なら、`final`、`play_ready`（読み込み後で開始前のとき）、`state` の順に送る（仕様 3.6）。
  サーバーが受け取り済みなら重複した `seq` は `invalid_final` で捨てられるだけで、試合は終わらない。
- 切れたときに残す試合は、`start` を受けた後・プレイ中・`final` 後、および `play_ready` 送信済みの読み込み。
  それ以外はサーバーが `match_reserved` ほかの写しを送り直す。残した試合に写し（`match_reserved`、`chart`、`prepare`、`option_select`、`start`）が
  もう一度来ても、曲は始め直さない（`prepare` が来ても `app/arena_flow.rs` が同じ試合の曲を起動しない）。
- `hello_ok` の `active_match_id` が空、または別の試合なら、手元の試合を捨てる。
- ルーム：`arena_status.player.queue.room_code` を読み、サーバーにルームが残っていれば同じコードで画面に戻す。
  参加者の一覧は次の `room_status` で埋まる（再接続の直後には届かない。資料に再同期のメッセージがない）。サーバーに無ければ手元も消す。
- `match_resume`（`countdown` / `playing`）は、残した試合の `start_at` を更新するだけ。BMZ を終了して開き直した場合は、曲に戻れない
  （サーバーが切断として棄権にする）。

### ふさぐ操作と棄権

- 曲の時計が始まった後は、ESC、E2+E3、E1+E2 長押しなど、プレイを抜ける操作をすべてふさぎ、トーストを出す（`stop_play_like_escape` の入口）。
  READY までは抜けられる（抜けると `forfeit`）。
- 選曲画面のパネルの「棄権する」で `forfeit`（`final` を作る前まで）。BMZ を閉じたときも、試合中なら `forfeit` を送ってから閉じる。
- 起こらないはずの「プレイ中に `final` なしで選曲に戻る」は、`state` の前に `forfeit` を送る。
  なお、資料では `forfeit` に `reason` 項目が無いので送っていない（仕様書の `reason` は付けない）。
- 試合中（`match_reserved` から終わりまで）は、決定画面・練習・AUTOPLAY・リプレイ・コース・結果画面の再挑戦の開始をふさぎ、トーストを出す
  （`arena_blocks_normal_start`）。コースの途中の次の曲への進み方には入れていない（試合の前に始めたコースを壊さないため）。

### `state` と `ready`

| BMZ の場面 | `state` |
|---|---|
| 選曲 / 決定 | `select` / `decide` |
| 通常の手動プレイ（G-BATTLE も） | `play` |
| コースのプレイと結果 | `course` |
| PRACTICE | `practice` |
| AUTOPLAY、AUTOPLAY BATTLE | `autoplay` |
| リプレイ再生 | `replay` |
| 単曲の結果 | `result` |

`ready` はルームで準備 OK を押しているときだけ、`select` では常に、`result` では通常の IR 送信（Sending → Done。IR の設定が無ければ最初から）が
終わってから `true`。ほかの場面は `false`。サーバーは `select` のときだけ `ready` を読むので、`result` の値は仕様どおりに送る分で、サーバーには効かない。
`result` で準備 OK の要求は消さない（ルームに残っているため）。取り消し・外された・棄権の確定では消す。

### 表示

選曲画面と結果画面の Arena パネルに出す：段階（プレイ中、プレイ終了・結果待ち）、全員の進み具合（`live` の写し）、結果（順位・名前・EX・クリア、
棄権は `DNF`）、取り消しの理由（B4 で、資料にある 9 件を 6 言語の文にした。知らない理由は「取り消されました」だけで、コードは出さない）、棄権の確定、ルームの解散・追放。

### 未確認・しなかったこと

- `chat_history` の要素、`chat_send` の応答、`chart` の追加キーは資料でも未確認なので読んでいない。
- `force_end_vote`（強制終了）、`result.series`、`return_to_select_at`（BO2）は土台の範囲外。
- 実サーバーでの `live` / `final` の受け付け、READY 後の `start_at` の精度、RANDOM の並びの一致は実機で確認していない（[手順](ARENA_MANUAL_TEST.md)で行う）。
- `final` の `processed_notes`（`complete`）は `play_ready` の `totalnotes` と同じ基準（LN 1 本 = 1、CN/HCN = 2）。CN/HCN の部屋は始められないので、この基準は LN の部屋でしか試せていない。

## B4 の見直し

B1〜B3 を `ARENA_SERVER_MESSAGES.md` と突き合わせて、次を直しました（すべてテスト付き。`arena/live_final_tests.rs`）。

| 直したもの | 内容 |
|---|---|
| 取り消し理由の文面 | `match_cancelled.reason` の 9 件（`not_enough_players_with_chart`、`load_timeout`、`loaded_longnote_contract_mismatch`、`not_enough_ready_players`、`not_enough_option_ready_players`、`not_enough_connected_players`、`not_enough_players_after_withdrawal`、`reservation_timeout`、`server_fault`）を、コードではなく 6 言語の文で出す。資料にあってもこの 9 件に無い理由と、知らない理由は、汎用の「試合が取り消されました」 |
| 古いメッセージ | `match_cancelled` / `match_released` は `match_id` を読み、今の試合と違えば無視する（遅れて届いた別の試合の通知で、今の試合を消さない） |
| `match_resume` | `state=playing` も受ける。長く切れて `start` を逃しても、`start_at` が過去なので READY が止まらず始まる |
| 読み込み中の再接続 | `prepare` を受けて読み込みを始めた試合は、`play_ready` の前でも再接続をまたいで残す（捨てると、手元の READY が開始待ちなしで進んでしまう）。切断中に読み込みが終わったら、再接続後に `play_ready` を送る。app がまだ取っていない `prepare` も残す |
| `live` の早送り | `start_at` の 1 秒前までは送らない（サーバーは 2 秒より前を `too_early` にする。`playstart` が長いスキンで起きうる） |

見直して直さなかったもの（資料に無い、または影響が小さいため）。最初の 2 つは、オペレーターの決定で対応済み：

- `room_disbanded` に `match_id` が付いている（試合中）ときは、試合の取り消し（`private_room_disbanded`）として扱う。ESC などのロックを外し、曲は通常のプレイとして続ける。`final` の送り直しは消す。
- `forfeit_accepted` の理由と再推薦の理由（`retry_reason`、`chart_missing`）は、6 言語の短い文で表示する。知らない理由は汎用の文。
- 選曲画面のオプション（`play_option`）は、前の試合の値が次の試合の `option_ready` に残りうる（app が各試合のオプション選択で必ず送り直すため、通常は問題にならない）。

環境依存の失敗（外部スキンの素材が無い、OS の言語、`storage::scan`）は、B3 と同じ 30 件で、増えていません。

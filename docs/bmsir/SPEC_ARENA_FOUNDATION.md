# 仕様書：BMZ フォークの Arena の土台（接続・対戦の進行・スコア送信・結果）

最終更新：2026-11-12（Issue #1588、親 #1580 の ④）

BMZ のフォーク（`tenP0312-dev/bmz-player`）から BMS-IR Arena に参加できるようにするための、
最初の段階（土台）の仕様です。実装はまだしていません。

読んだ版：

| 対象 | 版 | 読んだもの |
| --- | --- | --- |
| BMS-IR | `main` の `2c7d647` | `docs/BMSIR_ARENA_V1.md`、`tools/bmsir_arena_server.py`、`tools/lr2ir_server/arena.py`、`tools/test_bmsir_arena.py`、`docs/BMZ_MIGRATION_SURVEY.md`、`docs/BMZ_PLAYER_NATIVE_IR_JA.md` |
| BMZ フォーク | `claude/bmsir-only` の `92fed5b5`（0.5.1 の上に BMS-IR 専用化） | `AGENTS.md`、`docs/bmsir/`、`docs/score-persistence.md`、`docs/gameplay-runtime.md`、`crates/` の該当部分 |
| Arena oraja | `main` の `9474d78`（0.4.14.129） | `doc/BMSIR_ARENA.md`、`BMSIRArenaClient.java` ほか Arena のクライアント部分 |

確かめ方の限界：

- どのクライアントもビルド・起動していません。サーバーも動かしていません。コードと文書を読んだ結果です。
- Arena oraja のコードは、動きとプロトコルを読むためだけに使いました。書き写していません。
  BMZ の実装は、この文書の動きの説明から新しく書きます（フォークの `docs/bmsir/README.md` 決定 6）。
- 確かめられなかった点は「未確認」と書きました。推測では埋めていません。

この文書でいう「サーバー」は BMS-IR の Arena サーバー（`bms-ir-arena.service`）です。
通常の IR（スコア送信・ランキング）は「通常の IR」と書きます。

## 1. 目的と範囲

### 土台に入れるもの

- BMS-IR の Arena サーバーへの WSS 接続と認証。
- 対戦の進行：待機 → マッチ → 譜面の決定と受け渡し → 所持の確認 → オプションの確定 →
  読み込み → 同時スタート → プレイ中の進み具合の送受信 → 最終結果 → 結果。
- Arena で遊んだ曲の、通常の IR へのスコア送信（今ある BMS-IR の送信をそのまま使う）。
- Arena の結果の表示（順位・EX・クリアランプ・キャンセルや棄権の理由）。

### 土台に入れないもの（後の段階）

| 機能 | 段階 | 理由 |
| --- | --- | --- |
| レート戦（4 人の BO2、CPU、レートに応じた選曲の上限） | ⑦ レート戦 | BO2 と CPU の譜面選び（6 章）が要る。土台の後に分ける |
| ルームの細かい設定（ルール・ゲージ・選曲範囲・連戦の形式・ホストの操作・見学） | ⑤ ルーム | 土台では決まった設定のルームを作る・コードで入るだけにする |
| 縦の EX グラフ、ターゲット、チャット、通知音、HUD | ⑥ 対戦の画面 | 土台では文字の一覧で結果と進み具合を出す |
| CN / HCN のルーム | ⑤ ルーム | 土台は LN だけにする（4 章） |

おすすめの入り口は「レートなしのルーム」です（10 章の決めてほしいこと 1）。
理由：今のサーバーでは、レート戦の待ち列に入るとプロトコル v6 の BO2 と、足りない席の CPU が必ず付きます。
CPU の譜面選びに参加できないクライアントだけのレート戦は、サーバーが `server_cpu_unavailable` で取り消します
（`tools/bmsir_arena_server.py` の待ち列の処理）。

## 2. 利用者から見た動き

### 設定

- プロファイルの設定に「BMS-IR Arena」の ON / OFF を 1 つ足します。**既定は OFF** です。
  OFF の間は Arena のサーバーへ何もつなぎません。
- ID と認証は、すでにある BMS-IR の IR 設定（プレイヤー ID とゲームトークン）をそのまま使います。
  BMS-IR にログインしていなければ Arena も使えません。その旨を表示します。
- 接続先は製品版では固定です（`wss://www.bms-ir.org/new/arena/ws/client`）。
  開発用の接続先の変更は、BMZ の既存の開発用の仕組み（CLI の上書きなど）に限ります（未確認：どの仕組みに合わせるか）。
  フォークの「BMS-IR 専用」（`fork_policy.rs`）の確認を、Arena の接続先にも同じように通します。
- 設定は BMZ のプロファイルの TOML に置きます。資格情報は書きません。

### 画面と操作

- 選曲画面で開ける Arena のパネル（egui）を 1 つ足します。プレイ中は表示だけで、キー入力を取りません。
- パネルに出すもの：
  - 接続の状態（未接続・接続中・接続済み・拒否された理由）。
  - 待機の状態（ルームのコード、参加者、準備 OK の人数）。
  - 対戦の段階と残り秒数（サーバーの時計に合わせる。8 章）。
  - 結果（順位・名前・EX・クリアランプ・DNF）。
- 操作：
  - ルームを作る（土台では決まった設定。下の表）／コードで入る／抜ける。
  - 準備 OK。
  - 譜面の推薦：選曲画面で今選んでいる譜面を推薦する／サーバーのランダムに任せる。
  - オプションの確定（READY）。
  - 対戦からの棄権。

土台で作るルームの設定（固定）：

| 項目 | 値 |
| --- | --- |
| 公開 | コードだけ（一覧に出さない） |
| 形式 | 1 曲（`series_format=single`） |
| 勝ち負け | EX SCORE |
| ゲージ | 自由 |
| 選曲範囲 | 自由選曲（`free`）。サーバーの譜面一覧にある、ノーツのある譜面 |
| 推薦 | 全員（`all`） |
| LN | LN |
| ルール | プロファイルのルール（`lr2` か `oraja`） |
| キー数 | 7KEY だけ |

（未確認：土台で 7KEY 以外も許すか。BMZ の 5/10/14KEY の扱いと、サーバーの `play_mode` の対応を 4 章で確かめてから決める）

### 対戦中の動き

- マッチが決まったとき通常のプレイ中なら、その曲と通常の IR 送信を先に終えます。
- マッチ中は、通常の曲の開始をふさぎます。推薦の時間は選曲画面を動かせて、決定は推薦になります。
- 課題の譜面が決まると、BMZ が自分で曲を開始します（利用者の決定操作ではありません）。
- 読み込みが終わると READY で止まり、全員がそろうまで待ちます。サーバーが決めた時刻に全員同時に始まります。
- Arena のプレイ中は、ESC と START+SELECT でプレイを抜けられません。
- 曲が終わると、結果の画面へ進みます。Arena の結果はパネルに出ます。
- アシスト、トレーナー、BATTLE、G-BATTLE、キー数の変換などは、その 1 曲だけ使えなくなり、終わったら元に戻ります（4 章）。

## 3. プロトコル v8 のメッセージの流れ

### 3.1 つなぎ方の決まり

| 項目 | 値 | 出どころ |
| --- | --- | --- |
| 接続先 | `wss://www.bms-ir.org/new/arena/ws/client`（サーバーは `/ws/client` も同じに扱う） | サーバー `client_paths` |
| 形式 | WebSocket のテキストフレーム 1 つに JSON オブジェクト 1 つ | サーバー `websocket_loop` |
| 大きさの上限 | 16 KiB（超えると切断） | `MAX_MESSAGE_BYTES` |
| 最初のメッセージ | 必ず `hello`。それ以外は `hello_required` で切断 | |
| 無通信の上限 | 20 秒受け取りがないと切断 | `CLIENT_IDLE_SECONDS` |
| ping | 5 秒ごとに `ping` を送る（Arena oraja と同じ） | |
| WebSocket の ping フレーム | サーバーは pong フレームで返す | |
| 時刻 | すべて UNIX 秒（小数あり）のサーバー時刻 | |

### 3.2 hello（接続と認証）

送る `hello`：

| 項目 | 内容 |
| --- | --- |
| `type` | `"hello"` |
| `protocol` | `8` |
| `player_id` | BMS-IR のプレイヤー ID（数値） |
| `passmd5` | ゲームトークンを UTF-8 にした MD5（小文字 16 進 32 文字）。BMZ 用の通常 IR の API と同じ変換（`auth_protocol.lr2_passmd5`） |
| `client_version` | Arena 用のクライアント版の文字列（5 章で決める。64 文字まで） |
| `build_hash` | ビルド元のコミット（5 章。128 文字まで） |
| `body_version` | BMZ の版（`CARGO_PKG_VERSION`）。サーバーは今は読まない。記録用 |
| `client_flavor` | `"bmz-player"`（新しい値。5 章。**今のサーバーは拒否する**） |
| `ruleset_profile` | `"lr2"` か `"oraja"`（プロファイルのルール。4 章） |
| `arena_enabled` | 設定の ON / OFF |
| `server_cpu_v1` | 土台では送らない（`false` と同じ）。⑦ レート戦で `true` にする |

受け取るもの（成功）：

1. `hello_ok`：`protocol`、`server_protocol`、`session_id`、`client_flavor`、`ruleset_profile`、
   `heartbeat_seconds`（10）、`active_match_id`（進行中の試合。なければ空）、`server_time`。
2. `arena_status`：自分のレート・待機の一覧・ランキング・公開ルーム・ロビーのチャット。
3. `arena_manual`：説明文（土台では読まなくてよい）。
4. 進行中の試合があれば、その段階に合わせた写し（3.6）。

失敗（`type=error` の `code`。どれも直後に切断されます）：

| code | 意味 | BMZ の動き |
| --- | --- | --- |
| `protocol_mismatch` | プロトコルの版が対象外 | 自動の再接続をやめ、「更新が必要」と出す |
| `unsupported_client` | クライアント版が許可リストにない | 同上 |
| `unsupported_build` | ビルドが許可リストにない | 同上 |
| `unsupported_client_flavor` | `client_flavor` が知らない値 | 同上 |
| `authentication_failed` | ID・トークンが違う、またはアカウントが使えない | 自動の再接続をやめ、IR 設定の確認を促す |
| `client_flavor_conflict` | 進行中の試合のルールや形式に、このクライアントが対応していない | 自動の再接続をやめ、理由を出す |
| `hello_required` | 最初が hello でない | 実装の誤り。ログに残す |

### 3.3 いつも送るもの

| 送る | 中身 | いつ |
| --- | --- | --- |
| `ping` | `client_time`（UNIX 秒、小数） | 5 秒ごと。`pong` の `server_time` で時計を合わせる（8 章） |
| `state` | `state`、`arena_enabled`、`ready` | 画面が変わったとき、`hello_ok` の直後、マッチが決まったとき、通常の結果の送信が終わったとき |
| `arena_status` | なし | パネルを開いたとき（任意） |
| `forfeit` | `match_id`、`reason` | 棄権・異常な終わり方（3.5） |

`state` の値（これ以外は `unknown` として扱われます）：

| 値 | BMZ の場面 |
| --- | --- |
| `select` | 選曲画面 |
| `decide` | 決定画面（曲の読み込みの前） |
| `play` | 通常の手動プレイ（SESSION MODE が NORMAL と G-BATTLE） |
| `result` | 単曲の結果画面 |
| `course` | コース・段位のプレイと結果 |
| `practice` | PRACTICE |
| `autoplay` | AUTOPLAY と AUTOPLAY BATTLE |
| `replay` | リプレイ再生 |

`ready` は「今 Arena の次の段階へ進めるか」です。`select` のとき、または `result` で通常の IR 送信が
終わったときだけ `true` にします。ルームでは、さらに準備 OK を押しているときだけ `true` です。
サーバーは `select` のときだけ `ready` を受け取ります。

### 3.4 試合の流れ（正常）

サーバーの試合の段階は `filling`（人集め）→ `selecting`（推薦）→ `loading`（所持の確認）→
`options`（オプションの確定）→ `loading`（読み込み）→ `countdown`（開始待ち）→ `playing` → 終わり、です。

| 順 | サーバーから受け取る | BMZ が送る | BMZ の動き |
| --- | --- | --- | --- |
| 0 | — | `room_entry`（作る／コードで入る）→ `room_ready`（`ready=true`） | どちらも `match_mode="private"`。作るときは `room_code` なしで土台の固定設定を付ける。入るときは `room_code`（とパスワードがあれば `room_password`）を付け、設定は作った人のものを引き継ぐ |
| 1 | `room_status` | — | 参加者・準備の状態を出す |
| 2 | `match_reserved`（`match_id`、`ruleset_profile`、`ln_mode`、`finish_current_song`、`rules`、`reservation_timeout`） | `state` | 試合 ID を覚える。通常のプレイ中なら終わるまで待つ。通常の曲の開始をふさぐ |
| 3 | `fill_started`（`deadline`、`player_count`、`max_players`）、`players_updated` | — | 残り秒数と人数を出す |
| 4 | `nomination_started` / `nomination_status`（自分の推薦の有無、締め切り） | `chart_nominate`（`match_id`、`chart_hash`=MD5）か `chart_nomination_skip`（`match_id`） | 選曲画面の決定を推薦に変える。自由選曲のルームでは、少なくとも 1 人がはっきり推薦する必要がある（締め切りや振り直しは、推薦された候補の中だけで行われる） |
| 5 | `nomination_accepted` | — | 受け付けた譜面を出す |
| 6 | `nominations_revealed` | — | 候補と選ばれた候補を出す |
| 7 | `chart`（`chart`=`md5`/`title`/`artist`/`totalnotes` など、`random_seed`、`ln_mode`、`load_deadline`、`rules`） | `chart_check`（`match_id`、`chart_hash`、`available`、`totalnotes`、`play_mode`） | 手元の曲 DB で MD5 を探す。あれば `available=true`。`totalnotes` は手元で分かる数（分からなければ 0）。`play_mode` は 5/7/9/10/14 |
| 8 | `option_select`（`deadline`、`play_mode`、`ready_count`） | `option_ready`（`match_id`、`play_option`、`play_mode`、`ln_mode`） | Arena 用のオプションを当てはめる（4 章）。利用者が READY を押すか、締め切りの 0.25 秒前に自動で送る |
| 9 | `prepare`（`play_option`、`play_mode`、`random_seed`、`load_deadline`） | — | サーバーが確定したオプションで、自分で曲を開始する（読み込み） |
| 10 | — | `play_ready`（`match_id`、`chart_hash`、`ln_mode`、`totalnotes`） | 読み込みと READY の準備が終わったら 1 回送る。`totalnotes` は実際に読み込んだ譜面のノーツ数 |
| 11 | `start`（`start_at`、`ln_mode`） | — | READY で待ち、サーバー時刻が `start_at` になった瞬間に曲の時計を始める |
| 12 | `live`（全員の進み具合の一覧） | `live`（3.5） | 1 秒ごとに自分の値を送る。受け取った一覧をパネルに出す |
| 13 | — | `final`（3.5） | 曲が終わった瞬間に 1 回送る。届くまで覚えておく |
| 14 | `result`（`players`、`rated`、`auto_reentry_player_ids`、`room_retained_player_ids`） | `state`（`result`、通常の送信が終わったら `ready=true`） | 結果を出す。Arena 用に変えたオプションを元に戻す |

補足：

- 7 の `chart` のとき、試合はすでに `loading` です。サーバーは `chart_check` を「`prepare` を送る前の `loading`」でだけ受け取ります。
- `chart_check` の `chart_hash` が今の候補と違う返事（候補の振り直しの前の返事）は、サーバーが黙って捨てます。
- LN のときは、`totalnotes` が 0 でもなく、サーバーの数とも違うと「持っていない」と同じ扱いになります。
- 10 の `play_ready` で、全員の `ln_mode` と `totalnotes` が同じでないと、試合は `loaded_longnote_contract_mismatch` で取り消されます。
  そろえば、その数が `live` と `final` の確かめに使う「この曲のノーツ数」になります。
- `option` は常に `"PLAYER_SELECTED"` です（オプションは各自が選ぶ）。

### 3.5 プレイ中と最後に送るもの

`live`（1 秒ごと。サーバーは前回から 0.2 秒未満のものを黙って捨てます）：

| 項目 | 内容 |
| --- | --- |
| `match_id` | 試合 ID |
| `seq` | 試合ごとに 1 から増える番号（`final` と共通） |
| `exscore` | 今の EX SCORE |
| `processed_notes` | 処理したノーツ数（判定済みの数と、通過した数の大きい方。この曲のノーツ数を超えない） |
| `minbp` | 今の BP（BAD+POOR+空 POOR） |
| `combo_break` | BAD+POOR の数（空 POOR を含めない）。サーバーはこれを BP として使う |
| `max_combo` | 最大コンボ（`processed_notes` を超えない） |
| `play_option` | `prepare` で受け取った値 |
| `play_mode` | `prepare` で受け取った値 |
| `ln_mode` | 試合の LN の種類 |

`final`（曲が終わった瞬間に 1 回）：`live` の項目に加えて、

| 項目 | 内容 |
| --- | --- |
| `state` | 最後まで遊んだら `complete`、途中で落ちたら `hard_fail` |
| `clear_type` | beatoraja のクリアの番号（0=NO PLAY、1=FAILED、2=ASSIST EASY、3=L-ASSIST、4=EASY、5=NORMAL、6=HARD、7=EX HARD、8=FULL COMBO、9=PERFECT、10=MAX） |
| `processed_notes` | `complete` のときはこの曲のノーツ数そのもの。`hard_fail` のときは実際の数 |

サーバーが拒否する条件（`invalid_live` / `invalid_final` が返ります）：

- `seq` が前より大きくない。EX・処理数・BP・最大コンボが減った。
- 処理数がこの曲のノーツ数を超える（`live` だけ、少しの超過は丸められる）。EX が処理数の 2 倍を超える。
  新しく処理した 1 ノーツあたり 2 を超えて EX が増えた。
- `play_option`・`play_mode`・`ln_mode` が途中で変わった。アシストのオプション。
- `hard_fail` なのに `clear_type` が 1 でない。`complete` なのに `clear_type` が 1 以下。`complete` なのに処理数が足りない。
- 開始時刻より 2 秒以上前に届いた（`too_early`）。

`invalid_live` は、その 1 回だけを捨てます。BMZ は記録して続けます。
`invalid_final` のときは試合は終わらず、最後の猶予（下の表）が切れると棄権になります。BMZ は記録して、パネルに 1 度だけ出します。

### 3.6 失敗・切断・途中からの再開

| 場面 | サーバーの動き | BMZ の動き |
| --- | --- | --- |
| 接続が切れた（人集め・読み込み・開始待ち・プレイ中） | 30 秒待つ。過ぎたら棄権（0 点） | 5 秒後に自動で再接続。曲は止めずに続ける |
| 再接続できた | `hello_ok` の `active_match_id` と、段階に合わせた写し（`match_reserved`、`fill_started`、`nominations_revealed`、`chart`、`option_select`、`prepare`、`match_resume`、`live`、`chat_history`）を送る | 送れていない `final` があれば、まずそれを送り直す。`play_ready` を送った後なら送り直す。そのあと `state` を送る |
| `hello_ok` の `active_match_id` が空なのに、手元は試合中 | — | 手元の試合の状態を消し、変えたオプションを戻す |
| Arena のプレイ中に、`final` なしで選曲画面へ戻った | `play_aborted` で棄権 | 起こらないようにする（ESC 等をふさぐ）。起きたら `forfeit`（`reason=play_aborted`）を送る |
| 結果の画面へ進んだが `final` が届いていない | 10 秒待つ。過ぎたら棄権 | `final` は結果の画面より前に送る |
| 利用者が棄権を選んだ | 開始前なら試合から外す（レートの影響なし）。開始後は棄権（0 点） | `forfeit`（`reason=player_withdrew`）を送る |
| BMZ を終了する | 切断として扱う | 試合中なら `forfeit` を送ってから閉じる |
| 締め切りまでに READY がない | 自動で確定（オプション）／その人を外す（読み込み） | 自動の `option_ready` を送る |
| 譜面を持っていない | 1 度だけ別の候補へ振り直す。2 度目も持っていなければその人を外す。2 人未満なら取り消し | 「持っていない」と出す |
| 試合の取り消し | `match_cancelled`（`reason`） | 手元の試合の状態を消し、オプションを戻し、理由を出す |
| 試合から外された | `match_released` | 同上 |
| 同じアカウントが別のクライアントでつないだ | 古い方に `replaced` を送って切断 | 自動の再接続をしない（取り合いを防ぐ。10 章の決めてほしいこと 7） |
| Arena サーバーの再起動 | 進行中の試合は `server_fault`（レートなし） | 再接続のあと、`active_match_id` が空なので状態を消す |
| 知らないメッセージを受け取った | — | 無視してログに残す（後の段階のメッセージが来ても止まらない） |

`match_cancelled` の `reason` の例：`not_enough_players_with_chart`、`load_timeout`、
`loaded_longnote_contract_mismatch`、`not_enough_ready_players`、`not_enough_option_ready_players`、
`not_enough_connected_players`、`not_enough_players_after_withdrawal`、`reservation_timeout`。
知らない値は「試合が取り消されました」とだけ出します。

### 3.7 その他のエラーの code

`queue_entry_failed`、`room_entry_failed`、`room_ready_failed`、`nomination_rejected`、
`nomination_closed`、`nomination_already_submitted`、`invalid_chart_check`、`invalid_option_ready`、
`invalid_play_ready`、`invalid_json`、`unknown_message`、`arena_disabled` などがあります。
どれも接続は切れません。パネルに 1 度だけ出し、ログに残します。

## 4. BMZ での作り

### 置き場所

| 部分 | 置き場所 | 理由 |
| --- | --- | --- |
| メッセージの型（serde の構造体）と JSON の読み書き | `bmz-player` の新しいモジュール `arena/protocol` | ネットワークを持ち込まないため `bmz-gameplay` には置かない |
| 試合の状態の移り変わり（送る・受ける・待つ） | `arena/session`（入出力を持たない純粋な状態機械） | 単体テストで流れを確かめるため |
| WSS の接続・再接続・ping | `arena/connection`。専用スレッドに tokio の current-thread runtime と `tokio-tungstenite`（OBS 連携と同じ依存） | window スレッドとゲームプレイのスレッドを止めない |
| 画面遷移・プレイの開始と止め方・結果へのつなぎ | `app/` の既存の流れ（`play_flow/launch`、`result_flow`、選曲の入力）から `arena` を呼ぶ | 既存の流れを変えず、入口を足すだけにする |
| パネル | `ui/` の egui | AGENTS.md の「egui の状態は app 側」に合わせる |
| 資格情報 | `ir/credentials.rs` の BMS-IR の資格情報を読むだけ | Arena 用の保存場所を増やさない |
| 接続先の確認 | `fork_policy.rs` | BMS-IR 専用化の確認を Arena にも通す |

Arena を IR の provider の一部にはしません。provider は「送信のキュー」の仕組みで、
Arena は「常につながる対戦サービス」なので、別の境界にします。

スレッドの決まり：

- 接続スレッドと window スレッドの間は、チャンネルでメッセージを受け渡します。
- プレイ中の値（EX、判定の数、最大コンボ）は、ゲームプレイのスレッドが作る既存のスナップショットから
  window スレッドが読み、1 秒ごとに `live` を作って接続スレッドへ渡します。ゲームプレイのスレッドには手を入れません。
- 接続の失敗やサーバーの不具合は、通常のプレイ・スコアの保存・通常の IR 送信に影響させません。

### 既存の区分を変えないこと

- Arena の曲は SESSION MODE が NORMAL の、ふつうの手動プレイです。G-BATTLE・AUTOPLAY・PRACTICE にはしません。
- スコアは通常の score bucket（DOUBLE OPTION が Off か FLIP）に保存し、通常どおり BMS-IR へ送ります。
  Arena 専用の保存先は作りません。リプレイも通常どおり保存し、実際に使った seed を残します。
- score identity（SHA-256 / LN policy / DOUBLE bucket / RuleMode）、保存の条件（`docs/score-persistence.md`）は変えません。

### Arena の曲で当てはめるもの（その 1 曲だけ。終わったら戻す）

| 項目 | Arena での値 |
| --- | --- |
| ルール | `ruleset_profile` が `lr2` なら `Lr2Oraja`、`oraja` なら `Beatoraja`。`Dx` は使わない |
| LN | 土台は LN だけ（`LnPolicySetting::ForceLn`）。CN / HCN（`AutoCn` / `AutoHcn` に当てる案）は ⑤ で確かめる |
| 配置（`ArrangeOption`） | NORMAL / MIRROR / RANDOM / R-RANDOM / S-RANDOM / SPIRAL はそのまま。H-RANDOM、ALL-SCR、RANDOM-EX、S-RANDOM-EX、F-RANDOM、MF-RANDOM は NORMAL にする |
| RANDOM の seed | 配置が RANDOM の側には、サーバーの `random_seed` をそのまま使う（R-RANDOM・SPIRAL・S-RANDOM には使わない） |
| DOUBLE OPTION | Off と FLIP だけ。BATTLE と BATTLE AS は Off にする |
| SESSION MODE | NORMAL |
| アシスト | EXTRA NOTE、地雷、LN の変更、Random Trainer、SP-TO-DP、キー数の変換、判定の自動調整など、譜面や判定を変えるものは切る |
| ゲージ | 利用者の設定のまま（土台のルームはゲージ自由） |
| HS、レーンカバー、表示の時刻 | 利用者の設定のまま |
| ワンバス入力、READY の最初のノーツ表示 | ワンバスは Arena では働かない（`SPEC_ONE_BASS_INPUT.md` の決まり）。READY の表示は任意 |

`play_option` の数の作り方：1 の位が 1P の配置（0=NORMAL、1=MIRROR、2=RANDOM、3=R-RANDOM、4=S-RANDOM、5=SPIRAL）、
10 の位が 2P の配置、100 の位が FLIP（1）です。SP は 1 の位だけを送ります。

`play_mode` は、5KEY=5、7KEY=7、9KEY=9、10KEY=10、14KEY=14 です。
BMZ の譜面のモードからこの数への対応表を作ります（未確認：BMZ の譜面のモードの型の名前と、24K/48K などの有無）。

### 実装で必ず確かめる点（未確認）

- 読み込みの後、音声時計を始める位置。BMZ は READY の前の準備用 runtime を持ち、音声時計の開始後に
  ゲームプレイのスレッドへ渡します（`docs/gameplay-runtime.md` の Lifecycle）。サーバー時刻の `start_at` まで
  この開始を待たせる入口を `app/play_flow/launch` のどこに置くかは、コードを読み直して決めます。
  スキンの `playstart` の分だけ曲の 0 秒が後ろにずれる点も合わせます。
- ESC と START+SELECT でプレイを抜ける処理の場所。Arena のプレイ中だけふさぎます。
- RANDOM の並びが Arena oraja と同じになるか。BMZ の配置の乱数（`screens/play_session/arrange/rng.rs`）は
  beatoraja の `LaneRandomShuffleModifier` と同じ方式とコメントにあり、`java.util.Random` 互換の乱数を使っています。
  サーバーにも同じ並びを作る関数（`_java_lane_pattern`）があるので、テストで突き合わせます（7 章）。
- BMZ の `Lr2Oraja` ルールが、Arena oraja の LR2 ルール（判定の幅、ゲージ、BAD の選び方、LN の遅い BAD）と同じか。
  同じでなければ、Arena oraja と同じ試合に入れると公平ではありません（5 章のプールの分け方）。
- 同じ譜面で、BMZ と Arena oraja の読み込み後のノーツ数が同じか。違うと、混ぜた試合は `play_ready` で取り消されます。
- egui のパネルがプレイ中に入力を取らないこと。

### BMZ の AGENTS.md の決まりとの対応

- ゲームプレイのスレッド・音声のコールバックに通信を持ち込みません。
- 非同期の結果は「試合 ID」と「段階」で古いものを捨てます（AGENTS.md の「要求の世代」の決まりと同じ考え方）。
- 新しい設定は 6 言語の翻訳を付けます。
- フォークの決まり：コミットにモデル名を書かない。PR はフォークの `main` へ、マージせずに置く。本家には出さない。

## 5. サーバー側の受け付け（BMS-IR 側に必要な変更）

### 今の受け付け

- `client_flavor` は `endless-dream`、`beatoraja`、`arena-oraja`、`openlr2` だけです。ほかは `unsupported_client_flavor` で拒否します。
- 本番では、`client_version` が許可リスト（`BMSIR_ARENA_ALLOWED_CLIENT_VERSIONS`）に、
  `build_hash` が許可リスト（`BMSIR_ARENA_ALLOWED_BUILD_HASHES`）に入っていなければ拒否します。
  2 つのリストは flavor を区別しません。環境変数と、非公開の許可リストのファイルを合わせて読みます。
- マッチはルールの種類（`lr2` / `oraja`）と部屋の決まりで組みます。flavor が違っても、同じルールに
  対応していれば同じ試合に入ります（`arena-oraja` は両方のルールに対応）。
- 許可リストの値は、この文書にも Issue にも書きません。

### 決めごと（案）

| 項目 | 案 |
| --- | --- |
| 版の識別（flavor） | 新しい値 `bmz-player` を足す。`lr2` と `oraja` の両方に対応する |
| クライアント版 | フォークで Arena 用の版の文字列を 1 つ決める（例：`bmz-bmsir-` で始まる形）。BMZ 本体の版（`CARGO_PKG_VERSION`）とは別に、Arena のプロトコルに関わる変更のたびに上げる |
| ビルドの識別 | BMZ のビルドが埋め込むコミット（`BMZ_BUILD_COMMIT`、40 桁）を `build_hash` に入れる。`-dirty` や `unknown` のビルドは、本番では受け付けない（許可リストに載らない） |
| 許可リスト | flavor ごとに別のキーにする（`BMSIR_ARENA_ALLOWED_BMZ_CLIENT_VERSIONS`、`BMSIR_ARENA_ALLOWED_BMZ_BUILD_HASHES` の案）。Arena oraja の値を BMZ が名乗っても、その逆でも通らないようにする |
| マッチのプール | 当面、`bmz-player` は `bmz-player` どうしだけで組む（4 章の未確認のルールとノーツ数の差のため）。ルームも、作った人の flavor の種類と同じものだけが入れる |
| できることの制限 | 土台の間、`bmz-player` はレート戦の待ち列（`queue_entry`）に入れない。ルームは 1 曲・LN・自由選曲か公式表だけ。これ以外は分かりやすいエラーで断る。段階が進んだら外す |
| 開発用 | 手元の開発では、今ある `BMSIR_ARENA_ALLOW_DEV_CLIENT=1` で版とビルドの確認を外せる（本番では使わない） |

反則への対策の考え方：

- BMZ もフォークも誰でもビルドできる公開のコードです。版とビルドの識別は「どの版の動きか」を揃えるための
  入口の確認で、改造されたクライアントを防ぐ証明にはなりません（Arena oraja と同じ考え方。`BMSIR_ARENA_V1.md`）。
- 反則への対策は、今と同じくサーバー側の確かめ（EX と処理数の上限・増え方、順番、ノーツ数、時刻、最終結果の条件）です。
  BMZ のために緩めません。
- レート戦（⑦）に BMZ を入れる前に、混ぜた試合の公平さ（4 章の未確認の 2 点）を確かめます。
- 通常の IR へのスコアは、今ある BMZ 用の API（`/api/bmz-player/v1/score`、`client_kind` は `bmz-lr2` / `bmz-oraja`）で届きます。
  Arena oraja の本体の指紋の許可リストは、BMZ には使いません（BMZ の API はもともと本体の指紋を見ません）。

### BMS-IR 側の変更（1 つの PR）

1. `tools/lr2ir_server/arena.py`：`CLIENT_FLAVORS` に `bmz-player` を足す。データベースの列は文字列なので、形の変更は要りません。
2. `tools/bmsir_arena_server.py`：
   - `client_supports_ruleset` で `bmz-player` を `lr2` と `oraja` の両方に対応させる。
   - `authenticate` で、flavor ごとの許可リストを読む（上の案）。許可リストのファイルの再読み込みの仕組みは今のものを使う。
   - `/healthz` の「許可リストが空なら 503」は、今のキー（Arena oraja）だけを見るままにする。BMZ のキーが空でも Arena 全体は止めない。
   - マッチの組み合わせ（待ち列とルームへの参加）で、`bmz-player` と他の flavor を分ける。
   - 土台の間の制限（レート戦の待ち列とルームの設定）。
3. `tools/test_bmsir_arena.py`：上のすべての受け付けと拒否のテスト（7 章）。
4. 文書：`docs/BMSIR_ARENA_V1.md` の「Authentication And Release Gate」と flavor の一覧、
   `docs/implemented/` と `docs/regression/` の Arena の項目。
5. 確認：`python3 -m py_compile` と `python3 tools/test_bmsir_arena.py -v` と `python3 tools/regression_guard.py`（`BMSIR_ARENA_V1.md` の Repository Validation）。

本番への反映：

- サーバーの変更は、オペレーターの明示の deploy の依頼で、`docs/PRODUCTION_VPS_OPERATIONS.md` に従います。
  Arena の再起動は、待機中 0 人・進行中の試合 0 のときに行い、JST 18:00–24:00 は避けます。
- BMZ のビルドの版とビルドを本番の許可リストに足すのは、オペレーターの作業です（非公開の値なので、リポジトリに入れません）。
- フォークのビルドの配り方は決まっていません（10 章の決めてほしいこと 5）。BMS-IR のランチャーで配るなら、
  `docs/PRODUCTION_VPS_OPERATIONS.md` のゲートの手順の対象になります。

参考（文書と実装の差）：`BMSIR_ARENA_V1.md` は `live` の受け付けを「0.75 秒に 1 回」と書いていますが、
今のコードは 0.2 秒（`MIN_LIVE_INTERVAL_SECONDS = 0.20`）です。BMZ は 1 秒ごとに送るので、どちらでも問題ありません。

## 6. レート戦（⑦）で増えるもの（参考）

土台には入れませんが、作りがぶつからないように書いておきます。

- 待ち列：`queue_entry`（`unrestricted_rating`、`allow_cpu`、`allow_higher_selection`、`ln_mode="LN"`）。`queue_cancel`。
  レート戦は `lr2` ルールとプロトコル v6 以上が必要です。
- 人数が 4 人に足りないと、サーバーの CPU が入ります。そのため人間は全員 `server_cpu_v1=true` を名乗り、
  `cpu_chart_request` に `cpu_chart_candidate`（手元にある、指定の範囲の公式表の譜面）で答える必要があります。
  名乗らないクライアントがいると、試合は取り消されます。
- BO2：2 曲の連戦。1 曲目の結果に `return_to_select_at`（15 秒後）が付き、その時刻に結果の画面を抜けます。
- レートの表示と、選曲の上限（`☆1`〜`★25`）。

## 7. テストの観点

### BMZ の単体テスト

- メッセージの読み書き：各メッセージの JSON（サーバーのテストにある形）を読めること。知らない `type` と、知らない項目があっても失敗しないこと。
- 状態機械：3.4 の正常な流れを順にたどれること。古い `match_id` や前の候補の返事を捨てること。
  `match_cancelled`、`match_released`、`replaced`、`hello_ok` の `active_match_id` が空、の各場面で状態が消えること。
- 再接続：送れていない `final` を、`hello_ok` の後に `state` より先に送り直すこと。`play_ready` の送り直し。
- 時計：`ping` / `pong` の往復の中間から差を求めること。`start_at` の判定。
- `live` の間隔：1 秒に 1 回を超えないこと。`seq` が増え続けること。処理数・最大コンボの上限。
- `final`：`complete` で処理数がこの曲のノーツ数になること、`hard_fail` で `clear_type=1` になること。
- `play_option` の数の作り方（SP、DP、FLIP）と、アシストの配置を NORMAL にすること。
- クリアの種類から番号への対応（0〜10）。
- RANDOM の並び：サーバーの `_java_lane_pattern(seed, 7)` が作る並びと、BMZ の RANDOM が作る並びが、
  いくつかの seed で一致すること（サーバー側の関数から作った固定の表をテストに置く）。
- Arena の曲の後に、変えた設定（配置、DOUBLE OPTION、SESSION MODE、LN、ルール、アシスト）が元に戻ること。
- Arena の曲のスコアが、通常の bucket に、通常の条件で保存されること（`score-persistence.md` の表どおり）。

### BMS-IR のテスト（`tools/test_bmsir_arena.py`）

- `client_flavor=bmz-player` の hello が、BMZ 用の許可リストで通り、Arena oraja の許可リストの値では通らないこと（その逆も）。
- `bmz-player` が `lr2` と `oraja` の両方で `hello_ok` を受け取れること。
- `bmz-player` と `arena-oraja` が同じ試合・同じルームに入らないこと。
- 土台の制限：レート戦の待ち列とルームの設定を、分かりやすいエラーで断ること。
- 今ある flavor の動きが変わらないこと（今あるテストがすべて通ること）。

### つないで確かめる（開発環境。オペレーターの PC）

- 手元で Arena サーバーを `BMSIR_ARENA_ALLOW_DEV_CLIENT=1` で動かし、BMZ の接続先を手元に向けます。
- 相手には、2 つ目の BMZ を使います（同じ PC の別プロファイル、別アカウント）。
- 本番のサーバーにはつなぎません（許可リストへの追加が終わるまで）。

## 8. 時計を合わせる

- 5 秒ごとに `ping`（`client_time`=送った時刻）を送り、`pong`（`server_time`、`client_time`）を受け取ります。
- 差 = `server_time` − （送った時刻 + 往復の時間の半分）。`hello_ok` の `server_time` でも最初の値を作ります。
- 残り秒数の表示と、`start_at` の判定は「手元の時刻 + 差」で行います。
- `start_at` は `play_ready` が全員そろってから 5 秒後です（`START_LEAD_SECONDS`）。
- 曲の時計の始まりは、手元の時刻ではなく BMZ の音声時計で進みます。`start_at` に合わせるのは「始める瞬間」だけです。

## 9. 段階の分け方と見積もり

| 段階 | リポジトリ | 中身 | 大きさ | 前提 |
| --- | --- | --- | --- | --- |
| S1 | BMS-IR | `bmz-player` の flavor、flavor ごとの許可リスト、プールの分け方、土台の制限、テスト、文書（5 章） | 小〜中（PR 1 つ） | 10 章の 2〜4・8 の返事 |
| B1 | BMZ フォーク | 接続の土台：設定（既定 OFF）、`arena/protocol`、`arena/connection`（hello、ping、時計、再接続、拒否の表示）、`state` の送信、パネルの最小（接続の状態とレートの表示） | 中（PR 1 つ） | なし（手元のサーバーで確かめる） |
| B2 | BMZ フォーク | 試合の前半：ルームを作る・入る、準備 OK、推薦、`chart` と `chart_check`、オプションの当てはめと戻し、`option_ready`、`prepare` で自分で曲を始める、`play_ready`、`start_at` まで待つ | 大（PR 1 つ。大きければ「ルームと推薦」と「読み込みと開始」の 2 つ） | B1 |
| B3 | BMZ フォーク | 試合の後半：`live` と `final`、送れていない `final` の送り直し、ESC 等をふさぐ、`forfeit`、結果・取り消し・棄権の表示 | 中（PR 1 つ） | B2 |
| B4 | BMZ フォーク | つないだ確認の手順と、見つかった直し、文書（フォークの `docs/bmsir/`） | 小（PR 1 つ） | B3、S1 の本番反映（本番で試す場合） |

合計：BMS-IR 1 つ、BMZ 4〜5 つの PR。どれも小さく分け、既存の G-BATTLE・スコア・リプレイの区分は変えません。
S1 と B1 は並行して進められます。本番につなぐのは S1 の本番反映と、許可リストへの追加の後です。

## 10. オペレーターに決めてほしいこと

1. **土台の入り口**：レートなしのルーム（コードで作る・入る、固定の設定）から始め、レート戦は ⑦ にする案でよいか。
2. **flavor の名前**：`bmz-player` でよいか。
3. **許可リストの分け方**：flavor ごとに別のキーにする案でよいか。
4. **プールの分け方**：当面、BMZ は BMZ どうしだけで対戦する案でよいか（Arena oraja と混ぜるのは、ルールとノーツ数の差を確かめた後）。
5. **フォークのビルドの配り方**：だれがビルドし、どこで配るか。BMS-IR のランチャーに載せるか。
   版の文字列（例：`bmz-bmsir-` で始まる形）の決め方。
6. **既定の接続**：Arena の接続は既定 OFF でよいか。
7. **`replaced` のときの動き**：別のクライアントに置き換えられたら、自動で再接続しない案でよいか。
8. **土台の間の制限**：サーバー側で、BMZ のレート戦への参加と、土台で扱わないルームの設定を断る案でよいか。
9. **キー数**：土台は 7KEY だけにする案でよいか。

## 参照

- Issue #1588（この仕様）、#1580（BMZ への移行）
- `docs/BMSIR_ARENA_V1.md`、`docs/BMZ_MIGRATION_SURVEY.md`、`docs/BMZ_PLAYER_NATIVE_IR_JA.md`
- `docs/bmz/SPEC_FHS_MARK.md`、`docs/bmz/SPEC_ONE_BASS_INPUT.md`、`docs/bmz/SPEC_READY_FIRST_NOTES.md`
- `tools/bmsir_arena_server.py`、`tools/lr2ir_server/arena.py`、`tools/test_bmsir_arena.py`
- BMZ フォーク：`AGENTS.md`、`docs/bmsir/`、`docs/score-persistence.md`、`docs/gameplay-runtime.md`
- Arena oraja：`doc/BMSIR_ARENA.md`

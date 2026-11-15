# BMS-IR Arena WebSocket プロトコル v8 メッセージ仕様（クライアント実装者向け）

サーバ実装（Arena コーディネータ）から読み取った「メッセージの形」だけをまとめたもの。
ホスト名・ポート・認証情報の値などの運用情報は含まない。コードから確認できなかった点は **未確認** と記す。

## 0. 共通事項

| 項目 | 内容 |
|---|---|
| 転送 | WebSocket テキストフレーム（opcode 0x1）。1メッセージ = 1 JSON オブジェクト。バイナリは不可（切断）。クライアントフレームはマスク必須 |
| サイズ上限 | 1フレーム 16 KiB（超過で切断） |
| アイドル | 受信が 20 秒途絶えると切断。`ping` を約 10 秒ごとに送る（`hello_ok.heartbeat_seconds` = 10） |
| 判別 | すべて `"type"` で判別。未知の type → `error` `unknown_message` |
| 時刻 | `deadline` / `start_at` 等はすべて **サーバ時計の UNIX epoch 秒（float）**。`hello_ok.server_time` / `pong.server_time` で時計差を補正する |
| ID | `player_id` は整数、`match_id` / `series_id` / `session_id` は文字列（形式は不透明として扱う） |
| バージョン | `PROTOCOL_VERSION = 8`。受理: 3,4,5,6,7,8。v4=シリーズ, v5=管理ルーム, v6=レート BO2, v7=custom 譜面範囲, v8=CN/HCN（`ln_mode`）|
| bool の扱い | 多くのフラグは **`=== true` のときのみ真**（`"1"` や `1` は偽扱い）。例外は個別に記載 |

### error（サーバ→クライアント）
```json
{"type":"error","protocol":8,"code":"<code>","message":"<英語の説明>"}
```
`message` は表示用。分岐は `code` で行う（一覧は §5）。

### ln_mode（v8）
| 値（送受信の正規形） | 意味 | 受理される別表記 |
|---|---|---|
| `"LN"`（既定） | ロングノート | `0`, `"0"`, `"LONGNOTE"`, `"LONG_NOTE"`, `"LONG-NOTE"` |
| `"CN"` | チャージノート | `1`, `"1"`, `"CHARGENOTE"`, `"CHARGE_NOTE"` |
| `"HCN"` | ヘルチャージノート | `2`, `"2"`, `"HELLCHARGENOTE"`, `"HELL_CHARGE_NOTE"` |

型は文字列（整数も受理）。大文字小文字無視。bool は不正。省略時 `"LN"`。サーバは常に大文字文字列で返す。
`CN`/`HCN` は v8 未満クライアントでは `room_entry_failed`。v8 未満クライアントの送る ln_mode は無視され `"LN"` 扱い。
部屋の `ln_mode` は作成時に固定（`room_settings` では変更不可）。マッチ中に送る `option_ready`/`play_ready`/`live`/`final` の `ln_mode` が部屋と異なるとエラー。
LN 以外では `chart_check.totalnotes` の照合をスキップし、`play_ready.totalnotes`（実ロード値）を全員一致で採用する。

## 1. 接続ハンドシェイク

| 方向 | type | フィールド |
|---|---|---|
| C→S | `hello`（最初の1通必須） | `player_id` int, `passmd5` string(32桁hex), `client_version` string(≤64), `build_hash` string(≤128), `protocol` int(=8), `client_flavor` string, `ruleset_profile` `"lr2"`\|`"oraja"`（省略時 flavor から推定: beatoraja→oraja, 他→lr2）, `arena_enabled` bool, `server_cpu_v1` bool, `server_cpu_catalog_v1` bool |
| S→C | `hello_ok` | `protocol`, `server_protocol`, `player_id`, `session_id`, `client_flavor`, `ruleset_profile`, `heartbeat_seconds`(10), `active_match_id`（進行中マッチ、無ければ `""`）, `server_time` |
| S→C | 続けて `arena_status`, `arena_manual`, その後再同期メッセージ（§4.4） | |
| S→C | `error` で拒否→切断 | `hello_required`, `protocol_mismatch`, `unsupported_client_flavor`, `unsupported_client`, `unsupported_build`, `authentication_failed`, `client_flavor_conflict` |
| S→C | `replaced` | 同一アカウントの別接続が来た。`message` のみ。その後旧接続は閉じられる |
| C→S / S→C | `ping` / `pong` | `ping.client_time`(任意,数値) → `pong` は `server_time` と（あれば）`client_time` を返す |

## 2. クライアント→サーバ

### 2.1 room_entry（ルーム作成 / 参加）
`match_mode` は `"private"` のみ有効（`"ranked"` はキュー `queue_entry` を使う、`"casual"` は廃止→エラー）。
**作成**: `room_code` を省略/空。**参加**: `room_code` に既存コード（英数字6文字、大小無視・空白除去）。

| フィールド | 型 | 値 / 既定 | 作成時 | 参加時 |
|---|---|---|---|---|
| `match_mode` | string | `"private"`（必須。不正値はエラー） | ○ | ○ |
| `room_code` | string | 空=新規作成 / 6文字英数字=参加 | 省略 | 必須 |
| `room_password` | string | ≤64文字、制御文字不可。空=鍵なし | 設定 | 照合（不一致→`room_entry_failed` "room password is incorrect"） |
| `room_name` | string | ≤40文字（NFC・空白正規化）。空→`"<作成者名>の部屋"` | ○ | 無視 |
| `score_rule` | string | `"exscore"`(既定)\|`"minbp"`\|`"max_combo"` | ○ | 検証のみ、部屋の値で上書き |
| `forced_gauge` | string | `"free"`(既定)\|`"normal"`\|`"hard"`\|`"exhard"`\|`"hazard"` | ○ | 同上 |
| `chart_scope` | string | `"official"`(既定)\|`"free"`\|`"custom"`(v7+) | ○ | 同上 |
| `ruleset_profile` | string | `"lr2"`\|`"oraja"`。省略時 flavor から推定 | ○ | 部屋の値（非対応クライアントは拒否） |
| `ln_mode` | string/int | §0 参照、既定 `"LN"` | ○ | 部屋の値 |
| `nomination_policy` | string | `"all"`(既定)\|`"host"`\|`"rotate"` | ○ | 部屋の値 |
| `nomination_seconds` | int | 10–180、既定 60 | ○ | 部屋の値 |
| `option_seconds` | int | 5–60、既定 10 | ○ | 部屋の値 |
| `intermission_seconds` | int | 0–60、既定 0（シリーズ次ラウンドまでの待ち） | ○ | 部屋の値 |
| `series_format` | string | `"single"`(既定)\|`"all_picks"`\|`"first_to"`\|`"bo2"`（single 以外は v4+） | ○ | 部屋の値 |
| `first_to_wins` | int | 2–5、既定 2（`first_to` の目標勝数。他形式でも範囲検証される） | ○ | 部屋の値 |
| `spectator_public` | bool | 既定 false。true=観戦公開（=ロビーの公開ルーム一覧/観戦対象） | ○ | 部屋の値 |
| `force_host_option` | bool | 既定 false。true=ホストの譜面オプションを全員に強制 | ○ | 部屋の値 |
| `allowed_play_modes` | int[] | 5,7,9,10,14 の部分集合。空/不正→`[7]` | ○ | 部屋の値 |
| `custom_selection` | object | `chart_scope="custom"` 時必須。`{"tables":[{"kind":"supported","table_key":str,"levels":[str]} \| {"kind":"user","table_id":int,"share_key":str?,"levels":[str]}]}`（最大12表） | ○ | 無視 |
| `stay_in_room` | bool | 既定 false。試合後も部屋に残る | ○ | ○ |
| `room_participating` | bool | **`false` のときのみ非参加**（既定 true）。非参加=観戦席扱い | ○ | ○ |

- **`public` / `seconds` / `password` という名前のフィールドは存在しない**（それぞれ `spectator_public`、`nomination_seconds`/`option_seconds`/`intermission_seconds`、`room_password`）。
- 前提: `hello.arena_enabled`（または `state.arena_enabled`）が true でないと `arena_disabled`。
- 成功時の直接の応答メッセージは無い。部屋の全メンバーに `arena_status` と `room_status` が配信され、参加者には（あれば）`chat_history` が届く。作成者は `arena_status.player.queue.room_code` で発行されたコードを知る。
- 失敗: `error` `room_entry_failed`（message に理由。部屋なし / パスワード不一致 / クライアント種別違い / プロトコル不足 / custom で譜面ゼロ 等）。
- 進行中シリーズのある部屋に参加者として入ると、そのシリーズ終了後から参加（`room_status.players[].pending=true`）。

### 2.2 ルーム操作

| type | フィールド | 権限 | 応答 / 効果 | 失敗 code |
|---|---|---|---|---|
| `room_settings` | §2.1 の `score_rule`,`forced_gauge`,`chart_scope`,`ruleset_profile`,`nomination_policy`,`nomination_seconds`,`option_seconds`,`intermission_seconds`,`series_format`,`first_to_wins`,`spectator_public`,`force_host_option`,`allowed_play_modes`,`custom_selection`,`room_name`；パスワード変更は `update_password:true` + `room_password`（空で解除） | ホストのみ | 全員に `arena_status`+`room_status`。待機中（filling）マッチがあれば設定反映・**全員の ready 解除**・`live` 配信。`ln_mode` は変更不可。注意: `nomination_policy`/`nomination_seconds`/`option_seconds`/`intermission_seconds`/`series_format`/`first_to_wins` は省略すると既定値に戻る（送るときは全項目送ること）。他の項目は省略時現状維持。series 以外→`nomination_policy` は強制 `all`。シリーズ進行中は形式/目標勝数変更不可 | `room_settings_failed` |
| `room_ready` | `ready` bool | 参加中メンバー | `client.ready = ready && arena_enabled && state=="select"`。全員に `room_status`（`players[].ready`）。待機マッチ中は `live` 配信と開始判定 | `room_ready_failed` |
| `room_stay` | `stay_in_room` bool | 非ranked のエントリー保有者 | 自分に `arena_status` | `room_stay_unavailable` |
| `room_participation` | `participating` bool | メンバー | 参加/観戦席の切替。シリーズ中に参加へ切替えると次シリーズから。全員に `room_status` | `room_participation_failed` |
| `queue_cancel` | なし | 任意 | **部屋から退出（=エントリー取消）**。マッチ前なら即時退出、予約/試合中は `withdraw_requested` となり、サーバがマッチから外す（`match_released` または試合中なら `forfeit_accepted`）。ホストが退出すると最古参加者へホスト移譲、誰もいなくなれば部屋消滅。自分に `arena_status` | `queue_cancel_failed` |
| `room_disband` | なし | ホストのみ | 部屋解散。全メンバー（と観戦者）に `room_disbanded` | `room_disband_failed` |
| `room_kick` | `player_id` int | ホストのみ（自分以外） | 対象に `room_kicked`。対象がマッチ中なら準備段階は `match_released`(reason `private_room_kicked`)、プレイ中は強制棄権 | `room_kick_failed` |
| `room_transfer_host` | `player_id` int | ホストのみ | ホスト変更。マッチがあれば `live` 配信 | `room_transfer_host_failed` |
| `room_set_selector` | `player_id` int | ホストのみ、`series_format="single"` 部屋 | `rotate` 方式の現在の選曲者を指定。マッチがあれば `live` 配信 | `room_set_selector_failed` |

### 2.3 状態通知

| type | フィールド | 説明 |
|---|---|---|
| `state` | `state` string: `select`\|`decide`\|`play`\|`result`\|`course`\|`practice`\|`autoplay`\|`replay`（他は `unknown`）, `arena_enabled` bool（省略時現状維持）, `ready` bool | 画面状態を随時送る。**`ready` は `state=="select"` のときだけ有効**で、送るたびに ready が上書きされる（`ready` を省略すると false）。`playing` 中に `select` へ戻ると棄権（`play_aborted`）、`result` へ移ると 10 秒の final 猶予 |

### 2.4 マッチ進行（すべて `match_id` string 必須）

| type | 追加フィールド | 送るタイミング | 備考 / エラー |
|---|---|---|---|
| `chart_nominate` | `chart_hash` string（md5） | `nomination_started` 受信後、締切前 | 成功→自分に `nomination_accepted`、全員に `nomination_status`。`nomination_closed` / `nomination_rejected` / `nomination_already_submitted` |
| `chart_nomination_skip` | なし | 同上 | 指名を放棄（サーバ/他候補から選ばれる）。free+シリーズでは不可（`nomination_rejected`） |
| `chart_check` | `chart_hash` str, `available` bool（truthy 判定）, `totalnotes` int, `play_mode` int | `chart` 受信後 | 手元に譜面があるか。`play_mode` は 5,7,9,10,14,25,50（部屋の `allowed_play_modes` 外は `invalid_chart_check`）。LN 部屋で `totalnotes` が 0 でも一致でもなければ「無し」扱い。古い `chart_hash` は黙って無視 |
| `option_ready` | `play_option` int, `play_mode` int, `ln_mode` | `option_select` 受信後、締切前 | `play_option` = 1P乱数 + 10×2P乱数 + 100×FLIP。乱数 0..5（NORMAL,MIRROR,RANDOM,R-RANDOM,S-RANDOM,SPIRAL）、FLIP 0/1、DP系(10,14,50)以外は <10。`invalid_option_ready`。状態不一致は黙って無視 |
| `play_ready` | `chart_hash` str, `ln_mode`, `totalnotes` int（v8 必須・>0、実ロードしたノーツ数） | `prepare` 受信後、譜面ロード完了時 | `invalid_play_ready`。全員揃うと `start` |
| `live` | `seq` int（単調増加）, `exscore` int（単調非減少）, `processed_notes` int, `combo_break`（または `minbp`）int, `max_combo` int, `play_option` int, `play_mode` int, `ln_mode` | `start_at` 以降プレイ中（最短 0.2 秒間隔。超過分は黙って破棄） | `start_at` の2秒以上前は `too_early`。オプション/モード変更・BP/MAXCOMBO の減少・EX>2×processed は `invalid_live` |
| `final` | `seq`,`exscore`,`processed_notes`,`combo_break`/`minbp`,`max_combo`,`play_option`,`play_mode`,`ln_mode`, `state` `"complete"`\|`"hard_fail"`, `clear_type` int | プレイ終了時 | `clear_type`: 0 NO PLAY,1 FAILED,2 AE,3 LAE,4 EC,5 NC,6 HC,7 EXHC,8 FC,9 P-A,10 MAX。`hard_fail` は clear_type=1 必須、`complete` は ≥2 かつ processed_notes=totalnotes 必須。省略時 clear_type は hard_fail→1 / それ以外→5。`invalid_final` |
| `forfeit` | なし | 任意 | 準備段階（filling〜loading）は離脱→自分に `match_released`(reason `player_withdrew`)。countdown/playing は棄権→`forfeit_accepted` |
| `force_end_vote` | なし | `live.state=="playing"` 中、未 final のとき | 未完走の人間全員が投票すると `force_end_approved` → `result`。投票状況は `live.force_end`。`force_end_unavailable` |

その他（本書範囲外の概要のみ）: `arena_status`（状態要求）、`arena_manual`、`queue_entry`（ranked キュー）、`custom_catalog_request`、`cpu_chart_candidate`（サーバCPU用）、`chat_send`（`match_id`, `text`。応答の詳細は **未確認**）、`lobby_chat_send`。

## 3. サーバ→クライアント

### 3.1 ルーム系

**room_status**（部屋メンバー全員＋ルーム観戦者。room_entry / settings / ready / participation / ホスト移譲で配信）
```text
{type:"room_status", room_code:str, paused:bool(参加者0人), spectator_public:bool, room_name:str, locked:bool(パスワード有),
 rules:{match_mode:"private", room_code, room_name, room_host_id:int, spectator_public, score_rule, forced_gauge, chart_scope,
        ruleset_profile, series_format, first_to_wins, nomination_policy, nomination_seconds, option_seconds,
        intermission_seconds, force_host_option, ln_mode},
 players:[{player_id:int, name:str, host:bool, participating:bool, pending:bool(次シリーズから参加),
           ready:bool, room_matches:int, room_wins:int, room_losses:int, room_draws:int, room_win_rate:float}]}
```
`players` は入室順。`ready` はサーバ側の ready（`room_ready`/`state` で更新）。`allowed_play_modes`/`custom_selection` は含まれない（マッチの `rules` で届く）。

| type | フィールド | 送信条件 |
|---|---|---|
| `arena_status` | `player`{…,`queue`{`status`,`room_code`,`room_host_id`,`room_name`,`room_locked`,`room_participating`,`stay_in_room`,ルール各種,`ln_mode`…},`client`{…}}, `waiting_players`, `ranking`, `public_rooms`, `lobby_chat` 他 | 接続時・要求時・ルーム操作時。詳細構造は本書では概要のみ |
| `room_disbanded` | `match_id`（マッチ無しなら `""`）, `reason:"private_room_disbanded"` | ホストが `room_disband` |
| `room_kicked` | `room_code`, `player_id`（=自分） | ホストにキックされた本人へ |
| `room_closed` | `room_code`, `reason:"empty_room"` | **ルーム観戦接続にのみ**送られ、その接続は閉じられる（最後の参加者が退出） |
| `chat_history` | `match_id`, `room_code`, `messages`[] | 入室・再接続時（メッセージ要素の形は **未確認**） |

### 3.2 マッチ準備

`rules` オブジェクト（`match_reserved`/`chart`/`option_select`/`prepare`/`nomination_*`/`live`/`result` に同梱）:
```text
{match_mode, ruleset_profile, ln_mode, score_rule, forced_gauge, chart_scope, room_code, room_name, room_host_id,
 nomination_policy, nomination_seconds, option_seconds, intermission_seconds, series_format, first_to_wins,
 series_id, series_round:int, selector_player_id, spectator_public, force_host_option, allowed_play_modes:int[],
 custom_selection(公開用に整形), series_remaining_charts:int,
 series_standings:[{player_id, wins, points, exscore_total, max_exscore_total, placement:int|null}],
 rated:bool(ranked のみ true), rating_policy:"elo"|"none", season:{}(ranked のみ中身あり)}
```
`chart` オブジェクト: `{md5, exlevel:int, level:str, title, artist, totalnotes:int, target_band:int}`（譜面範囲により追加キーあり得る: **未確認**）。

| type | フィールド | 送信条件 |
|---|---|---|
| `match_reserved` | `match_id`, `client_flavor`, `ruleset_profile`, `ln_mode`, `message`("マッチングしました！"), `finish_current_song` bool（decide/play 中なら true）, `block_next_selection`:true, `reservation_timeout`(epoch), `rules` | 参加中（`room_participating`）で接続中・arena_enabled・画面状態 select/decide/play のメンバーが2人以上（最大8人）揃い待機マッチ（state `filling`）が作られたとき・再接続時。シリーズ次ラウンドは選曲画面に戻るまで遅延 |
| `players_updated` | `match_id`, `player_ids` int[] | マッチへの追加時（全員へ） |
| `fill_started` | `match_id`, `deadline`, `player_count`, `max_players` | 開始カウント開始。**private では参加者全員 ready で deadline=現在時刻**（即開始）。ready が崩れると `live.fill_deadline=null` で取消 |
| `match_released` | `match_id`, `reason`, `queue_retained` bool | 開始前にマッチから外された本人へ。reason: `player_withdrew`, `private_room_kicked`, `not_ready_at_fill_deadline`, `option_selection_incomplete`, `chart_missing_after_reroll`, `repeated_missing_chart_suspension`, `disconnect_timeout`, `web_withdrawal` 等 |
| `nomination_started` | 下記 nomination ペイロード | 指名フェーズ開始（再開時も同名）。**締切フィールド名は `deadline`** |
| `nomination_status` | 同上 | 誰かが指名/スキップするたび |
| `nomination_accepted` | `match_id`, `chart`, `submitted_count`, `required_count` | 自分の `chart_nominate` 成功時（本人のみ） |
| `nominations_revealed` | `match_id`, `nominations`[{`player_id`,`name`,`source`,`chart`}], `selected_player_id`(0=サーバ選曲), `chart`, `reroll_count`, `rules` | 譜面決定時（`chart` の直前）、再接続時 |
| `chart` | `match_id`, `chart`, `reroll_count`, `option`:"PLAYER_SELECTED", `ln_mode`, `load_deadline`, `random_seed`, `play_mode`(全員同一なら値、他 null), `play_mode_label`, `rules` | 譜面決定。受信したら `chart_check` を返す |
| `chart_candidate_rejected` | `match_id`, `chart_hash`, `reason`:"chart_missing", `missing_player_ids` int[] | free/custom/シリーズで誰かが譜面未所持→次候補へ（続けて新しい `nominations_revealed`+`chart`） |
| `option_select` | `match_id`, `chart`, `deadline`, `ready` bool（自分）, `ready_count`, `player_count`, `option`, `ln_mode`, `play_mode`, `play_mode_label`, `rules` | 全員の `chart_check` OK 後、および各 `option_ready` ごと（各自宛） |
| `prepare` | `match_id`, `chart`, `load_deadline`, `option`, `ln_mode`, `random_seed`, `play_option`（確定した自分のオプション。force_host_option 時はホストの値）, `play_mode`, `play_mode_label`, `rules` | 全員 option_ready（または締切で未送信者は play_option=0）後。受信したらロードして `play_ready` |
| `start` | `match_id`, `start_at`(現在+5秒), `option`, `ln_mode` | 全員 `play_ready` 後。`start_at` にプレイ開始 |
| `match_resume` | `match_id`, `state`(`countdown`\|`playing`), `chart`, `start_at`, `random_seed` | countdown/playing 中に再接続したとき |

nomination ペイロード（`nomination_started`/`nomination_status`）:
```text
{type, match_id, deadline:epoch|null, target_band:int, target_level:str, reference_rating, chart_scope,
 can_nominate:bool, required_count:int(自分の指名数ノルマ), submitted_count:int, retry_reason:str, rules,
 players:[{player_id, entry_order, name, submitted, required_count, submitted_count, host, selector, ready, wins}],
 your_nomination:chart|{}, your_nominations:chart[], your_source:""|"player_selected"|"server_random"}
```
ノルマ: `host` 方式=ホストのみ1、`rotate`=selector のみ1、`all`=全員1、`all_picks`=1、`first_to`=`first_to_wins` 件。
`retry_reason`（再指名の理由）: `free_selection_requires_nomination`, `free_selection_requires_unique_nomination`, `no_shared_free_selection_candidate`。
`nominations[].source`: `player_selected`, `room_candidate`（他人の候補を流用）, `server_random`, `series_pool`。

### 3.3 プレイ中・終了

**live**（サーバ→クライアントのスナップショット。C→S の `live` と同名なので向きで区別）: マッチ状態の変化のたびに全員と観戦者へ。
```text
{type:"live", match_id, client_flavor, state:"filling"|"selecting"|"options"|"loading"|"countdown"|"playing"|...,
 chart, play_mode, play_mode_label, start_at, ready_count, player_count, fill_deadline, max_players, rules,
 force_end:{available:bool, vote_player_ids:int[], required_player_ids:int[]},
 battle_scale:{}(minbp 時のみ中身), option_selection:{deadline,ready_count,player_count,ready_player_ids}|{},
 nomination:{deadline,target_band,target_level,reference_rating,players:[{player_id,submitted,submitted_count,required_count}]}|{},
 players:[{player_id,name,connected,reconnect_deadline,test_bot,host,selector,ready,series_wins,series_points,
           series_exscore_total,series_max_exscore_total,series_placement,exscore,processed_notes,minbp,max_combo,
           play_option,play_mode,play_mode_label,play_option_label,play_option_1p,play_option_2p,flip,
           battle_value,battle_max,battle_rate,finished,final_state,clear_type,clear_label}]}
```
`battle_value`: exscore→EX、minbp→totalnotes−BP、max_combo→MAXCOMBO。`battle_max`: exscore→2×totalnotes、他→totalnotes。

| type | フィールド | 送信条件 |
|---|---|---|
| `result` | `match_id`, `chart`, `players`[順位順: `placement`,`before`,`after`,`delta`(レート。非ranked/CPU は null or 欠落),`auto_reentered`,`exscore`,`processed_notes`,`final_state`(`complete`\|`hard_fail`\|`forfeit`\|`walkover`),`clear_type`,`clear_label`(棄権 "DNF", 不戦勝 "W.O."),`play_option`,`play_mode`,`play_option_label`,`minbp`,`max_combo`,`player_id`,`entry_order`,`name`,`play_mode_label`,`play_option_1p`,`play_option_2p`,`flip`,`battle_value`,`battle_max`,`battle_rate`,`finished`,`series_*`], `auto_reentry_player_ids`, `rated`, `rules`, `battle_scale`, `room_retained_player_ids`（部屋に残る人）, `series`（シリーズ時: `series_id`,`series_format`,`first_to_wins`,`round_winner_id`,`winner_player_id`,`complete`,`remaining_charts`,`standings`,…）, `return_to_select_at`（BO2 途中のみ、+15秒） | 全員 final（棄権/強制終了含む）時 |
| `forfeit_accepted` | `match_id`, `reason`（`player_withdrew`, `play_aborted`, `disconnect_timeout`, `final_result_timeout`, `private_room_kicked` 等） | 本人の棄権確定 |
| `force_end_approved` | `match_id`, `reason`:"unanimous_vote" | 強制終了成立（未完走者は hard_fail 扱い）→直後に `result` |
| `match_cancelled` | `match_id`, `reason`, （server_fault 時のみ `reentry_required`:true） | マッチ不成立。部屋のエントリーは原則維持（server_fault 時は取消され再入室が必要） |

`match_cancelled.reason`: `reservation_timeout`, `not_enough_ready_players`, `not_enough_connected_players`, `not_enough_players_after_withdrawal`, `not_enough_players_after_kick`, `not_enough_players_with_chart`, `not_enough_option_ready_players`, `load_timeout`, `loaded_longnote_contract_mismatch`（ロードした ln_mode/totalnotes が不一致）, `series_has_no_shared_chart`, `no_human_players`, `server_cpu_unavailable`, `cpu_has_no_owned_chart`, `cpu_chart_selection_timeout`, `server_fault`, `settlement_watchdog`。

## 4. シーケンス

### 4.1 通常のプライベートルーム1曲戦（single, policy=all）
| # | 方向 | メッセージ |
|---|---|---|
| 1 | A→S | `hello` → S→A `hello_ok`, `arena_status`, `arena_manual` |
| 2 | A→S | `state`{state:"select", arena_enabled:true} |
| 3 | A→S | `room_entry`{match_mode:"private", …設定…}（room_code なし）→ S→A `arena_status`(queue.room_code), `room_status` |
| 4 | B→S | `hello`…, `room_entry`{match_mode:"private", room_code, room_password?} → A,B に `arena_status`+`room_status` |
| 5 | S→A,B | 参加者2人以上で待機マッチ生成: `match_reserved`, `players_updated`, `live`(state:"filling") |
| 6 | A,B→S | `room_ready`{ready:true}（state=select であること）→ `room_status`, `live` |
| 7 | S→all | 全員 ready で `fill_started`(deadline=現在) |
| 8 | S→each | 次の tick（≤0.25秒）で `nomination_started`(deadline) + `live`(state:"selecting") |
| 9 | each→S | `chart_nominate`{match_id, chart_hash} → 本人 `nomination_accepted`, 全員 `nomination_status` |
| 10 | S→all | 全員提出または deadline 到達: `nominations_revealed`, `chart`(load_deadline=+120秒) |
| 11 | each→S | `chart_check`{match_id, chart_hash, available, totalnotes, play_mode}（不足→`chart_candidate_rejected` または再抽選） |
| 12 | S→each | 全員 OK: `option_select`(deadline=+option_seconds) + `live`(state:"options") |
| 13 | each→S | `option_ready`{match_id, play_option, play_mode, ln_mode} → 各自 `option_select`(ready_count 更新) |
| 14 | S→each | `prepare`(確定 play_option, random_seed, load_deadline) + `live`(state:"loading") |
| 15 | each→S | `play_ready`{match_id, chart_hash, ln_mode, totalnotes} |
| 16 | S→all | `start`(start_at=+5秒) + `live`(state:"countdown") |
| 17 | each→S | `live`(seq 1,2,…) ↔ S→all `live` スナップショット（state:"playing"） |
| 18 | each→S | `final`{state, clear_type, …} |
| 19 | S→all | 全員 final で `result`。`stay_in_room` なら部屋に残り、再び 5 から（試合後サーバ側 ready は false に戻るので再度 `room_ready` が必要） |

### 4.2 締切・タイムアウト（サーバ tick 0.25秒）
`reservation_timeout`=予約から300秒。nomination 締切で未提出者は他候補/サーバ選曲。option 締切で未送信者は `play_option=0`（chart_check で play_mode 未申告なら離脱）。`load_deadline`=120秒超過で `load_timeout`。切断は 30 秒の再接続猶予（`live.players[].reconnect_deadline`）。

### 4.3 シリーズ
`all_picks`/`first_to`/`bo2` は1ラウンドごとに `result`（`series` 付き）→ `intermission_seconds` 後に次ラウンドの `match_reserved`（新しい `match_id`）→ 以後は事前指名プールから自動で `chart` へ進む（2ラウンド目以降は `nomination_started` なしで直接 `nominations_revealed`/`chart`）。

### 4.4 再接続時の再同期（hello_ok 後）
| マッチ状態 | 送られるもの |
|---|---|
| マッチなし・部屋あり | `chat_history`（あれば） |
| filling | `match_reserved`, （deadline 設定済みなら）`fill_started` |
| selecting | `match_reserved`, `nomination_started` |
| options / loading | `match_reserved`, `nominations_revealed`, `chart`, `option_select` または（prepare 済なら）`prepare`, `live` |
| countdown / playing | `match_reserved`, `nominations_revealed`, `match_resume`, `live` |
| （共通） | マッチのチャット履歴があれば `chat_history` |

## 5. error code 一覧（`type:"error"`）
| code | 契機 |
|---|---|
| `hello_required`, `protocol_mismatch`, `unsupported_client_flavor`, `unsupported_client`, `unsupported_build`, `authentication_failed`, `client_flavor_conflict` | 接続時（送信後に切断） |
| `invalid_json`, `unknown_message` | 不正 JSON / 未対応 type |
| `arena_disabled` | arena_enabled=false で room_entry |
| `room_entry_failed`, `room_settings_failed`, `room_ready_failed`, `room_stay_unavailable`, `room_participation_failed`, `room_disband_failed`, `room_kick_failed`, `room_transfer_host_failed`, `room_set_selector_failed`, `queue_cancel_failed` | ルーム操作 |
| `nomination_closed`, `nomination_rejected`, `nomination_already_submitted` | 指名 |
| `invalid_chart_check`, `invalid_option_ready`, `invalid_play_ready`, `invalid_live`, `invalid_final`, `too_early` | 進行 |
| `force_end_unavailable` | 強制終了投票 |
| `queue_entry_failed`, `rated_ln_only`, `custom_catalog_failed`, `custom_selection_unavailable`, `cpu_chart_rejected`, `cpu_chart_unavailable`, `server_cpu_managed`, `chat_*`, `invalid_chat`, `spectator_limit` | 本書範囲外の機能 |

注: 状態や match_id が合わない `chart_check`/`option_ready`/`play_ready`/`live`/`final`/`forfeit` は **エラーを返さず黙って無視** されることが多い。

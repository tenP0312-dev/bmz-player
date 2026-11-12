# 仕様書：フローティングハイスピード（FHS）の印を BMZ から BMS-IR へ送る

最終更新：2026-11-12（Issue #1580 の「3. 仕様書」、調査の A6）

BMZ（`hyrorre/bmz-player`）へ移す機能の仕様です。実装はまだしていません。
実装の前に、BMZ の最新の `AGENTS.md` と対象の文書を読み直してください。本家の内容が優先します。

読んだ版：BMZ `54c9503`（0.5.1）、Arena oraja `9474d78`（0.4.14.129）、BMS-IR `main` の `2ad3bfd`。
どのクライアントもビルド・起動していません。コードと文書を読んだ結果です。

## 1. 目的と、利用者から見た動き

- BMS-IR の Web では、フローティングハイスピードを使ったプレイのオプション欄に `FHS` と出ます
  （ランキング、プレイヤーページ、プレイ履歴など）。今は Arena oraja のスコアだけに出ます。
- BMZ で Floating を使ったプレイにも、同じ `FHS` が出るようにします。
- BMZ 側に設定・画面・操作は足しません。利用者は何もしなくてよく、既定の動きも変わりません。
- この印は表示だけのものです。順位、ランプ、スコアの受け付けの条件は変わりません。

## 2. 細かい動きの決まり

### Arena oraja の動き（読み取った結果）

- プレイごとに「FHS を使った」という印を 1 つ持ちます。プレイの開始時は「使っていない」です。
- 曲が実際に進んでいる間（ロード中と READY は含まない）に、フローティング方式が有効な瞬間が
  1 度でもあれば「使った」になります。途中で通常方式へ戻しても「使った」のままです。
- コース・段位では、どれか 1 曲で「使った」なら、コース全体も「使った」になります。
- 印は、そのプレイの IR 送信にだけ付きます。手元のスコア DB には保存しません。
  DB から読み直したスコアを送るときは「使っていない」になります。
- 判定、スコア、IR へ送る条件は変えません。

### BMZ での決まり

| 場面 | 動き |
| --- | --- |
| 「使った」の定義 | プレイの状態が Playing の間に、HS の方式が Floating である瞬間が 1 度でもあれば「使った」。READY 中とロード中だけ Floating だった場合は「使っていない」 |
| 途中の切り替え | プレイ中に Floating へ切り替えた時点で「使った」。そのあと基準方式へ戻しても変わらない |
| HS を変えられないプレイ | コースの制約などで HS が固定されているプレイは Floating にならないので「使っていない」 |
| 送る値 | 新しいプレイの送信には、必ず `true` か `false` を入れる（3 章） |
| コース・段位 | どれか 1 曲で「使った」なら、コースの送信も「使った」 |
| 送信の対象外のプレイ | オートプレイ、リプレイ再生、プラクティスなどは、もともと BMS-IR へ送らない。変えない |
| 手元のスコアの取り込み（backfill） | 印を入れない（値が分からないため） |
| クイックリトライ | 新しいプレイとして、印を最初から数え直す |

注意：BMZ の HS 設定の既定は `CLASSIC+FLOATING` です。HS-FIX が `OFF` 以外なら、最初から
Floating で始まります。そのため、BMZ では多くのプレイに `FHS` が付く見込みです
（7 章の「決めてほしいこと」）。

## 3. BMZ での実装方針

### 印を数える場所

- HS の方式（Normal / Classic / Floating）は、`bmz-gameplay` の `GameSession` が持っています。
  プレイ中の更新は gameplay 専用スレッドがします。
- `GameSession` に「このプレイで Floating を使ったか」の値を 1 つ足し、フレームの更新で
  「状態が Playing で、方式が Floating」のときに立てます。立てたら下げません。
- プレイ終了時の写し（`bmz-player` の `screens/play_finish.rs` の終了スナップショット）へ
  この値を写し、IR の送信内容を作るところ（`ir/payload.rs` の送信コンテキスト）へ渡します。
- window スレッド側で HS の方式を見て数える方法は取りません。フレームを落としたときに
  数え漏れるためです。

### 送る形

- 単曲：送信内容の `play_options` に `floating_hispeed`（真偽値）を足します。
- コース：コースの送信内容（`ir/course_payload.rs`）の `play_options` にも同じキーを足します。
  コースの進行（`screens/course_session.rs`）で、曲ごとの値を「どれかが真なら真」でまとめます。
- BMZ の送信内容は、すべての IR で共通の 1 つの JSON です（provider ごとに作り分けていません）。
  そのため、このキーは BMS-IR だけでなく、BMZ 公式 IR と rianIR 向けの送信にも入ります。
  - BMZ 公式 IR：`docs/ir.md` に「`play_options` は既存のキーを保持する」とあり、上限は 32 KiB です。
    知らないキーで拒否する処理は見つかりませんでした。値は保存されるだけです。
  - rianIR：BMZ 側で必要な項目だけを取り出して別の形に直すので、このキーは送られません。
- 調査では「BMS-IR の provider の中だけで済む見込み」と書きましたが、これは誤りでした。
  provider は出来上がった送信内容を受け取るだけで、プレイ中の HS の方式を知りません。
  共通の送信内容に 1 項目足す変更になります。

### BMZ の AGENTS.md の決まりとの対応

| 決まり | 対応 |
| --- | --- |
| crate の責任 | 数えるのは `bmz-gameplay`（session）。送信内容は `bmz-player` の `ir/`。描画・スキンには触らない |
| IR の境界 | 本体の中心に BMS-IR 専用の処理を足さない。足すのは、どの IR でも意味が通じる中立の項目 1 つ |
| 文書の更新 | `docs/ir.md` の `play_options` の説明に `floating_hispeed` を足す。`docs/hs.md` に 1 行足す |
| 6 言語の翻訳 | 画面の文字を足さないので不要 |
| テスト | 5 章 |
| 確認 | 2 つの crate を変えるので、workspace 全体の `check` / `clippy` / `test` |
| コミット | Conventional Commits。scope は `bmz-gameplay,bmz-player` |

## 4. 既存の動作を変えないことの確認点

- 判定・スコア・ゲージ：足すのは値 1 つと、それを立てる処理だけ。判定の経路に入れない。
- スコアを分ける単位（score identity）：変えない。手元の `score.db` の形も変えない。
- リプレイ：リプレイの形式に足さない。リプレイ再生は送信しないので影響しない。
- G-BATTLE：スコアは通常の区分のまま送る。印は自分のプレイの HS だけで決まる。
- 送信キュー：すでにキューに入っている古い送信内容にはキーがない。サーバーは「分からない」
  として何も表示しない（今と同じ）。
- ほかの IR：BMZ 公式 IR への送信内容に項目が 1 つ増える。順位・検証に使われないことを、
  `bmz-ir-web` のテスト（`bun run test:ir`）で確かめる。
- 冪等性：`idempotency_key` と署名の対象は、キューへ入れる前に作った送信内容。印もその時点で
  決まっているので、再送で値が変わらない。

## 5. テストの観点と、手での確認

### 単体テスト（BMZ）

- READY の間だけ Floating → 「使っていない」。
- Playing の間ずっと Floating → 「使った」。
- Playing の途中で Floating へ切り替え、すぐ戻す → 「使った」。
- 基準方式のまま最後まで → 「使っていない」。
- HS を変えられないプレイ → 「使っていない」。
- 送信内容：`play_options.floating_hispeed` が `true` / `false` で入る。backfill には入らない。
- コース：3 曲のうち 1 曲だけ「使った」→ コースの送信は `true`。
- rianIR 向けの変換結果が変わらない。

### 手での確認（オペレーター向け）

BMS-IR 側の変更（6 章）が本番に入ってから行います。

1. BMZ の HS CONFIG を `CLASSIC`（Floating なし）にして 1 曲プレイし、BMS-IR へ送る。
   Web のその曲のランキングで、自分の行に `FHS` が**出ない**ことを見る。
2. HS CONFIG を `FLOATING` にして同じ曲をプレイし、送る。自分の行に `FHS` が**出る**ことを見る。
3. HS CONFIG を `CLASSIC+FLOATING`、HS-FIX を `OFF` にして始め、プレイ中に E1 + E2 で Floating へ
   切り替える。送ったスコアに `FHS` が出ることを見る。
4. 3 つとも、順位・ランプ・スコアが `FHS` の有無で変わっていないことを見る。

## 6. BMS-IR 側に必要な変更（別の PR）

確かめた結果：**今の BMS-IR の BMZ 用 API は、この印を受け取りません。**

- BMZ 用のスコア API は、送られてきた `play_options` から決まった項目だけを取り出します
  （`tools/lr2ir_server/bmz_player_adapter.py`）。`floating_hispeed` は取り出していません。
  今 BMZ がこのキーを送っても、無視されるだけで、拒否はされません。
- 一方、取り出したあとの共通の保存処理（`tools/lr2ir_server/score_submission_protocol.py`）は、
  `floating_hispeed` という項目があれば `1` / `0` に直して保存します。Web の表示
  （`web_score_components.py` の `FHS`、ランキング・プレイヤーページ・履歴）は、
  保存された印を読むだけで、クライアントの種類で分けていません。

必要な変更：

1. BMZ 用の変換（単曲とコースの両方）で、`play_options.floating_hispeed` を読み、共通の保存処理へ
   渡す。真偽値のほか、分からない値は「印なし」として扱う（今の共通処理と同じ決まり）。
2. `tools/test_bmz_player_score_adapter.py` に、`true` / `false` / キーなし / 変な値の 4 通りを足す。
3. `docs/BMZ_PLAYER_NATIVE_IR_JA.md` の送信内容の説明に、このキーを足す。
4. `docs/IMPLEMENTED_BEHAVIOR.md` の該当する節に、BMZ のスコアにも `FHS` が出ることを足す。

進め方：

- BMS-IR 側は小さい PR 1 つです。本番への反映（deploy）は、オペレーターの明示の依頼が要ります。
- 順番はどちらが先でも壊れません。サーバーが先なら、BMZ が送り始めた時点で表示されます。
  BMZ が先なら、サーバーが対応するまで無視されます。サーバーを先にするのをおすすめします。
- 公開の更新記録（changelog）は、BMZ 側の配布で利用者に見えるようになった時点で考えます。

未確認：

- BMZ のスコアのうち、CN / HCN、Battle、Battle AS の区分の行は、Web の通常のランキングとは別の
  読み方をします。それらの行のオプション欄に `FHS` が出るかは確かめていません。
  BMS-IR 側の PR で、表示の経路を 1 つずつ確かめます。

## 7. 調査で「未確認」だった項目の確認結果

| 項目 | 結果 |
| --- | --- |
| A6：BMZ 用の API がこの印を受け取れるか | **受け取れない**（6 章）。BMS-IR 側に小さな変更が要る |
| A6：BMZ が送る項目に足せるか | 技術的には足せる。共通の送信内容に入るので、BMZ 公式 IR にも届く。本家が受け入れるかは**未確認**（PR で判断してもらう） |

## 8. 見積もりと PR の分け方

| PR | 場所 | 中身 | 見積もり |
| --- | --- | --- | --- |
| 1 | BMS-IR | BMZ 用の変換で印を受け取る、テスト、文書 | 小 |
| 2 | BMZ（フォーク → 本家） | 印を数える、送信内容へ足す、テスト、`docs/ir.md` と `docs/hs.md` | 小 |

### 決めてほしいこと

1. **「使った」の定義**。BMZ は既定が `CLASSIC+FLOATING` なので、多くのプレイに `FHS` が付きます。
   Arena oraja と同じ「プレイ中に 1 度でも Floating なら付ける」でよいか。
2. **共通の送信内容に足すこと**。このキーは BMZ 公式 IR への送信にも入ります。本家の PR の説明に
   はっきり書いて出す進め方でよいか（①〜③は相談なしで進める決定ですが、ここだけ本家の IR に関わります）。
3. **BMS-IR 側の PR と本番への反映の時期**。サーバーを先に反映する順番でよいか。

## 参照

- BMZ：`AGENTS.md`、`docs/hs.md`、`docs/ir.md`、`crates/bmz-gameplay/src/session/`、
  `crates/bmz-player/src/ir/payload.rs`、`ir/course_payload.rs`、`ir/bms_ir.rs`、`screens/play_finish.rs`
- Arena oraja：`docs/FLOATING_HISPEED.md`、FHS の印のテスト（`BMSIRFloatingHispeedScoreTest`）
- BMS-IR：`docs/BMZ_PLAYER_NATIVE_IR_JA.md`、`tools/lr2ir_server/bmz_player_adapter.py`、
  `score_submission_protocol.py`、`play_options.py`、`web_score_components.py`
- `docs/BMZ_MIGRATION_SURVEY.md`（調査）、Issue #1580

# BMZ フォーク：LR2・oraja スキンの互換性の調査（1 段目）

Issue #1586（親 #1585 の「3. LR2 と oraja のスキンを完全に動かす」）の 1 段目です。
コードと仕様だけを読んで調べました。実物のスキンを BMZ で動かしての確認はしていません。

- 調べた日：2026-11-12
- BMZ：`tenP0312-dev/bmz-player` の main（`54c9503c`、本家 `hyrorre/bmz-player` と同じ main）
- 元のプレイヤー：`tenP0312-dev/oraja`（Arena oraja）の main（`9474d787`）
- oraja のコードは、差を見つけるためだけに読みました。コードは書き写していません。
- この文書は調査結果です。BMZ・本家・oraja には何も書き込んでいません。

## 読んだもの

| 区分 | ファイル |
|---|---|
| BMZ の作業方針 | `AGENTS.md`（ルート） |
| BMZ の互換の記録 | `docs/skin-compatibility.md`（BMZ 自身の対応表。最終確認日 2026-10-06）、`docs/skin.md`（約 3,100 行の仕様）、`notes/2026/*lr2*` |
| BMZ の LR2 読み込み | `crates/bmz-skin/src/lr2.rs`、`src/lr2/builder_core.rs`（命令の振り分け）、`builder_select.rs`、`builder_result.rs`、`builder_play.rs`、`processor.rs`（`#IF`）、`refs.rs` |
| BMZ の JSON 形式 | `crates/bmz-skin-document/src/schema/document.rs`、`load.rs`、`property_names/generated.rs` |
| BMZ の Lua | `crates/bmz-skin/src/lua/`（`runtime.rs`、`sandbox/`、`conversion/`） |
| BMZ の描画 | `crates/bmz-render/src/skin/`（`state_values/`、`document_render/`、`pm_chara.rs`） |
| BMZ のスキン設定 | `crates/bmz-player/src/config/profile_config/skin_ir.rs`、`src/skin_loader/decode/source.rs` |
| BMZ のテスト | `crates/bmz-skin/src/lr2/tests/`、`src/tests/`、`crates/bmz-player/src/skin_loader/tests/`（`lr2/`・`lua/`） |
| oraja の LR2 読み込み | `core/src/bms/player/beatoraja/skin/lr2/`（`LR2SkinCSVLoader.java`、`LR2PlaySkinLoader.java`、`LR2SelectSkinLoader.java`、`LR2ResultSkinLoader.java`、`LR2CourseResultSkinLoader.java`、`LR2SkinSelectSkinLoader.java`） |
| oraja の JSON・Lua | `skin/json/JsonSkin.java`、`skin/lua/MainStateAccessor.java`、`LegacySkinLuaApi.java`、`SkinLuaRestrictedLibs.java` |
| oraja の値の番号 | `skin/SkinProperty.java`、`skin/SkinType.java` |
| oraja の記録 | `docs/ED_FEATURE_PARITY.md`、`docs/SKIN_TEXT_GRADIENT.md`、`docs/SKIN_SCENE_FORMAT.md`、`doc/BMSIR_ARENA.md` |
| BMS-IR の docs | `docs/LR2_REAL_SPEC_ARCHIVE_AUDIT_2026-06-08.md`、`docs/BMZ_MIGRATION_SURVEY.md` |

BMS-IR の docs には、LR2 スキン（CSV 命令）の仕様の記録はありませんでした。
`docs/LR2_REAL_SPEC_ARCHIVE_AUDIT_2026-06-08.md` は判定・ゲージの仕様で、スキンの記述はありません。
`docs/lr2_specs_archive/` はこのリポジトリにありません。
そのため LR2 の「本来の動き」は、oraja の LR2 読み込みと BMZ 自身の記録だけで比べています。
LR2 本体の動きと直接は比べていません（未確認）。

## まとめ

- BMZ は LR2 の CSV、beatoraja の JSON、Lua の 3 形式をすべて読みます。
  対応の範囲は広く、LR2 の命令は oraja が持つ命令のほぼすべてを扱います。
- BMZ は LITONE5、W-MIX、ECSE、3R、PLATINUM、ECBE、Luxe Flat、SocialSkin、ModernChic などで
  個別の互換修正を重ねています（BMZ の `docs/skin-compatibility.md` と `notes/`）。
- 差は **31 件** 見つけました（下の表）。
  - LR2 の CSV：12 件
  - JSON・Lua の両方：9 件
  - Lua だけ：3 件
  - Arena oraja だけにある拡張：7 件
- いちばん大きいのは「LR2 スキンのポップンキャラ（`PM_CHARA`）が CSV では出ない」と
  「LR2oraja ED の数値 378 / 379 が無い（LITONE が使う）」です。
- Arena だけの拡張（レーダー、Skin Scene、文字の縦グラデーション等）は、
  BMZ に同じ名前で入れるかどうかをオペレーターに決めてもらう必要があります。

## 1. BMZ が対応している形式・命令・機能

### スキンの形式と画面

| 項目 | BMZ | 根拠 |
|---|---|---|
| LR2 CSV（`.lr2skin` / `.csv`） | 読む | `bmz-skin/src/lr2.rs` |
| beatoraja JSON（`.json`） | 読む。コメント・カンマの省略・Shift_JIS も受ける | `bmz-skin-document/src/load.rs`、BMZ `docs/skin.md`「JSONの構文と文字コード」 |
| Lua（`.luaskin` / `.lua`） | 読む。Lua 5.4 を使い、LuaJ（Lua 5.2）の動きに寄せている | `bmz-skin/src/lua/` |
| 画面：選曲・決定・プレイ・リザルト・コースリザルト | 対応 | `config/profile_config/skin_ir.rs`（`select` / `decide` / `play*` / `result` / `course_result`） |
| プレイ：5 / 7 / 9 / 10 / 14 鍵、5 / 7 鍵 BATTLE | 対応 | 同上（`play5`〜`play14`、`battle5`、`battle7`） |
| プレイ：4 / 6 / 8 鍵 | 対応（BMZ 独自、type 22〜24） | BMZ `docs/skin.md`「Skin Type」 |
| 画像の形式 | PNG / BMP / JPEG / GIF / TGA / WebP / CIM、動画（mp4・wmv・mpg・avi 等） | `skin_loader/decode/source.rs`、BMZ `docs/skin-compatibility.md`「画像sourceの形式」 |
| 旧 DXA 書庫 | v1〜v4 | `bmz-skin-assets/src/dxa.rs` |

### LR2 CSV の命令

`crates/bmz-skin/src/lr2/builder_core.rs` の振り分けと、`builder_select.rs`・`builder_result.rs` を見ました。
oraja の `addCommandWord` の一覧と突き合わせた結果、次の命令は BMZ でも扱います。

- 部品：`#SRC_/DST_IMAGE`、`BUTTON`、`ONMOUSE`、`IMAGESET`、`NUMBER`、`TEXT`、`SLIDER`、`SLIDER_REFNUMBER`、
  `BARGRAPH`、`BARGRAPH_REFNUMBER`、`BGA`、`LINE`、`JUDGELINE`、`HIDDEN`、`LIFT`、`GROOVEGAUGE`（`_EX` 含む）
- ノーツ：`NOTE`、`LN_*`、`HCN_*`、`MINE`、`AUTO_*`、`DST_NOTE2`、`DST_NOTE_EXPANSION_RATE`、`#HORIZONTAL`
- 判定とコンボ：`NOWJUDGE_1P/2P/3P`、`NOWCOMBO_1P/2P/3P`
- グラフ：`NOTECHART`（`_1P`）、`BPMCHART`、`TIMING_1P`、`GAUGECHART_1P/2P`、`SCORECHART`
- 選曲バー：`BAR_BODY_ON/OFF`、`TITLE`、`LEVEL`、`LAMP`、`MY_LAMP`、`RIVAL_LAMP`、`RIVAL`、`FLASH`、
  `LABEL`、`TROPHY`、`GRAPH`（type 0 だけ）、`BAR_CENTER`、`BAR_AVAILABLE`
- 見出し・設定：`#INFORMATION`、`#CUSTOMOPTION`、`#CUSTOMOPTION_ADDITION_SETTING`、`#CUSTOMFILE`（`|1P|` 等のタグ付きを含む）、
  `#CUSTOMOFFSET`、`#RESOLUTION`（省略時は推定）、`#INCLUDE`（ファイル名の `*` を含む）、`#SETOPTION`
- 分岐：`#IF` / `#ELSEIF` / `#ELSE` / `#ENDIF`（入れ子、CSV ごとに独立したスタック）
- 素材：`#IMAGE`、`#FONT`、`#LR2FONT`、`#TRANSCOLOR`（綴りの誤り 2 種を含む）、`#CONTINUE`
- 時間：`#STARTINPUT`、`#SCENETIME`、`#FADEOUT`、`#CLOSE`、`#LOADSTART`、`#LOADEND`、`#PLAYSTART`、
  `#JUDGETIMER`、`#FINISHMARGIN`
- その他：`#STRETCH`、`#FLIPRESULT`、`#DISABLEFLIP`、`#SCRATCHSIDE`（読み飛ばし）
- 合成：`blend=3`（減算）、スライダーの `blend=0`

知らない `#SRC_*` / `#DST_*` は警告を出して読み飛ばします（`builder_core.rs` の最後の分岐）。

参照できる値は、BMZ の `docs/skin.md`「LR2のnumber / option」「LR2のSelect / Result」に
LR2 で意味が違う番号（POOR、ランク差、選曲の IR 状態 600〜608 など）を分けて書いてあります。
番号ごとの全件の照合はしていません（未確認）。

### JSON の項目

`schema/document.rs` の項目と oraja の `JsonSkin.java` の項目を比べました。BMZ が読む項目：

`category`、`property`、`filepath`、`offset`、`source`、`font`、`image`、`imageset`、`value`、`text`、
`slider`、`graph`、`gaugegraph`、`judgegraph`、`bpmgraph`、`hiterrorvisualizer`、`timingvisualizer`、
`timingdistributiongraph`、`hiddenCover`、`liftCover`、`note`（`dst2`・`expansionrate`・`group`・`bpm`・`stop`・`time` を含む）、
`gauge`、`judge`、`pmchara`、`bga`、`songlist`、`destination`（`timer`・`op`・`draw`・`offsets`・`stretch`・`center`・クリップ）、
`customEvents`、`customTimers`。

名前による値の参照（`value: "score"` のような書き方）は、beatoraja の参照ソースから作った表で解決します
（`property_names/generated.rs`、beatoraja `d22ce10b` から生成）。

### Lua の機能

BMZ の `docs/skin-compatibility.md`「Luaの推論とruntime」と `bmz-skin/src/lua/` を見ました。

- 関数の項目（`draw`、`value`、`timer`、`act`）は、読み込み時に推論できればそのまま、
  できなければ実行時に Lua で評価します（`--lua-skin-runtime compat`）。
- `main_state` の主な関数（`number`、`float_number`、`text`、`option`、`timer`、`set_timer`、`event_exec`、
  `rate`、`exscore_*`、`gauge`、`audio_*`、`volume_*` など）、`timer_util`、`skin_config`、`bit32`。
- `luajava` は、ファイル読み込みだけの代わりの関数で受けます（`lua/sandbox/environment.rs`）。
- `os` / `io` は限られた範囲だけ。書き込みはディスクに書かず、メモリに残します。
- 命令数・メモリ・テーブル量に上限があります（BMZ の設計の約束。`AGENTS.md`）。

oraja の `MainStateAccessor.java` などが Lua に出している 88 個の名前を BMZ のコードで探し、
見つからなかったのは 19 個でした。内訳は下の差の表（L-1〜L-3、A-4）に入れました。

## 2. 元のプレイヤーとの差

記号の意味：

- 影響：**大**＝多くのスキンが使う／画面が大きく崩れる、**中**＝一部の人気スキンが使う、**小**＝使うスキンが少ない
- 難しさ：**小**＝数日、**中**＝1〜2 週間、**大**＝それ以上、または作りの変更が要る（目安）
- 「BMZ のファイル」「oraja のファイル」は根拠として見た場所です。

### LR2 CSV（12 件）

| # | 差 | 影響 | 難しさ | BMZ の設計とぶつかるところ | 根拠 |
|---|---|---|---|---|---|
| R-1 | `#SRC_PM_CHARA_IMAGE`、`#DST_PM_CHARA_1P/2P/ANIMATION/IMAGE` を扱わない。LR2 のプレイスキンでポップンキャラが出ない | 中（9 鍵の LR2 スキンでよく使う。7 鍵では少ない） | 中（JSON の `pmchara` の描画は既にあるので、つなぐだけで済む見込み） | なし。JSON 用の `pm_chara` の仕組みを使える | BMZ：`lr2/builder_core.rs`（命令が無く警告で飛ばす）、`bmz-render/src/skin/pm_chara.rs`。oraja：`lr2/LR2PlaySkinLoader.java` |
| R-2 | 選曲バー `BAR_RANK`（IR ランキング一覧の装飾）を扱わない | 小〜中 | 中 | BMZ は「同じ機能が無いので対応しない」と決めている（BMZ `docs/skin.md` 260 行付近） | BMZ：`docs/skin.md`「LR2のSelect / Result」。oraja：`LR2SelectSkinLoader.java` |
| R-3 | `BAR_GRAPH` の type 1（スコアランクの分布、28 種類）が出ない。読み込み時に警告 | 小〜中 | 小 | なし | BMZ：`docs/skin.md`、`lr2/builder_select.rs`。oraja：`LR2SelectSkinLoader.java` |
| R-4 | `#SRC_README` / `#DST_README`（選曲画面に文書を埋め込む）を扱わない | 小 | 中 | BMZ は文書表示を自分の画面で行う方針 | BMZ：`docs/skin.md` 490 行付近。oraja：`LR2SelectSkinLoader.java` |
| R-5 | スキン選択画面（type 9）と `#SAMPLEBMS`、キー設定画面（type 8）のスキンが使えない | 小（使うスキンは少ない） | 大（BMZ の設定画面は egui で作られている） | BMZ はキー設定・スキン選択を egui の自前画面で行う。スキン化すると二重になる | BMZ：`config/profile_config/skin_ir.rs`（項目なし）、`docs/skin.md` 370 行。oraja：`LR2SkinSelectSkinLoader.java`、`SkinType.java` |
| R-6 | 9 鍵 BATTLE（type 14）・24 鍵（type 16〜18）のプレイスキン | 小 | 中（9 鍵 BATTLE）〜大（24 鍵は鍵盤の種類から無い） | BMZ の `KeyMode` に 24 鍵が無い | BMZ：`bmz-core/src/lane.rs`、`skin_ir.rs`。oraja：`SkinType.java` |
| R-7 | 選曲の FX / EQ / 音の設定パネル、組み合わせのキーフィルター、フォルダの並べ替え、特殊な S-RANDOM のボタン | 中（LR2 の選曲スキンのパネル 2 以降で使う） | 中〜大 | BMZ に同じ設定が無いものは押しても無反応、という方針 | BMZ：`docs/skin.md` 488〜489 行、`notes/2026/2026-10-13-lr2-embedded-panels-skin-settings.md` |
| R-8 | 数値：レベルバー、ゴーストの進み具合、ライバルの IR 順位（292 / 293） | 小〜中 | 小〜中（値を作る所が要る） | なし | BMZ：`docs/skin-compatibility.md`「LR2のnumber / option」、`docs/skin.md` 259 行 |
| R-9 | リザルトの `SCORECHART` の index 1 / 2（過去ベスト・ライバルのグラフ）、対戦相手の確定値・履歴 | 中（リザルトの見た目に出る） | 中（ベストのプレイ途中の記録が要る） | BMZ のスコア保存に途中経過が無いと作れない（未確認） | BMZ：`docs/skin.md` 491 行 |
| R-10 | LR2 の描画順の入れ替え（別の部品の DST をアニメの途中に挟み、前後が変わる動き）を再現しない | 小〜中（スキンにより崩れ方が違う） | 大 | BMZ は宣言順で描画する作り | BMZ：`docs/skin.md` 338 行 |
| R-11 | `#FONT` の文字の太さ・アンチエイリアスが DxLib と違う。`#HORIZONTAL` のカバーの横向き切り取りが無い | 中（文字の見た目は多くの LR2 スキンで差が出る） | 中〜大 | BMZ は文字を自前のアトラスで描く | BMZ：`docs/skin.md` 537・543 行 |
| R-12 | 7→9 鍵の変換イベント（360 / 361） | 小 | 小〜中 | BMZ に 7→9 の変換機能が無い（未確認） | BMZ：`docs/skin.md` 2006 行 |

### JSON・Lua の両方（9 件）

| # | 差 | 影響 | 難しさ | BMZ の設計とぶつかるところ | 根拠 |
|---|---|---|---|---|---|
| J-1 | 数値 378（プレイした日）と 379（落ちた小節）が無い。LR2oraja Endless Dream の機能で、LITONE が使う | 中（LITONE は人気） | 小 | なし。ただし BMZ の IR ランキングの番号が 380 からなので、範囲を間違えないこと | BMZ：`bmz-render/src/skin/state_values/number/`（378 / 379 なし）。oraja：`SkinProperty.java`（`NUMBER_SCORE_DATE`、`NUMBER_FAIL_MEASURE`）、`docs/ED_FEATURE_PARITY.md` 14 番 |
| J-2 | `floatvalue`（小数の数値部品）を読まない | 中（新しめの beatoraja スキン） | 小〜中 | なし。BMZ の `docs/skin.md` 765 行に「新しいスキーマは含まない」とある | BMZ：`schema/document.rs`（項目なし）。oraja：`JsonSkin.java` `FloatValue` |
| J-3 | ポップンキャラのタイマー 900〜909 を、スキンが直接参照できるかが分からない | 小〜中 | 小 | BMZ はキャラの動きを内部で選ぶ | BMZ：`bmz-render/src/skin/pm_chara.rs`（900〜909 を探して見つからず）。oraja：`SkinProperty.java` `TIMER_PM_CHARA_*`。**動きは未確認** |
| J-4 | ポップンキャラの `--` 座標補間と、右下の色を透過色にする処理 | 小〜中 | 小〜中 | なし | BMZ：`docs/skin.md`「PMchara」 |
| J-5 | `font.type`（1 / 2、距離場フォント）を読むが描画に使わない。ModernChic・MILLIONDOLLAR が該当 | 中（文字の縁・光り方が違って見える） | 中 | BMZ の文字アトラスとは別の描き方が要る | BMZ：`docs/skin-compatibility.md`「品質改善案」 |
| J-6 | 文字の縁取りが 8 方向に描く近似 | 中（多くのスキンで縁が少し違う） | 中 | 同上 | 同上 |
| J-7 | ゲージの `parts` を作り直す時機、モード切り替え時のボーダーの位置、Lua の `P1_grooveflash` 等の固定位置の光り | 中（プレイ画面の見た目） | 未確認 | 未確認 | BMZ：`docs/skin-compatibility.md`「未検証の互換差分」 |
| J-8 | DXA の v5 以降・独自キー・書庫内の動画/音/CSV | 小（古い LR2 スキン） | 中 | なし | BMZ：`docs/skin-compatibility.md`「一部対応・未対応」 |
| J-9 | 曲バーの Lua `draw` をクリック判定に使わない | 小 | 中 | 行ごとに条件を評価する作り | 同上 |

### Lua だけ（3 件）

| # | 差 | 影響 | 難しさ | BMZ の設計とぶつかるところ | 根拠 |
|---|---|---|---|---|---|
| L-1 | Lua 5.4 と LuaJ の差：小数の `tostring`、`string.format("%d")` の小数、`os.date` の既定の書式、`nil` を 0 にしない | 中（数字の表示が崩れるスキンがある） | 中 | BMZ は Lua 5.4 を使う。LuaJ に完全に合わせるのは難しい | BMZ：`docs/skin-compatibility.md`「LuaJとLua 5.4の差」 |
| L-2 | HTTP 通信（`luajava` の HTTP の代わりの関数、`open_url`）が無い。失敗の値を返すだけ | 小（SocialSkin 等） | 中 | BMZ は Lua からの通信を許さない安全の方針 | BMZ：`docs/skin.md` 1284 行。oraja：`LegacySkinLuaApi.java`（`connect` 等） |
| L-3 | `os.execute` / `exit` / `getenv` / `remove` / `rename` / `tmpname`、`debug.getmetatable` | 小 | ― | oraja も `os.execute` 等は消している（`SkinLuaRestrictedLibs.java`）。差は害が無い見込み | BMZ：`lua/sandbox/`。oraja：`SkinLuaRestrictedLibs.java`、`LegacySkinLuaApi.java` |

### Arena oraja だけにある拡張（7 件）

これらは beatoraja 本家・LR2oraja には無く、Arena oraja が足したものです。
「元のプレイヤーと同じ」を Arena oraja まで含めるかどうかで、扱いが変わります。

| # | 差 | 影響 | 難しさ | BMZ の設計とぶつかるところ | 根拠 |
|---|---|---|---|---|---|
| A-1 | 譜面傾向のレーダー（`radargraph`、数値 1330〜1335） | 小（同梱スキンもまだ使っていない） | 中 | 番号の範囲が BMZ の独自の番号と重ならないか確認が要る（BMZ の option 1330〜1335 とは種類が違う） | oraja：`SkinRadarGraph.java`、`SkinProperty.java`、`doc/BMSIR_ARENA.md` 0.4.14.22 |
| A-2 | Skin Scene v1（`scenes`、`camera`、3D の変形・マスク） | 小 | 大 | 描画の作りがまったく違う。BMZ の Ambient 等の独自拡張と重なる | oraja：`skin/scene/`、`docs/SKIN_SCENE_FORMAT.md`。`docs/BMZ_MIGRATION_SURVEY.md` の S3 |
| A-3 | 文字の縦グラデーション：Arena は `gradientTopColor` / `gradientBottomColor`、BMZ は `colorTop` / `colorBottom` / `gradientAngle` | 小 | 小 | **同じ機能で項目名が違う**。どちらかに寄せるか、両方を受けるかを決める必要がある | oraja：`docs/SKIN_TEXT_GRADIENT.md`。BMZ：`docs/skin.md`「Text Gradient」 |
| A-4 | Arena の Lua 関数：`play_hispeed_margin`、`start_here_preview_*`、`bmsir_judge_timing_restore_*`、`play_key_fast/slow`、`play_scratch_fast/slow` | 小（Arena 同梱スキンだけ） | 小〜中 | 機能自体（Start Here プレビュー等）が BMZ に無い | oraja：`skin/lua/MainStateAccessor.java` |
| A-5 | Arena の option 2900（判定タイミング復元） | 小 | 小 | 同上 | oraja：`SkinProperty.java` |
| A-6 | Arena の数値（レーンカバーの表示時間 1312〜1327 など）は BMZ にも番号がある。意味が同じかは未確認 | 小 | 小 | ― | BMZ：`state_values/number/`。oraja：`SkinProperty.java` |
| A-7 | Arena の JSON にある `skinpreview` / `customBMS`（スキン設定画面のプレビュー用） | 小 | 中 | R-5 と同じ | oraja：`JsonSkin.java` `SkinConfigurationProperty` |

### 番号の照合について

oraja の `SkinProperty.java` の番号（NUMBER / OPTION / TIMER / STRING で約 900 個）を、
BMZ の名前表（`property_names/generated.rs`）と機械的に比べました。
名前表は「名前で書いたときの読み替え表」で、番号で書いた場合の対応とは別です。
そのため表に無い番号が「未対応」とは限りません。上の表には、BMZ の値の計算の場所
（`bmz-render/src/skin/state_values/`）でも見つからなかったものだけを入れました。
番号ごとに意味まで合っているかは確かめていません（未確認）。

## 3. 直す順番の案（上位 13 件）

影響が大きく、BMZ の設計とぶつからず、小さく直せるものを先にしました。

| 順 | # | 理由 |
|---:|---|---|
| 1 | J-1 数値 378 / 379 | LITONE が使う。小さい |
| 2 | R-1 LR2 の `PM_CHARA` | 9 鍵の LR2 スキンの見た目が大きく変わる。JSON 用の描画を使える |
| 3 | J-4 ポップンキャラの補間と透過色 | R-1 と一緒に直すと、LR2 のキャラがそのまま出る |
| 4 | J-3 タイマー 900〜909 の確認 | R-1 と同じ所。まず動きを確かめる |
| 5 | R-3 `BAR_GRAPH` type 1 | 小さい。警告が出なくなる |
| 6 | J-2 `floatvalue` | 新しい beatoraja スキンで値が出ない |
| 7 | J-7 ゲージの `parts`・光り | プレイ画面で目に付く。2 段目の見比べで原因を決める |
| 8 | R-11 LR2 の文字の見た目 | 多くの LR2 スキンで差が出る。ただし完全一致は難しい |
| 9 | J-6 文字の縁取り | 多くのスキンで少し違う |
| 10 | J-5 距離場フォント | ModernChic で目立つ |
| 11 | L-1 Lua 5.4 と LuaJ の差 | 数字の表示が崩れる例がある |
| 12 | R-9 リザルトの `SCORECHART` index 1 / 2 | リザルトの見た目。保存の作りを確かめてから |
| 13 | R-8 レベルバー・ゴースト進捗・ライバル IR 順位 | 値を作る所が要る |

A-3（グラデーションの項目名）は、直す量は小さいですが、オペレーターの判断が先です。
R-5・R-7・R-10・A-2 は大きいので、上位には入れていません。

## 4. 2 段目の手順の案（オペレーターの PC で見比べる）

2 段目は、オペレーターが自分の PC で行います。エージェントはプレイヤーを起動・操作しません。
スキンの素材（画像・音・フォント）とスクリーンショットは、このリポジトリに入れません。

### 用意するもの

1. BMZ（フォークの main をビルドしたもの）と、元のプレイヤー
   - LR2 スキン：LR2 本体（あれば）と Arena oraja（または LR2oraja）
   - beatoraja スキン：Arena oraja（Arena 拡張を使わないスキンは beatoraja 本家でも可）
2. 同じ解像度・同じウィンドウの大きさ（例：1920×1080 の全画面か、同じ大きさのウィンドウ）
3. 比べる譜面：同梱のサンプル曲か、手元の同じ 1 曲（7 鍵 1 譜面、9 鍵 1 譜面。LN を含むもの）
4. 同じプロフィール設定（ハイスピード、レーンカバー、ゲージ、判定表示）

### 代表的なスキン（案）

BMZ の記録で名前が出ていて、使う人が多いものから選ぶ案です。最後はオペレーターが決めてください。

- LR2：W-MIX（HD）、ECSE（EndlessCirculation SE）、LITONE5、3R、9 鍵の LR2 スキンを 1 つ（R-1 の確認用）
- beatoraja JSON：beatoraja の同梱スキン、ECFN / ADFX02
- Lua：LITONE（ED の 378 / 379 を使う版）、ModernChic（J-5 の確認用）、SocialSkin、Luxe Flat

### 比べる画面

| 画面 | 見るところ |
|---|---|
| 選曲 | 曲バー（ランプ・ラベル・トロフィー・BAR_GRAPH）、スクロール、パネル 1〜3 の開閉、数字・文字 |
| 決定 | 開始と終わりの演出、スキップ |
| プレイ | 開始前・中盤・終盤・曲が終わった後。ノーツ・LN・ゲージ（光り・ボーダー）・判定・コンボ・ポップンキャラ・BGA・レーンカバー |
| リザルト | グラフ（ゲージ・スコア・判定の分布）、ランク表示、更新の演出、ボタンを押した後の段階 |
| 段位（コースリザルト） | 各ステージの数字、トロフィー、最後の結果 |

### スクリーンショットの撮り方

1. 両方のプレイヤーで同じ画面を開く。
2. 時間で変わる画面は、同じ時点で撮る。
   - プレイ：オートプレイで同じ譜面を流し、開始から 5 秒・30 秒・最後のノーツの直後で撮る
     （BMZ には動画出力の機能がある（BMZ `docs/video-export.md`）。同じ時点を撮るのに使えるかは未確認）。
   - 選曲・リザルト：画面に入って 3 秒待ってから撮る。
3. ファイル名は `スキン名_画面_プレイヤー名_時点.png` のように揃える。
4. 左右に並べるか、差分の画像を作って違う所に印を付ける。
5. 差が見つかったら、次を書く：スキン名・版、画面、条件（設定・譜面）、元のプレイヤーでの見た目、BMZ での見た目。
   スクリーンショットは公開の場に貼る前に、素材の権利を確かめる。

### 記録する場所

- 結果は #1586 か、新しい子 Issue に文章で書く（画像はリポジトリに入れない）。
- 上の表の「未確認」を、確かめた結果で埋める。
- 新しく見つかった差は、この文書の表に足す。

## 5. 未確認のこと

- 実物のスキンを BMZ で動かした結果（このエージェントは GUI を使えないため、2 段目に回す）。
- LR2 本体の動き（LR2 の CSV 仕様の記録が BMS-IR に無いため、oraja と BMZ の記録だけで比べた）。
- oraja の値の番号（約 900 個）のうち、BMZ で意味まで同じかどうか。
- ポップンキャラのタイマー 900〜909 を BMZ のスキンが参照できるか（J-3）。
- ゲージの `parts` と光りの一致（J-7）。BMZ 自身も「未検証」としている。
- R-9 のために BMZ のスコア保存が途中経過を持っているか。
- R-12 で BMZ に 7→9 鍵の変換があるか。
- 2 段目でプレイ画面を同じ時点で撮る方法が BMZ にあるか。
- 各差を直すのにかかる時間（表の「難しさ」は目安）。

## 6. オペレーターに決めてほしいこと

1. **「元のプレイヤー」の範囲**：LR2・beatoraja 本家・LR2oraja ED までにするか、Arena oraja の拡張（A-1〜A-7）まで含めるか。
2. **文字のグラデーションの項目名**（A-3）：BMZ の `colorTop` 系に寄せるか、Arena の `gradientTopColor` 系も BMZ で受けるか。
3. **BMZ が「同じ機能が無いので対応しない」としている部品**（R-2 `BAR_RANK`、R-4 `README`、R-5 スキン選択・キー設定画面、R-7 FX / EQ パネル）を、目標の「完全に動かす」に入れるか。
4. **2 段目で使うスキン**：上の案でよいか、別のスキンにするか。
5. **直す先**：フォーク（`tenP0312-dev/bmz-player`）で直すか、本家（`hyrorre/bmz-player`）へ出すか。この調査ではどちらにも書き込んでいません。

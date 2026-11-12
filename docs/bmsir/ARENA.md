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

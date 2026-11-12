# BMS-IR 専用化（外部通信の一覧と判断）

このフォークのビルドは、BMS-IR 以外の IR・オンラインサービスへ何も送らず、接続もしません（[README](README.md) 決定 1）。
実装は `crates/bmz-player/src/fork_policy.rs` に集めています。設定ファイルでは戻せません。

## 外部通信の一覧と判断

| 通信 | 場所 | 判断 | 理由・方法 |
|---|---|---|---|
| BMS-IR（接続・ログイン・スコア送信・ランキング・ライバル・表・履歴） | `ir/bms_ir.rs` | 残す | 従来どおり。origin はビルド時に固定済み |
| BMZ 公式 IR | `ir/bmz_official.rs` | 止める | クライアント生成時に BMS-IR 以外の origin を拒否（`ensure_ir_origin_allowed`）。provider 一覧からも除外 |
| rianIR | `ir/rian_ir*` | 止める | 同上 |
| カスタム IR provider | 設定 UI・`ir login` | 止める | 一覧は BMS-IR の 1 枠のみ。追加・種別選択の UI を外し、`ir login` は BMS-IR 以外を拒否（既定も `bms-ir`） |
| 本家のリリース確認・更新ダウンロード（GitHub `hyrorre/bmz-player`） | `update.rs` | 止める | `ONLINE_UPDATES_ENABLED = false`。確認・ダウンロードの入口で終了。手動確認は「無効」と表示 |
| Discord Rich Presence | `discord_presence.rs` | 止める | 曲名等を Discord へ公開するため。`DISCORD_PRESENCE_ENABLED = false` |
| BMS-IR Arena（WebSocket） | `arena/` | 残す（既定OFF） | 接続先を BMS-IR のホストだけに制限（`ensure_arena_url_allowed`）。[BMS-IR Arena](ARENA.md) |
| 難易度表の取得 | `difficulty_table.rs` | 残す | ユーザーが登録した公開 URL への GET のみ。プレイヤーデータを含まない |
| 曲・差分ダウンロード | `song_download*` | 残す | ユーザー操作による公開 URL への GET のみ。プレイヤーデータを含まない |
| OBS WebSocket | `obs*` | 残す | ユーザーが指定する OBS（通常は localhost）。外部サービスではない |
| Viewer IPC | `viewer_ipc.rs` | 残す | ローカルの TCP/IPC のみ |
| スキンの Lua HTTP | `bmz-skin` | 元から無効 | `lua_disabled_http` のとおり |
| テレメトリ | — | 該当なし | 該当するコード・依存は見つからなかった |

## 設定への影響

- 既存プロファイルの BMZ IR / rianIR / カスタム provider は、読み込み時にメモリ上から外れ、次の保存でファイルからも消えます。
  `primary_provider` が外れた provider を指していれば空にします。
- 外れた provider の認証情報ファイルはディスクに残りますが、読まれません（削除はしません）。
- 本家のコードへ戻すときは、`fork_policy.rs` の 2 定数、`normalize_builtin_providers`、2 つのクライアント生成のゲート、設定 UI の provider 行を戻します。

## 手元で確かめる手順

1. `cargo run -p bmz-player`。設定の IR 画面に provider が BMS-IR の 1 枠だけ出ること、「プロバイダー追加」がないこと。
2. 手動の更新確認が「無効」と表示され、Discord を有効にしても何も起きないこと。
3. BMS-IR にログインし、スコア送信とランキング取得が従来どおり動くこと。
4. `bmz-player ir login --provider rian-ir ...` が `only BMS-IR can be used` で失敗すること。

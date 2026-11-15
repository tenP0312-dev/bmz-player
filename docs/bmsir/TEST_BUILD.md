# 実機テスト用ビルド（統合ブランチ）

オペレーターが Windows の実機で確かめるために、開いている `claude/` の PR 19 本（#1〜#19）を
1 本にまとめたブランチです。**テスト用で、main にはマージしません。** 本家（hyrorre/bmz-player）には何も送りません。

- 統合ブランチ：`claude/test-build-2026-11-15`（元は `main` = `54c9503c`）
- ビルドの版の名前：`bmz-bmsir-0.1.0`（Arena の `client_version`。`crates/bmz-player/src/arena/protocol.rs`）
- 接続先は BMS-IR だけです（[README](README.md) の決定 1、[BMSIR_ONLY.md](BMSIR_ONLY.md)）。

## 1. 統合したブランチ

merge は PR 番号の順に行いました。Arena は B1 → B4（#7 → #10 → #14 → #16 → #17）、スキンは番号順です。

| PR | ブランチ | 内容 |
|---|---|---|
| #1 | `claude/bmsir-only` | BMS-IR 専用化 |
| #2 | `claude/fhs-mark-bmsir` | FHS の印を BMS-IR へ送る |
| #3 | `claude/ready-first-notes` | READY の間に最初のノーツを表示（既定 OFF） |
| #4 | `claude/one-bass-input` | ワンバス入力（lane と seed の計算） |
| #5 | `claude/one-bass-wiring` | ワンバス入力のつなぎ込み |
| #6 | `claude/skin-ed-numbers-378-379` | LR2oraja ED の数値 378 / 379（J-1） |
| #7 | `claude/arena-b1-connection` | Arena の接続の土台（B1） |
| #8 | `claude/skin-pm-chara` | LR2 `PM_CHARA` 命令と `.chp` の `--`（R-1, J-4） |
| #9 | `claude/skin-bar-graph-rank` | LR2 `BAR_GRAPH` type 1（R-3） |
| #10 | `claude/arena-b2-match-setup` | Arena のルームと推薦（B2-1） |
| #11 | `claude/skin-lr2-number-abs` | LR2 の負の数を絶対値で描く |
| #12 | `claude/skin-floatvalue` | floatvalue（J-2） |
| #13 | `claude/skin-gauge-parts` | リザルトのゲージ・パーツ（J-7） |
| #14 | `claude/arena-b2-load-start` | Arena のオプション確定・読み込み・開始（B2-2） |
| #15 | `claude/skin-pmchara-corner-key` | PMchara の右下画素の透過（J-4 後半） |
| #16 | `claude/arena-b3-live-final` | Arena の live / final / ロック / 結果（B3） |
| #17 | `claude/arena-b4-finish` | Arena の仕上げ（B4） |
| #18 | `claude/skin-lua-compat` | LuaJ 互換の層（L-1） |
| #19 | `claude/skin-r8-level-ghost-ir` | レベルバー・ゴースト・IR 順位（R-8） |

## 2. 衝突の解き方

| 場所 | 取り込み時 | 解き方 |
|---|---|---|
| `docs/bmsir/README.md` | #6、#7 | 追記された節が別々なので、両方を残した（ワンバス → スキン仕様 → Arena の順） |
| `crates/bmz-render/src/skin/tests.rs` | #13 | `mod` 宣言の追加が同じ場所だったので、`float_value` と `gauge_result_stretch` の両方を残した |
| `crates/bmz-skin/src/pm_chara.rs` | #15 | #8 の層の組み立て（`--` 補間・コマの細分化）を使い、#15 の `source_id(path, true)`（右下画素の透過）を足した。#15 側の古い `build_frames` 呼び出しは捨てた（#8 で関数の形が変わったため） |
| `docs/skin.md` | #15 | #8 の `--` と LR2 CSV の `PM_CHARA` の説明を残し、「chroma-key は未対応」の一文を #15 の右下画素の説明に置き換えた |
| `docs/skin-compatibility.md` | #15 | PMchara の行を 1 行にまとめた：`--` 補間・コマの細分化・`PM_CHARA` 命令（#8）＋右下画素の透過（#15）。タイマー 900..909 の個別の動きは未対応のまま |

上のほかに衝突はありません。統合ブランチ専用の修正は、この文書の追加だけです（コードの修正はありません）。

## 3. 検証（Linux、統合ブランチ）

- `cargo fmt --check`：通った
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通った
- `cargo test --workspace --no-fail-fast`：5506 件成功、30 件失敗、90 件 ignored
  - 失敗 30 件（`skin_loader::tests` 25、`app::tests` 3、`screens::settings_edit` 1、`storage::scan` 1）は、
    `main` の `cargo test -p bmz-player` でも同じ 30 件が落ちます（同名で一致を確認）。新しい失敗はありません。
  - 原因は、この環境でスキンの submodule（`data/skins/mz-select` など）が空なことです。
    submodule を取得した環境では、この 30 件は未確認です。
- Windows 実機での動作は未確認です。

## 4. Windows のビルド

### 4-1. GitHub Actions では作れませんでした

フォークの `.github/workflows/` で Windows をビルドするのは `release-apps.yml`（Release Apps）だけです。
Actions は有効ですが、次の理由でこのブランチを対象に動かせないため、dispatch していません。

- `workflow_dispatch` の入力は「リリースの tag」で、ブランチ名は指定できません。
  tag から `v` を除いた値が `Cargo.toml` の版（`0.5.1`）と一致しないと、最初の job で止まります。
- 一致する tag（`v0.5.1`）を指定しても、その tag が指すコミットをビルドします（このブランチではありません）。
  このブランチを指す tag を作る（push する）のは、`claude/` ブランチ以外は push しない決まりのため行っていません。
- ほかのワークフローは Linux の tar（`linux-tar.yml`）と、スキンのテスト（`skin-security.yml`）です。

オペレーターが tag を作って動かす場合は、次の点に注意してください：tag の版は `0.5.1` と一致させる必要があり、
`upload_to_release` は false のままにします（自動更新の鍵が無い設定で公開しないため）。

### 4-2. オペレーターの PC でビルドする手順

前提：Windows 10/11 x64、Rust（stable）、Visual Studio の C++ ビルドツール、LLVM（`LIBCLANG_PATH`）、
Git、vcpkg（FFmpeg 用）、PowerShell 5.1 以降。

1. 取得

   ```powershell
   git clone https://github.com/tenP0312-dev/bmz-player
   cd bmz-player
   git checkout claude/test-build-2026-11-15
   git submodule update --init --recursive data/skins/Rmz-skin data/skins/mz-select data/skins/Luxez-Flat
   git rev-parse HEAD   # 40 桁。これが Arena の build_hash になる（5 章）
   ```

2. FFmpeg（vcpkg。`release-apps.yml` と同じ設定）

   ```powershell
   $env:VCPKG_ROOT = "C:\vcpkg"
   $env:VCPKG_DEFAULT_TRIPLET = "x64-windows-release"
   $env:VCPKGRS_TRIPLET = "x64-windows-release"
   $env:VCPKGRS_DYNAMIC = "1"
   $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
   & "$env:VCPKG_ROOT\vcpkg.exe" install "--overlay-triplets=$PWD\triplets" "ffmpeg:$env:VCPKG_DEFAULT_TRIPLET"
   ```

3. ビルド

   ```powershell
   cargo build -p bmz-player --release --locked
   ```

   - `-dirty` が付くとビルドのコミットが許可リストに入れられません。コミット済みの状態でビルドします
     （`git status` が `crates` / `Cargo.toml` / `Cargo.lock` で空であること）。
   - 手元の Arena サーバーに向ける場合だけ、ビルド時に `BMZ_BMS_IR_BASE_URL` を付けます
     （[ARENA_MANUAL_TEST.md](ARENA_MANUAL_TEST.md) の 1-2）。本番に向けるビルドでは付けません。

4. DLL・資源の置き方

   `target\release\bmz-player.exe` は、そのままでは FFmpeg の DLL と `resources` が足りず起動しません。
   次のどちらかで配置します。

   - 推奨：`scripts/package-windows.ps1` で、配置までまとめて作る

     ```powershell
     .\scripts\package-windows.ps1 -DllDir "$env:VCPKG_ROOT\installed\x64-windows-release\bin"
     ```

     出力：`dist\windows\BMZ Player\`（`bmz-player.exe`、`updater\`、`resources\`、FFmpeg の DLL）。
     `-Installer` を付けると Inno Setup のインストーラーも作ります（Inno Setup が必要）。
   - 手で並べる場合：`bmz-player.exe` の隣に、vcpkg の `installed\x64-windows-release\bin\*.dll`
     （`avcodec` / `avformat` / `avutil` / `swscale` / `swresample` など）を置き、
     同じ場所に `resources\`（`data\skins`、`data\songs\sample-playable`、フォント）を置きます
     （配置は [packaging.md](../packaging.md) の staging layout）。

5. 起動確認：`"dist\windows\BMZ Player\bmz-player.exe" --help` が出ること。

配布物に FFmpeg を含めるときのライセンスは [licenses.md](../licenses.md) を確認してください（このビルドは配布せず、テスト用です）。

## 5. BMS-IR 側の準備（Arena を試す場合だけ）

Arena を本番の BMS-IR で試すには、BMS-IR 側の次の許可リストに、このビルドの値を**オペレーターが**入れます。
**値はこの文書にも PR にも Issue にも書きません。**

| 許可リスト | 入れるもの |
|---|---|
| `BMSIR_ARENA_ALLOWED_BMZ_CLIENT_VERSIONS` | このビルドの版（`bmz-bmsir-0.1.0`） |
| `BMSIR_ARENA_ALLOWED_BMZ_BUILD_HASHES` | このビルドのコミット（40 桁。4-2 の `git rev-parse HEAD`） |

- コミットは、**ビルドした時点の HEAD** です。この文書や PR のコメントでコミットを足すと変わるので、
  ビルドする直前に確認します。
- 手順の詳細と、本番に入るまでの手元のサーバーでの試し方は [ARENA_MANUAL_TEST.md](ARENA_MANUAL_TEST.md) の 0 章と 1 章です。
- BMS-IR にログインしてスコアを送るだけなら、この許可リストは要りません。

## 6. 実機テストの順番

1. **BMS-IR にログインしてスコアが送れる**（#1〜#5）
   - BMS-IR 以外（BMZ 公式 IR、rianIR など）へ接続・送信しないこと（[BMSIR_ONLY.md](BMSIR_ONLY.md)）。
   - BMS-IR にログイン → 1 曲プレイ → スコアが BMS-IR に出る。FHS を使った曲は印が付く（#2）。
   - READY の最初のノーツ（#3）とワンバス（#4・#5）は既定 OFF です。ON にして確かめる場合は、各 PR の手順に従います。
2. **スキン**（#6、#8、#9、#11、#12、#13、#15、#18、#19）
   - 各 PR の説明にある確認の手順に従います。互換の対応表は [skin-compatibility.md](../skin-compatibility.md)、
     期待する動きは [SKIN_BEHAVIOR_SPEC.md](SKIN_BEHAVIOR_SPEC.md) です。
   - 観点：#6 数値 378/379、#8・#15 PMchara、#9 曲選択のバー、#11 負の数、#12 floatvalue、
     #13 リザルトのゲージ、#18 Lua の `tostring` / `string.format`、#19 レベルバー・ゴースト・IR 順位。
   - 同じスキンを複数の PR が触る場合（#8 と #15 の PMchara）は、両方の効果が同時に出ることを見ます。
3. **Arena**（#7、#10、#14、#16、#17）
   - 5 章の許可リストの準備をしてから、[ARENA_MANUAL_TEST.md](ARENA_MANUAL_TEST.md) の 2 章（接続）から 8 章（棄権）まで、
     番号の順に行います。Arena の接続は既定 OFF です（決定 6）。
   - 結果の記録は ARENA_MANUAL_TEST.md の 9 章に従い、秘密は書きません。

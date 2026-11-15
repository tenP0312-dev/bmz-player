# ワンバス入力：seed の計算

実施日：2026-11-12。仕様は [SPEC_ONE_BASS_INPUT.md](../../docs/bmsir/SPEC_ONE_BASS_INPUT.md)（BMS-IR フォーク用）。

## 実装

`crates/bmz-player/src/screens/play_session/arrange/one_bass.rs`。状態を持たない関数だけで、画面や入力にはつないでいない。
通常の RANDOM と同じ `shuffle_lane_groups`（beatoraja 互換の remove-at-index シャッフル）で配置を求めるため、
求めた seed は通常の RANDOM の seed としてそのまま再現できる。

## 計測

- 5 レーン 120 通り、7 レーン 5,040 通りは、seed 0〜5,000 / 0〜50,000 の範囲にすべて現れることをテストで固定した。
- 9 レーン（PMS）の逆順の並びを `--release` で全範囲探索した場合：seed 437623、約 4.5 ms。
  最悪（24 bit 全域の走査）でも 1 秒を大きく超えない見込み。対応表の再利用は不要と判断した。
  再計測：`cargo test -p bmz-player --lib --release nine_lane -- --ignored --nocapture`。
- debug ビルドでは遅いので、9 レーンの計測は `#[ignore]` にしてある。

## 未確認

- 決定画面の終わりで実際の押下を読めるか、読み込みのやり直しで延びる時間は、つなぎ込み側の PR で扱う。

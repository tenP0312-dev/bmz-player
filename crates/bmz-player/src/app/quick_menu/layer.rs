//! E4「システムボタン」の短押し・長押し・同時押しの判定。
//!
//! プレイ以外の画面で、キーコンフィグでE4に割り当てた入力をレイヤーキーとして扱う。
//! 物理ボタン名は扱わず、呼び出し側が [`SystemInput`] へ分類してから渡す。

use std::time::{Duration, Instant};

use bmz_core::lane::Lane;

/// これ以上押し続けて離した場合は短押しにしない。キーコンフィグの待ち受け取消にも使う。
pub(in crate::app) const SYSTEM_BUTTON_LONG_HOLD: Duration = Duration::from_secs(1);

/// UIから見た入力の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum SystemInput {
    E1,
    E2,
    E3,
    E4,
    /// 皿の上回し（一覧では前へ）。
    ScratchUp,
    /// 皿の下回し（一覧では次へ）。
    ScratchDown,
    /// KEY1〜KEY7。14KのKEY8〜KEY14も同じ番号へまとめる。
    Key(Lane),
    Other,
}

/// 1回の入力に対するレイヤーの判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum LayerEvent {
    /// レイヤーに関係しない。通常の経路へ渡す。
    Pass,
    /// レイヤーが受け取った。通常の経路へは渡さない。
    Swallow,
    /// E4を押し始めた。
    E4Down,
    /// E4を短押しした（ほかの入力なしで、長押しの時間より前に離した）。
    Tap,
    /// E4をほかの入力なしで長押しした。押している間に1回だけ出す。
    LongHold,
    /// E4を押したまま、ほかの入力を押した。
    Chord(SystemInput),
}

#[derive(Debug, Default)]
pub(in crate::app) struct SystemButtonLayer {
    down_at: Option<Instant>,
    /// 押している間にほかの入力があった、または取り消された。短押し・長押しにしない。
    spent: bool,
    long_hold_fired: bool,
}

impl SystemButtonLayer {
    /// E4を押している間か。E1〜E3の本来の意味を止める判定に使う。
    pub(in crate::app) fn held(&self) -> bool {
        self.down_at.is_some()
    }

    /// `others_held` はE4を押した時点で、ほかの入力が押されていたか。
    pub(in crate::app) fn press(
        &mut self,
        input: SystemInput,
        now: Instant,
        others_held: bool,
    ) -> LayerEvent {
        if input == SystemInput::E4 {
            if self.down_at.is_some() {
                // 別のコントローラーのE4などの重複Press。最初の押下を維持する。
                return LayerEvent::Swallow;
            }
            self.down_at = Some(now);
            self.spent = others_held;
            self.long_hold_fired = false;
            return LayerEvent::E4Down;
        }
        if self.down_at.is_none() {
            return LayerEvent::Pass;
        }
        self.spent = true;
        LayerEvent::Chord(input)
    }

    pub(in crate::app) fn release(&mut self, input: SystemInput, now: Instant) -> LayerEvent {
        if input != SystemInput::E4 {
            // ほかの入力のReleaseは押下状態を戻すため常に通す。
            return LayerEvent::Pass;
        }
        let Some(down_at) = self.down_at.take() else {
            return LayerEvent::Pass;
        };
        let spent = std::mem::take(&mut self.spent);
        let long_hold_fired = std::mem::take(&mut self.long_hold_fired);
        if spent || long_hold_fired {
            return LayerEvent::Swallow;
        }
        if now.saturating_duration_since(down_at) >= SYSTEM_BUTTON_LONG_HOLD {
            // frameの更新より先にReleaseが届いた場合も、長押しとして1回だけ扱う。
            LayerEvent::LongHold
        } else {
            LayerEvent::Tap
        }
    }

    /// frameごとに呼び、長押しを判定する。
    pub(in crate::app) fn tick(&mut self, now: Instant) -> Option<LayerEvent> {
        let down_at = self.down_at?;
        if self.spent
            || self.long_hold_fired
            || now.saturating_duration_since(down_at) < SYSTEM_BUTTON_LONG_HOLD
        {
            return None;
        }
        self.long_hold_fired = true;
        Some(LayerEvent::LongHold)
    }

    /// 押している間に、ボタン以外（アナログ皿など）の入力があった。
    pub(in crate::app) fn mark_spent(&mut self) {
        if self.down_at.is_some() {
            self.spent = true;
        }
    }

    /// フォーカス喪失・プレイ開始などで、押下中の判定を捨てる。
    pub(in crate::app) fn reset(&mut self) {
        *self = Self::default();
    }
}

//! 専コン（7 鍵＋皿＋E1〜E4）の入力から画面キーボードの操作への対応。
//!
//! 物理ボタンではなく、キーコンフィグで解決した後の論理ボタン（KEY1〜7・皿・E1〜E4）を受け取る。
//! DP では 2P 側の KEY8〜14 / 皿 2 を、呼び出し側で KEY1〜7 / 皿へ読み替えて渡す。
//!
//! | 入力 | 操作 |
//! | --- | --- |
//! | 皿 上 / 下 | カーソルを左 / 右 |
//! | KEY1 | 文字を入力 |
//! | KEY2 / KEY4 | カーソルを上 / 下 |
//! | KEY3 | 直前の文字を変換（゛・゜・小書き、英字の大小） |
//! | KEY5 | 1 文字消す |
//! | KEY6 | 次の文字種 |
//! | KEY7 | 確定 |
//! | E1（単独で押して離す） | 空白 |
//! | E1 hold + 皿 上 / 下 | キャレットを左 / 右 |
//! | E1 hold + KEY5 | すべて消す |
//! | E1 hold + KEY6、E3 | 前の文字種 |
//! | E2 / E4 | 取消（入力を変更済みなら 2 回押す） |

use super::state::SoftKeyboardAction;

/// キーコンフィグで解決した後の論理ボタン。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftKeyboardPadButton {
    /// 鍵盤。1〜7 以外の番号は無視する。
    Key(u8),
    ScratchUp,
    ScratchDown,
    E1,
    E2,
    E3,
    /// システムボタン。画面キーボードでは取消（閉じる）だけに使う。
    E4,
}

/// 押下・解放から [`SoftKeyboardAction`] を作る。E1 の hold を覚える以外の状態は持たない。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SoftKeyboardPadMapper {
    e1_held: bool,
    e1_combined: bool,
}

impl SoftKeyboardPadMapper {
    pub fn new() -> Self {
        Self::default()
    }

    /// E1 を押している間なら true（描画側で E1 hold 用の案内を出すため）。
    pub fn e1_held(&self) -> bool {
        self.e1_held
    }

    /// フォーカス喪失・画面キーボードを閉じたときなどに hold 状態を捨てる。
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// ボタンを押したときの操作。皿は 1 tick ごとに押下として渡す。
    pub fn press(&mut self, button: SoftKeyboardPadButton) -> Option<SoftKeyboardAction> {
        use SoftKeyboardAction as A;
        use SoftKeyboardPadButton as B;
        if button == B::E1 {
            self.e1_held = true;
            self.e1_combined = false;
            return None;
        }
        let shifted = self.e1_held;
        if shifted {
            self.e1_combined = true;
        }
        match (button, shifted) {
            (B::ScratchUp, false) => Some(A::CursorLeft),
            (B::ScratchDown, false) => Some(A::CursorRight),
            (B::ScratchUp, true) => Some(A::CaretLeft),
            (B::ScratchDown, true) => Some(A::CaretRight),
            (B::Key(1), _) => Some(A::InputSelected),
            (B::Key(2), _) => Some(A::CursorUp),
            (B::Key(3), _) => Some(A::Convert),
            (B::Key(4), _) => Some(A::CursorDown),
            (B::Key(5), false) => Some(A::Backspace),
            (B::Key(5), true) => Some(A::ClearAll),
            (B::Key(6), false) => Some(A::NextMode),
            (B::Key(6), true) => Some(A::PreviousMode),
            (B::Key(7), _) => Some(A::Submit),
            (B::Key(_), _) => None,
            (B::E3, _) => Some(A::PreviousMode),
            (B::E2 | B::E4, _) => Some(A::Cancel),
            (B::E1, _) => None,
        }
    }

    /// ボタンを離したときの操作。E1 をほかの入力と組み合わせずに離したときだけ空白を返す。
    pub fn release(&mut self, button: SoftKeyboardPadButton) -> Option<SoftKeyboardAction> {
        if button != SoftKeyboardPadButton::E1 || !self.e1_held {
            return None;
        }
        let tapped = !self.e1_combined;
        self.e1_held = false;
        self.e1_combined = false;
        tapped.then_some(SoftKeyboardAction::InputSpace)
    }
}

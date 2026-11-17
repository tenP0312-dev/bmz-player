//! コントローラーで操作する画面キーボード（文字入力の部品）。
//!
//! 選曲の検索・IR ログイン・プロファイル名など、文字入力の欄から共通で使う。
//! 状態機械（[`SoftKeyboard`]）は egui に依存せず、描画（[`draw_soft_keyboard`]）と分けている。
//!
//! 使い方:
//!
//! 1. [`SoftKeyboard::open`] に [`SoftKeyboardFieldSpec`] を渡して開く。
//! 2. 入力ごとに [`SoftKeyboardPadMapper::press`] / [`SoftKeyboardPadMapper::release`] で
//!    [`SoftKeyboardAction`] へ変換し、[`SoftKeyboard::apply`] へ渡す。
//! 3. 毎フレーム [`draw_soft_keyboard`] で描き、返ったマウス操作も `apply` へ渡す。
//! 4. [`SoftKeyboard::take_outcome`] が `Submitted(String)` / `Cancelled` を返したら閉じる。
//!
//! ボタンの割り当ては [`pad`] と `docs/controls.md` の「画面キーボード」を参照。

mod draw;
mod layout;
pub mod pad;
mod state;

#[cfg(test)]
mod tests;

pub use draw::{SoftKeyboardDrawOutput, draw_soft_keyboard};
pub use layout::{GRID_COLUMNS, SoftKeyboardCell, SoftKeyboardMode, convert_char};
pub use pad::{SoftKeyboardPadButton, SoftKeyboardPadMapper};
pub use state::{
    MASK_CHAR, SoftKeyboard, SoftKeyboardAction, SoftKeyboardEvent, SoftKeyboardFieldSpec,
    SoftKeyboardNotice, SoftKeyboardOutcome, SoftKeyboardRejection,
};

//! 画面ごとのクイックメニュー項目と、E4 hold + KEY1〜7 の近道の表。
//!
//! 項目を足すときは、次の3か所を変える。
//! 1. 必要なら [`QuickMenuAction`] に操作を足し、`WinitApp::execute_quick_menu_action` で実行する。
//!    選曲画面の既存操作は `QuickMenuAction::Select(SelectAction::…)` で足りる。
//! 2. 画面の表（`SELECT_ITEMS` など）へ [`QuickMenuEntry`] を足す。表の順が表示順になる。
//! 3. 表示名のキーを `i18n/locales/*.ftl` の全カタログへ足す。
//!
//! 近道は画面の近道表（`SELECT_SHORTCUTS` など）へ足す。同じ操作が項目にあれば、項目の横に表示する。

use bmz_core::lane::Lane;

use crate::app::scene_input::{SelectAction, SelectMove};

/// クイックメニューを開ける画面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum QuickMenuScreen {
    Select,
    /// 選曲画面の「設定」フォルダ（キーコンフィグを含む）。
    SettingsFolder,
    Result,
    /// Viewerの単曲再生後、次の命令を待っている状態。
    ViewerWaiting,
}

/// クイックメニュー項目・近道から実行する操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum QuickMenuAction {
    /// 選曲画面の既存操作。
    Select(SelectAction),
    /// 設定フォルダで選んでいるキーコンフィグの割り当てを外す。
    KeyConfigClear,
    /// 設定フォルダで選んでいるキーコンフィグの割り当てを既定に戻す。
    KeyConfigRestoreDefault,
    OpenBmzMenu,
    /// 終了の確認を表示する。
    RequestExit,
    /// 確認後に終了する。
    ConfirmExit,
    /// 確認を取り消して元の一覧へ戻る。
    CancelExit,
}

impl QuickMenuAction {
    /// 実行できる状態か。項目の表示と近道の両方に使う。
    pub(in crate::app) fn available(self, context: &QuickMenuContext) -> bool {
        match self {
            Self::KeyConfigClear | Self::KeyConfigRestoreDefault => context.key_binding_selected,
            Self::Select(_)
            | Self::OpenBmzMenu
            | Self::RequestExit
            | Self::ConfirmExit
            | Self::CancelExit => true,
        }
    }
}

/// 項目の有効・無効を決める、開いた時点の画面の状態。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::app) struct QuickMenuContext {
    /// 設定フォルダでキーコンフィグの行を選んでいて、待ち受け中ではない。
    pub(in crate::app) key_binding_selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct QuickMenuEntry {
    pub(in crate::app) action: QuickMenuAction,
    /// 表示名のメッセージキー。
    pub(in crate::app) label_key: &'static str,
}

const fn entry(action: QuickMenuAction, label_key: &'static str) -> QuickMenuEntry {
    QuickMenuEntry { action, label_key }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct QuickMenuShortcut {
    /// E4を押したまま押す鍵盤。
    pub(in crate::app) lane: Lane,
    pub(in crate::app) action: QuickMenuAction,
}

const OPEN_BMZ_MENU: QuickMenuEntry =
    entry(QuickMenuAction::OpenBmzMenu, "quick-menu-open-bmz-menu");
const REQUEST_EXIT: QuickMenuEntry = entry(QuickMenuAction::RequestExit, "quick-menu-exit");

const SELECT_ITEMS: &[QuickMenuEntry] = &[
    entry(
        QuickMenuAction::Select(SelectAction::Move(SelectMove::First)),
        "quick-menu-select-first",
    ),
    entry(QuickMenuAction::Select(SelectAction::Move(SelectMove::Last)), "quick-menu-select-last"),
    OPEN_BMZ_MENU,
    REQUEST_EXIT,
];

const SETTINGS_FOLDER_ITEMS: &[QuickMenuEntry] = &[
    entry(QuickMenuAction::KeyConfigClear, "quick-menu-key-config-clear"),
    entry(QuickMenuAction::KeyConfigRestoreDefault, "quick-menu-key-config-restore-default"),
    OPEN_BMZ_MENU,
    REQUEST_EXIT,
];

const RESULT_ITEMS: &[QuickMenuEntry] = &[OPEN_BMZ_MENU];

const VIEWER_WAITING_ITEMS: &[QuickMenuEntry] = &[OPEN_BMZ_MENU];

/// 終了の確認。カーソルは取消（2番目）から始める。
pub(in crate::app) const EXIT_CONFIRM_ITEMS: &[QuickMenuEntry] = &[
    entry(QuickMenuAction::ConfirmExit, "quick-menu-exit-confirm"),
    entry(QuickMenuAction::CancelExit, "quick-menu-exit-cancel"),
];

// 段階2で、選曲は検索・ソート等、リザルトはリプレイ保存等を足す。
const SELECT_SHORTCUTS: &[QuickMenuShortcut] = &[];
const SETTINGS_FOLDER_SHORTCUTS: &[QuickMenuShortcut] = &[];
const RESULT_SHORTCUTS: &[QuickMenuShortcut] = &[];
const VIEWER_WAITING_SHORTCUTS: &[QuickMenuShortcut] = &[];

pub(in crate::app) fn quick_menu_entries(screen: QuickMenuScreen) -> &'static [QuickMenuEntry] {
    match screen {
        QuickMenuScreen::Select => SELECT_ITEMS,
        QuickMenuScreen::SettingsFolder => SETTINGS_FOLDER_ITEMS,
        QuickMenuScreen::Result => RESULT_ITEMS,
        QuickMenuScreen::ViewerWaiting => VIEWER_WAITING_ITEMS,
    }
}

pub(in crate::app) fn quick_menu_shortcuts(
    screen: QuickMenuScreen,
) -> &'static [QuickMenuShortcut] {
    match screen {
        QuickMenuScreen::Select => SELECT_SHORTCUTS,
        QuickMenuScreen::SettingsFolder => SETTINGS_FOLDER_SHORTCUTS,
        QuickMenuScreen::Result => RESULT_SHORTCUTS,
        QuickMenuScreen::ViewerWaiting => VIEWER_WAITING_SHORTCUTS,
    }
}

/// E4 hold + 鍵盤で実行する操作。今の状態で使えない操作は返さない。
pub(in crate::app) fn shortcut_action_for(
    shortcuts: &[QuickMenuShortcut],
    lane: Lane,
    context: &QuickMenuContext,
) -> Option<QuickMenuAction> {
    shortcuts
        .iter()
        .find(|shortcut| shortcut.lane == lane)
        .map(|shortcut| shortcut.action)
        .filter(|action| action.available(context))
}

/// 項目の横に出す近道の鍵盤。
pub(in crate::app) fn shortcut_lane_for(
    shortcuts: &[QuickMenuShortcut],
    action: QuickMenuAction,
) -> Option<Lane> {
    shortcuts.iter().find(|shortcut| shortcut.action == action).map(|shortcut| shortcut.lane)
}

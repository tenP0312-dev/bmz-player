//! クイックメニューのカーソルとページ。描画とは独立した状態だけを持つ。

use std::time::{Duration, Instant};

use bmz_core::lane::Lane;

use super::registry::{
    EXIT_CONFIRM_ITEMS, QuickMenuAction, QuickMenuContext, QuickMenuEntry, QuickMenuScreen,
    quick_menu_entries, quick_menu_shortcuts, shortcut_lane_for,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) struct QuickMenuItem {
    pub(in crate::app) action: QuickMenuAction,
    pub(in crate::app) label_key: &'static str,
    /// E4 hold + この鍵盤でも実行できる。
    pub(in crate::app) shortcut: Option<Lane>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum QuickMenuPage {
    Main,
    ConfirmExit,
}

/// メニュー表示中のコントローラー入力。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum QuickMenuInput {
    Up,
    Down,
    /// 奇数鍵（KEY1 / 3 / 5 / 7）。
    Run,
    /// 偶数鍵・E2・E4。
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum QuickMenuCommand {
    None,
    /// カーソルやページが変わった。
    Moved,
    Close,
    /// メニューを閉じて操作を実行する。
    Execute(QuickMenuAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct QuickMenu {
    screen: QuickMenuScreen,
    page: QuickMenuPage,
    main_items: Vec<QuickMenuItem>,
    main_cursor: usize,
    cursor: usize,
}

impl QuickMenu {
    /// 使える項目がなければ開かない。
    pub(in crate::app) fn open(
        screen: QuickMenuScreen,
        context: &QuickMenuContext,
    ) -> Option<Self> {
        let shortcuts = quick_menu_shortcuts(screen);
        let main_items: Vec<_> = quick_menu_entries(screen)
            .iter()
            .filter(|entry| entry.action.available(context))
            .map(|entry| QuickMenuItem {
                action: entry.action,
                label_key: entry.label_key,
                shortcut: shortcut_lane_for(shortcuts, entry.action),
            })
            .collect();
        if main_items.is_empty() {
            return None;
        }
        Some(Self { screen, page: QuickMenuPage::Main, main_items, main_cursor: 0, cursor: 0 })
    }

    pub(in crate::app) fn screen(&self) -> QuickMenuScreen {
        self.screen
    }

    pub(in crate::app) fn page(&self) -> QuickMenuPage {
        self.page
    }

    pub(in crate::app) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(in crate::app) fn items(&self) -> Vec<QuickMenuItem> {
        match self.page {
            QuickMenuPage::Main => self.main_items.clone(),
            QuickMenuPage::ConfirmExit => EXIT_CONFIRM_ITEMS
                .iter()
                .map(|&QuickMenuEntry { action, label_key }| QuickMenuItem {
                    action,
                    label_key,
                    shortcut: None,
                })
                .collect(),
        }
    }

    fn item_count(&self) -> usize {
        match self.page {
            QuickMenuPage::Main => self.main_items.len(),
            QuickMenuPage::ConfirmExit => EXIT_CONFIRM_ITEMS.len(),
        }
    }

    /// 項目数で割った余りで、端から反対の端へ回り込む。
    pub(in crate::app) fn move_cursor(&mut self, steps: i32) -> bool {
        let count = self.item_count() as i64;
        if count <= 1 || steps == 0 {
            return false;
        }
        let next = (self.cursor as i64 + i64::from(steps)).rem_euclid(count) as usize;
        let moved = next != self.cursor;
        self.cursor = next;
        moved
    }

    pub(in crate::app) fn handle(&mut self, input: QuickMenuInput) -> QuickMenuCommand {
        match input {
            QuickMenuInput::Up => self.moved(-1),
            QuickMenuInput::Down => self.moved(1),
            QuickMenuInput::Close if self.page == QuickMenuPage::ConfirmExit => {
                self.leave_confirm()
            }
            QuickMenuInput::Close => QuickMenuCommand::Close,
            QuickMenuInput::Run => {
                let Some(item) = self.items().get(self.cursor).copied() else {
                    return QuickMenuCommand::None;
                };
                match item.action {
                    QuickMenuAction::RequestExit => {
                        self.main_cursor = self.cursor;
                        self.page = QuickMenuPage::ConfirmExit;
                        self.cursor = EXIT_CONFIRM_ITEMS
                            .iter()
                            .position(|entry| entry.action == QuickMenuAction::CancelExit)
                            .unwrap_or(0);
                        QuickMenuCommand::Moved
                    }
                    QuickMenuAction::CancelExit => self.leave_confirm(),
                    action => QuickMenuCommand::Execute(action),
                }
            }
        }
    }

    fn moved(&mut self, steps: i32) -> QuickMenuCommand {
        if self.move_cursor(steps) { QuickMenuCommand::Moved } else { QuickMenuCommand::None }
    }

    fn leave_confirm(&mut self) -> QuickMenuCommand {
        self.page = QuickMenuPage::Main;
        self.cursor = self.main_cursor;
        QuickMenuCommand::Moved
    }
}

/// アナログ皿のtickを、一定量ごとのカーソル移動へ変える。
#[derive(Debug, Default)]
pub(in crate::app) struct AnalogStepper {
    buffer: i32,
    last_tick_at: Option<Instant>,
}

impl AnalogStepper {
    /// 戻り値は移動する段数（符号つき）。途切れた後の端数は捨てる。
    pub(in crate::app) fn push(
        &mut self,
        delta: i32,
        now: Instant,
        ticks_per_step: i32,
        idle_after: Duration,
    ) -> i32 {
        let idle =
            self.last_tick_at.is_none_or(|last| now.saturating_duration_since(last) > idle_after);
        self.last_tick_at = Some(now);
        if idle {
            self.buffer = 0;
        }
        self.buffer += delta;
        let ticks_per_step = ticks_per_step.max(1);
        let steps = self.buffer / ticks_per_step;
        self.buffer -= steps * ticks_per_step;
        steps
    }

    pub(in crate::app) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// アナログ皿の1回の回転（途切れるまで）を、1回の操作へ変える。E4 hold + 皿のページ移動に使う。
#[derive(Debug, Default)]
pub(in crate::app) struct FlickDetector {
    buffer: i32,
    last_tick_at: Option<Instant>,
    fired: bool,
}

impl FlickDetector {
    /// 回転量がしきい値に届いた最初のtickだけ、方向（+1 / -1）を返す。
    pub(in crate::app) fn push(
        &mut self,
        delta: i32,
        now: Instant,
        threshold: i32,
        idle_after: Duration,
    ) -> Option<i32> {
        let idle =
            self.last_tick_at.is_none_or(|last| now.saturating_duration_since(last) > idle_after);
        self.last_tick_at = Some(now);
        if idle {
            self.buffer = 0;
            self.fired = false;
        }
        if self.fired {
            return None;
        }
        self.buffer += delta;
        if self.buffer.abs() < threshold.max(1) {
            return None;
        }
        self.fired = true;
        Some(self.buffer.signum())
    }

    pub(in crate::app) fn reset(&mut self) {
        *self = Self::default();
    }
}

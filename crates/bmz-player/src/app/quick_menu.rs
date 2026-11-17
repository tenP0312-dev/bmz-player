//! E4「システムボタン」とクイックメニュー。
//!
//! プレイ以外の画面（選曲・設定フォルダ・リザルト・Viewer待機）で、キーコンフィグのE4を次のように扱う。
//! - 短押し: クイックメニューを開く。BMZ メニュー表示中は閉じる。
//! - E4 hold + E3: スクリーンショット。
//! - E4 hold + 皿: 選曲のページ移動。
//! - E4 hold + KEY1〜7: 画面ごとの近道（[`registry`] の近道表）。
//! - 1秒の長押し: 設定フォルダのキーコンフィグ待ち受けを取り消す。
//!
//! E4 hold 中はE1〜E3の本来の意味を止める（`reconcile_select_holds_from_pressed_controls`）。
//! 判定はframeループのゲームパッド経路（`GamepadButtonEvent` の後）で行うため、全backendに効く。
//! キーボードのE4（既定は R）は対象外で、従来どおり動く。

use super::*;

mod layer;
mod menu;
mod registry;
#[cfg(test)]
mod tests;

pub(super) use layer::{LayerEvent, SystemButtonLayer, SystemInput};
pub(super) use menu::{QuickMenu, QuickMenuCommand, QuickMenuInput, QuickMenuPage};
pub(super) use registry::{
    QuickMenuAction, QuickMenuContext, QuickMenuScreen, quick_menu_shortcuts, shortcut_action_for,
};

use menu::{AnalogStepper, FlickDetector};

/// クイックメニューの表示状態と、E4 hold + 皿の回転量。
#[derive(Debug, Default)]
pub(super) struct QuickMenuRuntime {
    pub(super) menu: Option<QuickMenu>,
    menu_analog: AnalogStepper,
    page_flick: FlickDetector,
    /// E4を押し始めた画面。押している間に画面が変わったら短押しにしない。
    press_screen: Option<QuickMenuScreen>,
    /// クイックメニューの「終了」を確定した。選曲画面の終了経路が次のframeで処理する。
    pub(super) exit_requested: bool,
}

impl QuickMenuRuntime {
    pub(super) fn is_open(&self) -> bool {
        self.menu.is_some()
    }
}

/// コントローラーの入力名を、キーコンフィグの割り当てに従って分類する。
pub(super) fn classify_system_input(keys: &SelectKeyBindings, control: &str) -> SystemInput {
    match keys.e_action_for_control(control) {
        Some(InputActionConfig::E4) => return SystemInput::E4,
        Some(InputActionConfig::E3) => return SystemInput::E3,
        Some(InputActionConfig::E2) => return SystemInput::E2,
        Some(InputActionConfig::E1) => return SystemInput::E1,
        _ => {}
    }
    if keys.is_start(control) {
        return SystemInput::E1;
    }
    if keys.is_e2_action(control) {
        return SystemInput::E2;
    }
    if keys.is_e3_action(control) {
        return SystemInput::E3;
    }
    // 9KではKEY4 / KEY6が一覧の前後移動を兼ねる。
    if keys.is_select_previous(control) {
        return SystemInput::ScratchUp;
    }
    if keys.is_select_next(control) {
        return SystemInput::ScratchDown;
    }
    keys.ui_lane_for_control(control).map_or(SystemInput::Other, SystemInput::Key)
}

/// メニュー表示中の入力。皿で選び、奇数鍵で実行、偶数鍵・E2・E4で閉じる。
pub(super) fn quick_menu_input_for(input: SystemInput) -> Option<QuickMenuInput> {
    match input {
        SystemInput::ScratchUp => Some(QuickMenuInput::Up),
        SystemInput::ScratchDown => Some(QuickMenuInput::Down),
        SystemInput::Key(Lane::Key1 | Lane::Key3 | Lane::Key5 | Lane::Key7) => {
            Some(QuickMenuInput::Run)
        }
        SystemInput::Key(_) | SystemInput::E2 | SystemInput::E4 => Some(QuickMenuInput::Close),
        SystemInput::E1 | SystemInput::E3 | SystemInput::Other => None,
    }
}

/// キーコンフィグ待ち受け中のE4の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ListeningAction {
    None,
    /// E4に割り当てた入力そのものを、待ち受け中のスロットへ割り当てる。
    BindE4,
    /// 待ち受けを取り消す。
    Cancel,
}

/// 短押しはE4自身の割り当て、1秒の長押し（frame更新・Releaseのどちらで判定しても）は取消。
pub(super) fn listening_action_for(event: Option<LayerEvent>) -> ListeningAction {
    match event {
        Some(LayerEvent::Tap) => ListeningAction::BindE4,
        Some(LayerEvent::LongHold) => ListeningAction::Cancel,
        _ => ListeningAction::None,
    }
}

pub(super) fn shortcut_label(lane: Lane) -> String {
    format!("E4+KEY{}", lane as u8)
}

impl WinitApp {
    /// E4をシステムボタンとして扱う画面。プレイ・決定・プラクティスでは `None`。
    pub(super) fn system_button_screen(&self) -> Option<QuickMenuScreen> {
        if self.jobs.profile_change.is_some() {
            return None;
        }
        if self.viewer_waiting {
            return Some(QuickMenuScreen::ViewerWaiting);
        }
        if self.play.practice_session.is_some() || self.play.play_ending.is_some() {
            return None;
        }
        match self.view_state() {
            AppViewState::Select if in_settings_stack(&self.select.folder_stack) => {
                Some(QuickMenuScreen::SettingsFolder)
            }
            AppViewState::Select => Some(QuickMenuScreen::Select),
            AppViewState::Result => Some(QuickMenuScreen::Result),
            AppViewState::Decide | AppViewState::Play => None,
        }
    }

    fn key_config_listening_in_settings(&self) -> bool {
        in_settings_stack(&self.select.folder_stack)
            && self.select.key_config_edit.as_ref().is_some_and(|session| session.listening)
    }

    pub(super) fn quick_menu_context(&self) -> QuickMenuContext {
        QuickMenuContext {
            key_binding_selected: in_settings_stack(&self.select.folder_stack)
                && self.select.key_config_edit.is_none()
                && self.select.settings_edit.is_none()
                && matches!(
                    self.select.select_items.get(self.select.selected_index),
                    Some(SelectItem::KeyBinding(_))
                ),
        }
    }

    /// ゲームパッドのボタン入力をE4レイヤーで処理する。true なら通常の経路へ渡さない。
    pub(super) fn route_system_button(&mut self, event: &ControlInputEvent) -> bool {
        let Some(control) = event.name.as_deref() else {
            return false;
        };
        let Some(screen) = self.system_button_screen() else {
            self.reset_system_button();
            self.close_quick_menu(false);
            return false;
        };
        if self.ui.quick_menu.menu.as_ref().is_some_and(|menu| menu.screen() != screen) {
            self.close_quick_menu(false);
        }
        let input = classify_system_input(&self.select.select_keys, control);
        let now = Instant::now();
        if self.key_config_listening_in_settings() {
            return self.route_system_button_while_listening(control, input, event.pressed, now);
        }

        if !event.pressed {
            let layer_event = self.input.system_button.release(input, now);
            if input != SystemInput::E4 || layer_event == LayerEvent::Pass {
                return false;
            }
            let press_screen = self.ui.quick_menu.press_screen.take();
            self.ui.quick_menu.page_flick.reset();
            self.sync_select_holds_from_pressed_controls();
            if layer_event == LayerEvent::Tap && press_screen == Some(screen) {
                self.handle_system_button_tap(screen);
            }
            return true;
        }

        if self.ui.quick_menu.is_open() {
            if input == SystemInput::E4 {
                // 閉じたE4を離しても開き直さない。押している間の E4 + E3 などは使える。
                self.begin_system_button_hold(screen, now, true);
            }
            let skip_analog_edge = event.uses_analog_scroll(&self.select.select_keys);
            if let Some(menu_input) = quick_menu_input_for(input)
                && !(skip_analog_edge
                    && matches!(menu_input, QuickMenuInput::Up | QuickMenuInput::Down))
            {
                self.apply_quick_menu_input(menu_input);
            }
            return true;
        }

        if input == SystemInput::E4 {
            let others_held =
                self.input.pressed_controls.iter().any(|pressed| pressed.as_str() != control);
            self.begin_system_button_hold(screen, now, others_held);
            return true;
        }
        match self.input.system_button.press(input, now, false) {
            LayerEvent::Pass => false,
            LayerEvent::Chord(chord) => {
                self.handle_system_button_chord(screen, chord, event);
                true
            }
            LayerEvent::Swallow | LayerEvent::E4Down | LayerEvent::Tap | LayerEvent::LongHold => {
                true
            }
        }
    }

    fn begin_system_button_hold(&mut self, screen: QuickMenuScreen, now: Instant, spent: bool) {
        if self.input.system_button.press(SystemInput::E4, now, spent) == LayerEvent::E4Down {
            self.ui.quick_menu.press_screen = Some(screen);
            self.ui.quick_menu.page_flick.reset();
            // E1 / E2 のパネルなどを、E4を押している間は閉じる。
            self.sync_select_holds_from_pressed_controls();
        }
    }

    /// キーコンフィグの待ち受け中。E4の短押しはE4自身の割り当て、1秒の長押しは取消。
    fn route_system_button_while_listening(
        &mut self,
        control: &str,
        input: SystemInput,
        pressed: bool,
        now: Instant,
    ) -> bool {
        if input != SystemInput::E4 {
            if pressed {
                // ほかの入力は従来どおり割り当てる。押していたE4は短押しにしない。
                self.input.system_button.mark_spent();
            }
            return false;
        }
        if pressed {
            let others_held =
                self.input.pressed_controls.iter().any(|pressed| pressed.as_str() != control);
            self.input.system_button.press(SystemInput::E4, now, others_held);
            return true;
        }
        match listening_action_for(Some(self.input.system_button.release(SystemInput::E4, now))) {
            ListeningAction::BindE4 => self.apply_key_config_gamepad(control),
            ListeningAction::Cancel => self.cancel_key_config_edit(),
            ListeningAction::None => {}
        }
        self.ui.quick_menu.press_screen = None;
        self.sync_select_holds_from_pressed_controls();
        true
    }

    fn handle_system_button_tap(&mut self, screen: QuickMenuScreen) {
        if let Some(egui) = self.ui.egui.as_mut()
            && egui.is_menu_visible()
        {
            egui.close_menu();
            self.update_select_option_panel();
            return;
        }
        let blocked = match screen {
            QuickMenuScreen::SettingsFolder => self.select.settings_edit.is_some(),
            QuickMenuScreen::Result => self.result.result_exit.is_some(),
            QuickMenuScreen::Select | QuickMenuScreen::ViewerWaiting => false,
        };
        if blocked {
            return;
        }
        let context = self.quick_menu_context();
        let Some(menu) = QuickMenu::open(screen, &context) else {
            return;
        };
        self.ui.quick_menu.menu = Some(menu);
        self.ui.quick_menu.menu_analog.reset();
        // 開く前に動いていた一覧の長押し移動・回転量を止める。
        self.clear_select_hold();
        self.reset_select_analog_scroll();
        self.play_system_sound(crate::system_sound::SoundType::OptionOpen);
        tracing::info!(?screen, "quick menu opened");
    }

    fn handle_system_button_chord(
        &mut self,
        screen: QuickMenuScreen,
        chord: SystemInput,
        event: &ControlInputEvent,
    ) {
        match chord {
            SystemInput::E3 => self.request_manual_screenshot(),
            SystemInput::ScratchUp | SystemInput::ScratchDown => {
                // アナログ皿の合成ボタンは回転量（route_system_axis_ticks）で扱う。
                if screen == QuickMenuScreen::Select
                    && !event.uses_analog_scroll(&self.select.select_keys)
                {
                    let select_move = if chord == SystemInput::ScratchUp {
                        SelectMove::PagePrevious
                    } else {
                        SelectMove::PageNext
                    };
                    self.apply_select_action(SelectAction::Move(select_move), None);
                }
            }
            SystemInput::Key(lane) => {
                let context = self.quick_menu_context();
                if let Some(action) =
                    shortcut_action_for(quick_menu_shortcuts(screen), lane, &context)
                {
                    self.execute_quick_menu_action(action);
                }
            }
            // E1 / E2 はE4 hold中は使わない。
            SystemInput::E1 | SystemInput::E2 | SystemInput::E4 | SystemInput::Other => {}
        }
    }

    fn apply_quick_menu_input(&mut self, input: QuickMenuInput) {
        let Some(menu) = self.ui.quick_menu.menu.as_mut() else {
            return;
        };
        match menu.handle(input) {
            QuickMenuCommand::None => {}
            QuickMenuCommand::Moved => {
                self.play_system_sound(crate::system_sound::SoundType::Scratch);
            }
            QuickMenuCommand::Close => self.close_quick_menu(true),
            QuickMenuCommand::Execute(action) => {
                self.close_quick_menu(false);
                tracing::info!(?action, "quick menu action");
                self.execute_quick_menu_action(action);
            }
        }
    }

    pub(super) fn close_quick_menu(&mut self, play_sound: bool) {
        if self.ui.quick_menu.menu.take().is_none() {
            return;
        }
        self.ui.quick_menu.menu_analog.reset();
        if play_sound {
            self.play_system_sound(crate::system_sound::SoundType::OptionClose);
        }
    }

    /// クイックメニュー項目・近道の操作を実行する。段階2の項目もここへ足す。
    pub(super) fn execute_quick_menu_action(&mut self, action: QuickMenuAction) {
        match action {
            QuickMenuAction::Select(select_action) => {
                self.apply_select_action(select_action, None);
            }
            QuickMenuAction::KeyConfigClear => self.clear_selected_key_binding(),
            QuickMenuAction::KeyConfigRestoreDefault => {
                self.restore_selected_key_binding_default();
            }
            QuickMenuAction::OpenBmzMenu => {
                if let Some(egui) = self.ui.egui.as_mut() {
                    egui.open_menu();
                }
                self.update_select_option_panel();
                self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
            }
            QuickMenuAction::ConfirmExit => {
                tracing::info!("exit requested from quick menu");
                self.ui.quick_menu.exit_requested = true;
            }
            // QuickMenu がページを切り替える。
            QuickMenuAction::RequestExit | QuickMenuAction::CancelExit => {}
        }
    }

    /// アナログ皿のtick。メニュー表示中はカーソル、E4 hold中は選曲のページ移動に使う。
    pub(super) fn route_system_axis_ticks(&mut self, axis: &str, ticks: i32) -> bool {
        let Some(screen) = self.system_button_screen() else {
            return false;
        };
        let delta = select_analog_scroll_delta(axis, ticks, &self.select.select_keys);
        let now = Instant::now();
        let ticks_per_scroll = self.boot.profile_config.input.analog_ticks_per_scroll.max(1) as i32;
        let idle_after = Duration::from_millis(SELECT_ANALOG_SCROLL_TOLERANCE_MS);
        if let Some(menu) = self.ui.quick_menu.menu.as_mut() {
            if let Some(delta) = delta {
                let steps =
                    self.ui.quick_menu.menu_analog.push(delta, now, ticks_per_scroll, idle_after);
                if menu.move_cursor(steps) {
                    self.play_system_sound(crate::system_sound::SoundType::Scratch);
                }
            }
            return true;
        }
        if !self.input.system_button.held() {
            return false;
        }
        self.input.system_button.mark_spent();
        if screen == QuickMenuScreen::Select
            && let Some(delta) = delta
            && let Some(direction) =
                self.ui.quick_menu.page_flick.push(delta, now, ticks_per_scroll, idle_after)
        {
            let select_move =
                if direction > 0 { SelectMove::PageNext } else { SelectMove::PagePrevious };
            self.apply_select_action(SelectAction::Move(select_move), None);
        }
        true
    }

    /// frameごとの更新。長押しの判定と、画面が変わったときの片付けを行う。
    pub(super) fn tick_system_button(&mut self) {
        let Some(screen) = self.system_button_screen() else {
            self.reset_system_button();
            self.close_quick_menu(false);
            return;
        };
        if self.ui.quick_menu.menu.as_ref().is_some_and(|menu| menu.screen() != screen) {
            self.close_quick_menu(false);
        }
        if self.input.system_button.held() && !self.system_button_physically_held() {
            // Releaseをbounce filter等で受け取れなかった場合も、物理状態を正とする。
            self.reset_system_button();
            return;
        }
        let long_hold = self.input.system_button.tick(Instant::now());
        if self.key_config_listening_in_settings()
            && listening_action_for(long_hold) == ListeningAction::Cancel
        {
            self.cancel_key_config_edit();
        }
    }

    pub(super) fn system_button_physically_held(&self) -> bool {
        self.input.pressed_control_sources().any(|(device, control)| {
            *device != W_KEYBOARD_DEVICE_ID
                && self.select.select_keys.e_action_for_control(control)
                    == Some(InputActionConfig::E4)
        })
    }

    pub(super) fn reset_system_button(&mut self) {
        let was_held = self.input.system_button.held();
        self.input.system_button.reset();
        self.ui.quick_menu.press_screen = None;
        self.ui.quick_menu.page_flick.reset();
        if was_held {
            self.sync_select_holds_from_pressed_controls();
        }
    }

    /// egui へ渡すクイックメニューの表示内容。
    pub(super) fn quick_menu_panel(&self) -> Option<crate::quick_menu_ui::QuickMenuPanel> {
        let menu = self.ui.quick_menu.menu.as_ref()?;
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let title = text.text(match menu.page() {
            QuickMenuPage::Main => "quick-menu-title",
            QuickMenuPage::ConfirmExit => "quick-menu-exit-title",
        });
        let items = menu
            .items()
            .into_iter()
            .map(|item| crate::quick_menu_ui::QuickMenuPanelItem {
                label: text.text(item.label_key),
                shortcut: item.shortcut.map(shortcut_label),
            })
            .collect();
        Some(crate::quick_menu_ui::QuickMenuPanel {
            title,
            items,
            cursor: menu.cursor(),
            hint: text.text("quick-menu-hint"),
        })
    }
}

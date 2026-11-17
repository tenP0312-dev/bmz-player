use std::time::{Duration, Instant};

use bmz_core::lane::Lane;

use super::layer::SYSTEM_BUTTON_LONG_HOLD;
use super::menu::{AnalogStepper, FlickDetector};
use super::registry::{QuickMenuShortcut, quick_menu_entries, shortcut_lane_for};
use super::*;
use crate::app::input_runtime::{AppInputRuntime, ControlInputEvent};
use crate::app::play_support::reconcile_select_holds_from_pressed_controls;
use crate::config::profile_config::ProfileConfig;
use bmz_gameplay::input::backend::DeviceId;

const PAD: DeviceId = DeviceId(2);
const E1: &str = "Button9";
const E2: &str = "Button10";
const E3: &str = "Button11";
const E4: &str = "Button12";

fn default_keys() -> SelectKeyBindings {
    SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input)
}

fn ms(base: Instant, millis: u64) -> Instant {
    base + Duration::from_millis(millis)
}

#[test]
fn e4_press_and_release_without_other_input_is_a_tap() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    assert_eq!(layer.press(SystemInput::E4, start, false), LayerEvent::E4Down);
    assert!(layer.held());
    assert_eq!(layer.tick(ms(start, 200)), None);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 300)), LayerEvent::Tap);
    assert!(!layer.held());
}

#[test]
fn e4_with_other_input_in_between_is_a_chord_not_a_tap() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    layer.press(SystemInput::E4, start, false);
    assert_eq!(
        layer.press(SystemInput::E3, ms(start, 50), false),
        LayerEvent::Chord(SystemInput::E3)
    );
    // ほかの入力のReleaseは常に通常の経路へ渡す。
    assert_eq!(layer.release(SystemInput::E3, ms(start, 80)), LayerEvent::Pass);
    assert_eq!(
        layer.press(SystemInput::Key(Lane::Key2), ms(start, 90), false),
        LayerEvent::Chord(SystemInput::Key(Lane::Key2))
    );
    assert_eq!(layer.release(SystemInput::E4, ms(start, 120)), LayerEvent::Swallow);

    // 次の押下は新しい判定になる。
    layer.press(SystemInput::E4, ms(start, 500), false);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 600)), LayerEvent::Tap);
}

#[test]
fn e4_pressed_while_other_input_is_held_does_not_tap() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    layer.press(SystemInput::E4, start, true);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 100)), LayerEvent::Swallow);
}

#[test]
fn analog_input_during_e4_hold_spends_the_tap() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    layer.press(SystemInput::E4, start, false);
    layer.mark_spent();
    assert_eq!(layer.tick(ms(start, 1500)), None);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 1600)), LayerEvent::Swallow);
}

#[test]
fn e4_long_hold_fires_once_and_release_is_not_a_tap() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    layer.press(SystemInput::E4, start, false);
    assert_eq!(layer.tick(start + SYSTEM_BUTTON_LONG_HOLD - Duration::from_millis(1)), None);
    assert_eq!(layer.tick(start + SYSTEM_BUTTON_LONG_HOLD), Some(LayerEvent::LongHold));
    assert_eq!(layer.tick(ms(start, 1500)), None);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 1600)), LayerEvent::Swallow);
}

#[test]
fn e4_long_release_before_frame_tick_reports_long_hold() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    layer.press(SystemInput::E4, start, false);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 1200)), LayerEvent::LongHold);
}

#[test]
fn input_without_e4_hold_passes_and_duplicate_e4_press_keeps_first_hold() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();
    assert_eq!(layer.press(SystemInput::E1, start, false), LayerEvent::Pass);
    assert_eq!(layer.release(SystemInput::E4, start), LayerEvent::Pass);
    layer.press(SystemInput::E4, start, false);
    assert_eq!(layer.press(SystemInput::E4, ms(start, 10), false), LayerEvent::Swallow);
    assert_eq!(layer.release(SystemInput::E4, ms(start, 20)), LayerEvent::Tap);
}

#[test]
fn reset_discards_a_held_e4_like_focus_loss() {
    let mut input = AppInputRuntime::default();
    let start = Instant::now();
    input.system_button.press(SystemInput::E4, start, false);
    let _ = input.handle_focus_lost();
    assert!(!input.system_button.held());
    assert_eq!(input.system_button.release(SystemInput::E4, ms(start, 50)), LayerEvent::Pass);
}

#[test]
fn classify_uses_key_config_assignments() {
    let keys = default_keys();
    assert_eq!(classify_system_input(&keys, E4), SystemInput::E4);
    assert_eq!(classify_system_input(&keys, E3), SystemInput::E3);
    assert_eq!(classify_system_input(&keys, E2), SystemInput::E2);
    assert_eq!(classify_system_input(&keys, E1), SystemInput::E1);
    assert_eq!(classify_system_input(&keys, "Axis1+"), SystemInput::ScratchUp);
    assert_eq!(classify_system_input(&keys, "Axis1-"), SystemInput::ScratchDown);
    assert_eq!(classify_system_input(&keys, "Button1"), SystemInput::Key(Lane::Key1));
    assert_eq!(classify_system_input(&keys, "Button6"), SystemInput::Key(Lane::Key6));
    assert_eq!(classify_system_input(&keys, "Button40"), SystemInput::Other);

    // E4は固定のボタンではなく、キーコンフィグで割り当てた入力。
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    crate::config::key_config::apply_play_binding(
        &mut profile.input,
        bmz_core::lane::KeyMode::K7,
        crate::config::key_config::KeyBindingTarget::Action {
            action: InputActionConfig::E4,
            slot: crate::config::key_config::KeyBindingSlot::Controller,
        },
        "Button20",
    )
    .unwrap();
    let keys = SelectKeyBindings::from_profile(&profile.input);
    assert_eq!(classify_system_input(&keys, "Button20"), SystemInput::E4);
    assert_eq!(classify_system_input(&keys, E4), SystemInput::Other);
}

#[test]
fn e4_hold_suppresses_e1_e2_e3_holds() {
    let keys = default_keys();
    let mut input = AppInputRuntime::default();
    for control in [E1, E2, E3, E4] {
        input.track_control(&ControlInputEvent::gamepad(PAD, control, true));
    }
    reconcile_select_holds_from_pressed_controls(&mut input, &keys);
    assert!(input.start_held && input.select_held);
    assert!(input.select_e_action_holds.contains(&InputActionConfig::E3));

    input.system_button.press(SystemInput::E4, Instant::now(), true);
    reconcile_select_holds_from_pressed_controls(&mut input, &keys);
    assert!(!input.start_held, "E1 panel stays closed while E4 is held");
    assert!(!input.select_held, "E2 panel stays closed while E4 is held");
    assert_eq!(
        input.select_e_action_holds.iter().copied().collect::<Vec<_>>(),
        vec![InputActionConfig::E4]
    );

    input.system_button.release(SystemInput::E4, Instant::now());
    input.track_control(&ControlInputEvent::gamepad(PAD, E4, false));
    reconcile_select_holds_from_pressed_controls(&mut input, &keys);
    assert!(input.start_held && input.select_held, "physical holds return after E4");
}

#[test]
fn quick_menu_input_maps_scratch_odd_and_even_keys() {
    assert_eq!(quick_menu_input_for(SystemInput::ScratchUp), Some(QuickMenuInput::Up));
    assert_eq!(quick_menu_input_for(SystemInput::ScratchDown), Some(QuickMenuInput::Down));
    for lane in [Lane::Key1, Lane::Key3, Lane::Key5, Lane::Key7] {
        assert_eq!(quick_menu_input_for(SystemInput::Key(lane)), Some(QuickMenuInput::Run));
    }
    for lane in [Lane::Key2, Lane::Key4, Lane::Key6] {
        assert_eq!(quick_menu_input_for(SystemInput::Key(lane)), Some(QuickMenuInput::Close));
    }
    assert_eq!(quick_menu_input_for(SystemInput::E2), Some(QuickMenuInput::Close));
    assert_eq!(quick_menu_input_for(SystemInput::E4), Some(QuickMenuInput::Close));
    assert_eq!(quick_menu_input_for(SystemInput::E1), None);
    assert_eq!(quick_menu_input_for(SystemInput::E3), None);
}

#[test]
fn select_quick_menu_moves_with_wrap_and_runs_items() {
    let mut menu = QuickMenu::open(QuickMenuScreen::Select, &QuickMenuContext::default()).unwrap();
    let actions: Vec<_> = menu.items().iter().map(|item| item.action).collect();
    assert_eq!(
        actions,
        vec![
            QuickMenuAction::Select(SelectAction::Move(SelectMove::First)),
            QuickMenuAction::Select(SelectAction::Move(SelectMove::Last)),
            QuickMenuAction::OpenBmzMenu,
            QuickMenuAction::RequestExit,
        ]
    );
    assert_eq!(menu.handle(QuickMenuInput::Up), QuickMenuCommand::Moved);
    assert_eq!(menu.cursor(), 3, "cursor wraps to the last item");
    assert_eq!(menu.handle(QuickMenuInput::Down), QuickMenuCommand::Moved);
    assert_eq!(menu.handle(QuickMenuInput::Down), QuickMenuCommand::Moved);
    assert_eq!(
        menu.handle(QuickMenuInput::Run),
        QuickMenuCommand::Execute(QuickMenuAction::Select(SelectAction::Move(SelectMove::Last)))
    );
    assert_eq!(menu.handle(QuickMenuInput::Close), QuickMenuCommand::Close);
}

#[test]
fn exit_needs_confirmation_and_starts_on_cancel() {
    let mut menu = QuickMenu::open(QuickMenuScreen::Select, &QuickMenuContext::default()).unwrap();
    menu.move_cursor(3);
    assert_eq!(menu.handle(QuickMenuInput::Run), QuickMenuCommand::Moved);
    assert_eq!(menu.page(), QuickMenuPage::ConfirmExit);
    assert_eq!(menu.items()[menu.cursor()].action, QuickMenuAction::CancelExit);

    // 取消（実行・閉じる操作のどちらでも）で元の一覧と位置へ戻る。
    assert_eq!(menu.handle(QuickMenuInput::Run), QuickMenuCommand::Moved);
    assert_eq!((menu.page(), menu.cursor()), (QuickMenuPage::Main, 3));
    menu.handle(QuickMenuInput::Run);
    assert_eq!(menu.handle(QuickMenuInput::Close), QuickMenuCommand::Moved);
    assert_eq!(menu.page(), QuickMenuPage::Main);

    menu.handle(QuickMenuInput::Run);
    menu.handle(QuickMenuInput::Up);
    assert_eq!(
        menu.handle(QuickMenuInput::Run),
        QuickMenuCommand::Execute(QuickMenuAction::ConfirmExit)
    );
}

#[test]
fn settings_folder_offers_key_binding_items_only_on_a_slot() {
    let without = QuickMenu::open(QuickMenuScreen::SettingsFolder, &QuickMenuContext::default())
        .unwrap()
        .items();
    assert!(!without.iter().any(|item| item.action == QuickMenuAction::KeyConfigClear));

    let context = QuickMenuContext { key_binding_selected: true };
    let with = QuickMenu::open(QuickMenuScreen::SettingsFolder, &context).unwrap().items();
    assert_eq!(with[0].action, QuickMenuAction::KeyConfigClear);
    assert_eq!(with[1].action, QuickMenuAction::KeyConfigRestoreDefault);
}

#[test]
fn every_screen_has_items_with_catalog_labels() {
    let catalog = include_str!("../../i18n/locales/ja.ftl");
    for screen in [
        QuickMenuScreen::Select,
        QuickMenuScreen::SettingsFolder,
        QuickMenuScreen::Result,
        QuickMenuScreen::ViewerWaiting,
    ] {
        assert!(QuickMenu::open(screen, &QuickMenuContext::default()).is_some(), "{screen:?}");
        let labels = quick_menu_entries(screen)
            .iter()
            .chain(registry::EXIT_CONFIRM_ITEMS)
            .map(|entry| entry.label_key)
            .chain(["quick-menu-title", "quick-menu-hint", "quick-menu-exit-title"]);
        for label in labels {
            assert!(catalog.contains(&format!("\n{label} = ")), "missing {label}");
        }
    }
}

#[test]
fn shortcut_table_resolves_lanes_and_respects_availability() {
    let table = [
        QuickMenuShortcut { lane: Lane::Key1, action: QuickMenuAction::OpenBmzMenu },
        QuickMenuShortcut { lane: Lane::Key2, action: QuickMenuAction::KeyConfigClear },
    ];
    let context = QuickMenuContext::default();
    assert_eq!(
        shortcut_action_for(&table, Lane::Key1, &context),
        Some(QuickMenuAction::OpenBmzMenu)
    );
    assert_eq!(shortcut_action_for(&table, Lane::Key2, &context), None);
    assert_eq!(
        shortcut_action_for(&table, Lane::Key2, &QuickMenuContext { key_binding_selected: true }),
        Some(QuickMenuAction::KeyConfigClear)
    );
    assert_eq!(shortcut_action_for(&table, Lane::Key3, &context), None);
    assert_eq!(shortcut_lane_for(&table, QuickMenuAction::OpenBmzMenu), Some(Lane::Key1));
    assert_eq!(shortcut_label(Lane::Key7), "E4+KEY7");
}

#[test]
fn analog_stepper_converts_ticks_and_drops_stale_remainder() {
    let mut stepper = AnalogStepper::default();
    let start = Instant::now();
    let idle = Duration::from_millis(200);
    assert_eq!(stepper.push(3, start, 4, idle), 0);
    assert_eq!(stepper.push(6, ms(start, 50), 4, idle), 2);
    // 途切れた後の端数（1）は捨てる。
    assert_eq!(stepper.push(-3, ms(start, 400), 4, idle), 0);
    assert_eq!(stepper.push(-1, ms(start, 420), 4, idle), -1);
}

#[test]
fn flick_detector_fires_once_per_spin() {
    let mut flick = FlickDetector::default();
    let start = Instant::now();
    let idle = Duration::from_millis(200);
    assert_eq!(flick.push(2, start, 4, idle), None);
    assert_eq!(flick.push(2, ms(start, 20), 4, idle), Some(1));
    assert_eq!(flick.push(20, ms(start, 40), 4, idle), None, "one page per spin");
    assert_eq!(flick.push(-4, ms(start, 400), 4, idle), Some(-1));
}

#[test]
fn key_config_listening_binds_e4_on_short_press_and_cancels_after_one_second() {
    let mut layer = SystemButtonLayer::default();
    let start = Instant::now();

    // 短押し: E4自身を割り当てる（押した時点では確定しない）。
    layer.press(SystemInput::E4, start, false);
    assert_eq!(listening_action_for(layer.tick(ms(start, 300))), ListeningAction::None);
    assert_eq!(
        listening_action_for(Some(layer.release(SystemInput::E4, ms(start, 400)))),
        ListeningAction::BindE4
    );

    // 1秒の長押し: 押している間に取り消し、離しても割り当てない。
    layer.press(SystemInput::E4, ms(start, 1000), false);
    assert_eq!(listening_action_for(layer.tick(ms(start, 1999))), ListeningAction::None);
    assert_eq!(listening_action_for(layer.tick(ms(start, 2000))), ListeningAction::Cancel);
    assert_eq!(
        listening_action_for(Some(layer.release(SystemInput::E4, ms(start, 2500)))),
        ListeningAction::None
    );

    // E4を押したまま別のボタンを割り当てた場合、E4のReleaseでは何もしない。
    layer.press(SystemInput::E4, ms(start, 3000), false);
    layer.mark_spent();
    assert_eq!(
        listening_action_for(Some(layer.release(SystemInput::E4, ms(start, 3100)))),
        ListeningAction::None
    );
}

use super::*;
use crate::config::play_input::default_profile_input;
use crate::random_option_seed::RANDOM_OPTION_SEED_MAX;
use crate::screens::play_session::{seed_permutation, source_lane};
use bmz_gameplay::input::binding::{BindingEntry, LaneBinding};

/// A binding that gives every lane its own keyboard control `T<lane index>`.
fn input_for(key_mode: KeyMode) -> PlayOptionInput {
    let entries = Lane::ALL
        .into_iter()
        .map(|lane| BindingEntry {
            device: None,
            control: PhysicalControl::KeyboardKey(format!("T{}", lane.index())),
            lane,
            scratch_direction: None,
        })
        .collect();
    let profile = default_profile_input();
    let mut input = PlayOptionInput::new(
        key_mode,
        LaneBinding { entries },
        &profile,
        crate::input::gamepad::GamepadSlotMap::default(),
    );
    input.action_bindings = vec![
        action_binding("S1", InputActionConfig::E1),
        action_binding("S4", InputActionConfig::E4),
    ];
    input
}

fn action_binding(control: &str, action: InputActionConfig) -> PlayActionBinding {
    PlayActionBinding {
        device: None,
        control: PhysicalControl::KeyboardKey(control.to_string()),
        action,
    }
}

fn key(name: &str) -> (DeviceId, PhysicalControl) {
    (W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey(name.to_string()))
}

fn lane_key(lane: Lane) -> (DeviceId, PhysicalControl) {
    key(&format!("T{}", lane.index()))
}

fn held(items: &[(DeviceId, PhysicalControl)]) -> HashSet<(DeviceId, PhysicalControl)> {
    items.iter().cloned().collect()
}

fn gate(key_mode: KeyMode) -> OneBassGate {
    OneBassGate {
        enabled: true,
        key_mode,
        plain_launch: true,
        in_course: false,
        random_trainer: false,
        cli_layout: false,
    }
}

fn random_options(seed: i64) -> PlayStartOptions {
    PlayStartOptions {
        arrange: ArrangeOption::Random,
        arrange_2p: ArrangeOption::Random,
        arrange_seed: Some(seed),
        arrange_seed_2p: Some(seed + 1),
        ..Default::default()
    }
}

/// Entropy that counts its calls and hands out increasing seeds.
fn counting_entropy(calls: &std::cell::Cell<u32>) -> impl FnMut() -> RandomOptionSeed + '_ {
    move || {
        calls.set(calls.get() + 1);
        RandomOptionSeed::new(1_000 + calls.get() * 7_919).unwrap()
    }
}

fn plan(
    gate: &OneBassGate,
    options: &mut PlayStartOptions,
    pressed: &[(DeviceId, PhysicalControl)],
) -> Vec<OneBassApplied> {
    let input = input_for(gate.key_mode);
    let calls = std::cell::Cell::new(0);
    plan_one_bass(gate, options, &held(pressed), &input, &mut counting_entropy(&calls))
}

fn bass_lane_after(options: &PlayStartOptions, key_mode: KeyMode, side: OneBassSide) -> usize {
    let seed = match side {
        OneBassSide::P1 => options.arrange_seed,
        OneBassSide::P2 => options.arrange_seed_2p,
    };
    let seed = RandomOptionSeed::new(u32::try_from(seed.unwrap()).unwrap()).unwrap();
    let source = source_lane(key_mode, side).unwrap();
    seed_permutation(key_mode, side, seed).unwrap()[source.index()]
}

#[test]
fn start_requires_exactly_e1_and_not_e4() {
    let input = input_for(KeyMode::K7);
    assert!(one_bass_start_held(&held(&[key("S1")]), &input));
    assert!(!one_bass_start_held(&held(&[key("S4")]), &input), "E4 is not START here");
    assert!(!one_bass_start_held(&held(&[lane_key(Lane::Key4)]), &input));
    assert!(!one_bass_start_held(&held(&[]), &input));
}

#[test]
fn keys_are_read_from_the_pressed_set_only_when_start_is_held() {
    for lane in [Lane::Key1, Lane::Key4, Lane::Key7] {
        let mut options = random_options(5);
        let applied = plan(&gate(KeyMode::K7), &mut options, &[key("S1"), lane_key(lane)]);
        assert_eq!(applied.len(), 1, "{lane:?}");
        assert_eq!(applied[0].target, lane);
        assert_eq!(bass_lane_after(&options, KeyMode::K7, OneBassSide::P1), lane.index());
        assert!(applied[0].seed.value() <= RANDOM_OPTION_SEED_MAX);
    }
    let mut options = random_options(5);
    assert!(plan(&gate(KeyMode::K7), &mut options, &[lane_key(Lane::Key4)]).is_empty());
    assert_eq!(options.arrange_seed, Some(5), "no START: nothing changes");
}

#[test]
fn zero_or_several_keys_do_not_act_and_the_scratch_is_ignored() {
    let mut options = random_options(5);
    assert!(plan(&gate(KeyMode::K7), &mut options, &[key("S1")]).is_empty());
    let two = [key("S1"), lane_key(Lane::Key2), lane_key(Lane::Key5)];
    assert!(plan(&gate(KeyMode::K7), &mut options, &two).is_empty());
    assert_eq!(options.arrange_seed, Some(5));

    let with_scratch = [key("S1"), lane_key(Lane::Key3), lane_key(Lane::Scratch)];
    let applied = plan(&gate(KeyMode::K7), &mut options, &with_scratch);
    assert_eq!(applied[0].target, Lane::Key3);
    let scratch_only = [key("S1"), lane_key(Lane::Scratch)];
    let mut options = random_options(5);
    assert!(plan(&gate(KeyMode::K7), &mut options, &scratch_only).is_empty());
}

#[test]
fn only_a_plain_random_side_is_pinned() {
    for arrange in [
        ArrangeOption::Normal,
        ArrangeOption::Mirror,
        ArrangeOption::RRandom,
        ArrangeOption::SRandom,
        ArrangeOption::HRandom,
    ] {
        let mut options = PlayStartOptions { arrange, ..random_options(5) };
        let applied = plan(&gate(KeyMode::K7), &mut options, &[key("S1"), lane_key(Lane::Key4)]);
        assert!(applied.is_empty(), "{arrange:?}");
        assert_eq!(options.arrange_seed, Some(5));
    }
}

#[test]
fn a_seed_that_already_pins_the_bass_is_kept_and_needs_no_reload() {
    let target = Lane::Key5;
    let source = source_lane(KeyMode::K7, OneBassSide::P1).unwrap();
    let pinned = (0..=RANDOM_OPTION_SEED_MAX)
        .find(|&value| {
            let seed = RandomOptionSeed::new(value).unwrap();
            seed_permutation(KeyMode::K7, OneBassSide::P1, seed).unwrap()[source.index()]
                == target.index()
        })
        .unwrap();
    let mut options = random_options(i64::from(pinned));
    let calls = std::cell::Cell::new(0);
    let input = input_for(KeyMode::K7);
    let applied = plan_one_bass(
        &gate(KeyMode::K7),
        &mut options,
        &held(&[key("S1"), lane_key(target)]),
        &input,
        &mut counting_entropy(&calls),
    );
    assert_eq!(options.arrange_seed, Some(i64::from(pinned)));
    assert_eq!(calls.get(), 0, "no fresh seed is drawn");
    assert_eq!(applied.len(), 1);
    assert!(!one_bass_needs_reload(&applied), "the same seed does not reload the chart");
}

#[test]
fn a_changed_seed_asks_for_one_reload() {
    // Seed 0 does not put the bass on Key7 for all of these; find a case that must change.
    let mut options = random_options(0);
    let applied = plan(&gate(KeyMode::K7), &mut options, &[key("S1"), lane_key(Lane::Key7)]);
    assert_eq!(applied.len(), 1);
    assert_eq!(
        applied[0].changed(),
        applied[0].previous != Some(i64::from(applied[0].seed.value()))
    );
    assert_eq!(one_bass_needs_reload(&applied), applied[0].changed());
    assert!(!one_bass_needs_reload(&[]));
}

#[test]
fn a_borrowed_layout_swaps_two_lanes_and_keeps_zero_as_a_seed() {
    let source = source_lane(KeyMode::K7, OneBassSide::P1).unwrap();
    for borrowed_seed in [0_i64, 1, 777, 16_777_215] {
        let before = seed_permutation(
            KeyMode::K7,
            OneBassSide::P1,
            RandomOptionSeed::new(u32::try_from(borrowed_seed).unwrap()).unwrap(),
        )
        .unwrap();
        for target in [Lane::Key1, Lane::Key4, Lane::Key7] {
            let mut options =
                PlayStartOptions { arrange_seed_borrowed: true, ..random_options(borrowed_seed) };
            let calls = std::cell::Cell::new(0);
            let input = input_for(KeyMode::K7);
            let applied = plan_one_bass(
                &gate(KeyMode::K7),
                &mut options,
                &held(&[key("S1"), lane_key(target)]),
                &input,
                &mut counting_entropy(&calls),
            );
            assert_eq!(applied.len(), 1);
            assert_eq!(calls.get(), 0, "a borrowed layout draws no fresh seed");
            let after = seed_permutation(KeyMode::K7, OneBassSide::P1, applied[0].seed).unwrap();
            assert_eq!(after[source.index()], target.index());
            let differing = (0..before.len()).filter(|&lane| before[lane] != after[lane]).count();
            assert!(differing <= 2, "seed {borrowed_seed} -> {target:?}: {differing} lanes differ");
        }
    }
}

#[test]
fn a_borrowed_layout_without_a_seed_draws_a_fresh_one() {
    let mut options = PlayStartOptions {
        arrange_seed_borrowed: true,
        arrange: ArrangeOption::Random,
        arrange_seed: None,
        ..Default::default()
    };
    let calls = std::cell::Cell::new(0);
    let input = input_for(KeyMode::K7);
    let applied = plan_one_bass(
        &gate(KeyMode::K7),
        &mut options,
        &held(&[key("S1"), lane_key(Lane::Key6)]),
        &input,
        &mut counting_entropy(&calls),
    );
    assert_eq!(applied.len(), 1);
    assert!(calls.get() >= 1);
    assert_eq!(bass_lane_after(&options, KeyMode::K7, OneBassSide::P1), Lane::Key6.index());
    assert!(applied[0].changed());
}

#[test]
fn double_play_sides_are_decided_separately() {
    let both = [key("S1"), lane_key(Lane::Key3), lane_key(Lane::Key11)];
    let mut options = random_options(10);
    let applied = plan(&gate(KeyMode::K14), &mut options, &both);
    assert_eq!(applied.len(), 2);
    assert_eq!(bass_lane_after(&options, KeyMode::K14, OneBassSide::P1), Lane::Key3.index());
    assert_eq!(bass_lane_after(&options, KeyMode::K14, OneBassSide::P2), Lane::Key11.index());

    // One key on 1P, two on 2P: only 1P acts and the 2P seed is untouched.
    let mixed = [key("S1"), lane_key(Lane::Key3), lane_key(Lane::Key10), lane_key(Lane::Key12)];
    let mut options = random_options(10);
    let applied = plan(&gate(KeyMode::K14), &mut options, &mixed);
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].side, OneBassSide::P1);
    assert_eq!(options.arrange_seed_2p, Some(11));

    // A 2P side that is not RANDOM is left alone.
    let mut options = PlayStartOptions { arrange_2p: ArrangeOption::Mirror, ..random_options(10) };
    let applied = plan(&gate(KeyMode::K14), &mut options, &both);
    assert_eq!(applied.len(), 1);
    assert_eq!(options.arrange_seed_2p, Some(11));
}

#[test]
fn single_play_ignores_the_second_side_and_other_key_modes_work() {
    let mut options = random_options(10);
    let keys = [key("S1"), lane_key(Lane::Key12)];
    assert!(plan(&gate(KeyMode::K7), &mut options, &keys).is_empty());

    for (key_mode, lane) in
        [(KeyMode::K5, Lane::Key3), (KeyMode::K9, Lane::Key6), (KeyMode::K10, Lane::Key2)]
    {
        let mut options = random_options(10);
        let applied = plan(&gate(key_mode), &mut options, &[key("S1"), lane_key(lane)]);
        assert_eq!(applied.len(), 1, "{key_mode:?}");
        assert_eq!(bass_lane_after(&options, key_mode, OneBassSide::P1), lane.index());
    }
}

#[test]
fn the_gate_closes_for_every_case_the_spec_excludes() {
    let pressed = [key("S1"), lane_key(Lane::Key4)];
    let open = |gate: OneBassGate, options: PlayStartOptions| {
        let mut options = options;
        !plan(&gate, &mut options, &pressed).is_empty()
    };
    let base = gate(KeyMode::K7);
    assert!(open(base, random_options(5)));

    assert!(!open(OneBassGate { enabled: false, ..base }, random_options(5)), "setting OFF");
    assert!(!open(OneBassGate { plain_launch: false, ..base }, random_options(5)), "practice");
    assert!(!open(OneBassGate { in_course: true, ..base }, random_options(5)), "course");
    assert!(!open(OneBassGate { random_trainer: true, ..base }, random_options(5)), "trainer");
    assert!(!open(OneBassGate { cli_layout: true, ..base }, random_options(5)), "CLI seed");
    for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8] {
        assert!(!open(gate(key_mode), random_options(5)), "{key_mode:?}");
    }
    for session_mode in [
        SessionMode::Practice,
        SessionMode::Autoplay,
        SessionMode::AutoplayBattle,
        SessionMode::GBattle,
    ] {
        let options = PlayStartOptions { session_mode, ..random_options(5) };
        assert!(!open(base, options), "{session_mode:?}");
    }
    let autoplay = PlayStartOptions { autoplay: true, ..random_options(5) };
    assert!(!open(base, autoplay), "autoplay");
    let replay = PlayStartOptions { replay_player: Some(Default::default()), ..random_options(5) };
    assert!(!open(base, replay), "replay");
    let flip = PlayStartOptions { double_option: DoubleOption::Flip, ..random_options(5) };
    assert!(!open(base, flip), "FLIP");
    let pattern = PlayStartOptions { arrange_pattern: Some(vec![1, 2, 3]), ..random_options(5) };
    assert!(!open(base, pattern), "copied lane pattern");
    let trainer_seed = PlayStartOptions { random_trainer_seed: Some(3), ..random_options(5) };
    assert!(!open(base, trainer_seed), "trainer seed");
    let converted = PlayStartOptions {
        key_mode_conversion: crate::config::profile_config::KeyModeConversionConfig::SevenToNine,
        ..random_options(5)
    };
    assert!(!open(base, converted), "7K to 9K");
    let to_dp = PlayStartOptions {
        key_mode_conversion: crate::config::profile_config::KeyModeConversionConfig::SpToDp,
        ..random_options(5)
    };
    assert!(!open(base, to_dp), "SP to DP");
}

#[test]
fn a_gate_that_is_closed_never_reads_the_keys() {
    // `plan_one_bass` checks the gate first, so an excluded launch leaves the options as is.
    let mut options = PlayStartOptions { session_mode: SessionMode::GBattle, ..random_options(5) };
    let before = (options.arrange_seed, options.arrange_seed_2p);
    let pressed = [key("S1"), lane_key(Lane::Key4)];
    assert!(plan(&gate(KeyMode::K7), &mut options, &pressed).is_empty());
    assert_eq!((options.arrange_seed, options.arrange_seed_2p), before);
}

#[test]
fn the_decide_check_runs_once() {
    let mut checked = false;
    assert!(take_one_bass_check(&mut checked));
    assert!(!take_one_bass_check(&mut checked));
    assert!(!take_one_bass_check(&mut checked));
}

/// The key held into the play screen is already in the pressed set, so the play screen
/// sees its auto-repeat as a repeat and a repeat never changes HS or the lane cover.
#[test]
fn a_key_held_from_decide_is_a_repeat_on_the_play_screen() {
    use winit::event::ElementState;
    use winit::keyboard::{KeyCode, PhysicalKey};
    let mut runtime = AppInputRuntime::default();
    let held_key = PhysicalKey::Code(KeyCode::KeyD);
    runtime.track_control(&ControlInputEvent::keyboard_parts(
        held_key,
        ElementState::Pressed,
        false,
    ));
    assert!(runtime.keyboard_repeat(held_key, true));
    // Entering play clears the hold flags but not the pressed set.
    assert!(!runtime.pressed_play_inputs.is_empty());
    // A fresh press after a release is a new press again.
    runtime.track_control(&ControlInputEvent::keyboard_parts(
        held_key,
        ElementState::Released,
        false,
    ));
    assert!(!runtime.keyboard_repeat(held_key, true));
}

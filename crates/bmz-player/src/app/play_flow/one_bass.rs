//! One-bass input wiring. See `docs/bmsir/SPEC_ONE_BASS_INPUT.md`.
//!
//! The lane and seed calculation lives in `screens::play_session` (`one_bass_seed` and
//! friends). This file decides *when* it may act and reads the real key state once, at the
//! end of the decide screen: with a plain RANDOM, START (E1) held together with exactly one
//! key of a side pins the chart's leftmost key to that key by replacing the side's seed.
//! The play then loads again with the new seed; nothing one-bass specific is stored.
use super::*;
use crate::random_option_seed::RandomOptionSeed;
use crate::screens::play_session::{
    OneBassBase, OneBassSide, one_bass_seed, pressed_target, seed_permutation, supports_key_mode,
};
use std::collections::HashSet;

/// Facts from the app that decide whether one-bass may act for this launch.
#[derive(Debug, Clone, Copy)]
pub(super) struct OneBassGate {
    /// `play.one_bass_input`.
    pub(super) enabled: bool,
    /// Source key mode of the chart (before any key-count conversion).
    pub(super) key_mode: KeyMode,
    /// A normal play launch; Practice configuration is not one.
    pub(super) plain_launch: bool,
    /// A course / dan / RANDOM MIX is running.
    pub(super) in_course: bool,
    /// Random Trainer is enabled.
    pub(super) random_trainer: bool,
    /// The CLI fixed the seed, arrange or double option.
    pub(super) cli_layout: bool,
}

impl OneBassGate {
    /// Whether one-bass may look at the keys for a play started with `options`.
    pub(super) fn allows(&self, options: &PlayStartOptions) -> bool {
        self.enabled
            && self.plain_launch
            && !self.in_course
            && !self.random_trainer
            && !self.cli_layout
            && supports_key_mode(self.key_mode)
            && options.session_mode == SessionMode::Normal
            && !options.autoplay
            && options.replay_player.is_none()
            && options.battle_target.is_none()
            && !options.key_mode_conversion.applies_to(self.key_mode)
            && options.double_option == DoubleOption::Off
            && options.arrange_pattern.is_none()
            && options.random_trainer_seed.is_none()
    }
}

/// START is held: a binding of exactly E1. E4 is not counted here even though the play
/// screen treats it like E1.
pub(super) fn one_bass_start_held(
    pressed: &HashSet<(DeviceId, PhysicalControl)>,
    input: &PlayOptionInput,
) -> bool {
    pressed.iter().any(|(device, control)| {
        !input.resolves_lane(*device, control)
            && input.is_exact_action(*device, control, InputActionConfig::E1)
    })
}

/// Every lane (scratch included) the held controls are bound to.
pub(super) fn one_bass_pressed_lanes(
    pressed: &HashSet<(DeviceId, PhysicalControl)>,
    input: &PlayOptionInput,
) -> Vec<Lane> {
    pressed
        .iter()
        .filter_map(|(device, control)| input.resolve_entry(*device, control))
        .map(|resolved| resolved.lane)
        .collect()
}

/// A side whose seed one-bass chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OneBassApplied {
    pub(super) side: OneBassSide,
    pub(super) target: Lane,
    pub(super) seed: RandomOptionSeed,
    /// The side's seed before; `None` when it had none.
    pub(super) previous: Option<i64>,
}

impl OneBassApplied {
    /// The seed differs, so the chart must load again.
    pub(super) fn changed(&self) -> bool {
        self.previous != Some(i64::from(self.seed.value()))
    }
}

fn seed_from_option(value: Option<i64>) -> Option<RandomOptionSeed> {
    RandomOptionSeed::new(u32::try_from(value?).ok()?)
}

/// Replaces the seed of each side that is a plain RANDOM and has exactly one key held.
/// Sides that are not RANDOM, or have zero / several keys held, are left alone, as is
/// everything when no seed can be found (the ordinary RANDOM then stands).
pub(super) fn apply_one_bass(
    options: &mut PlayStartOptions,
    key_mode: KeyMode,
    pressed_lanes: &[Lane],
    entropy: &mut dyn FnMut() -> RandomOptionSeed,
) -> Vec<OneBassApplied> {
    let sides: &[OneBassSide] = if matches!(key_mode, KeyMode::K10 | KeyMode::K14) {
        &[OneBassSide::P1, OneBassSide::P2]
    } else {
        &[OneBassSide::P1]
    };
    let mut applied = Vec::new();
    for &side in sides {
        let (arrange, previous) = match side {
            OneBassSide::P1 => (options.arrange, options.arrange_seed),
            OneBassSide::P2 => (options.arrange_2p, options.arrange_seed_2p),
        };
        if arrange != ArrangeOption::Random {
            continue;
        }
        let Some(target) = pressed_target(key_mode, side, pressed_lanes.iter().copied()) else {
            continue;
        };
        let current = seed_from_option(previous);
        let borrowed_permutation = current
            .filter(|_| options.arrange_seed_borrowed)
            .and_then(|seed| seed_permutation(key_mode, side, seed));
        let base = match &borrowed_permutation {
            Some(permutation) => OneBassBase::Borrowed { permutation },
            None => OneBassBase::Own { current },
        };
        let Some(seed) = one_bass_seed(key_mode, side, target, base, entropy) else {
            tracing::info!(?side, ?target, "one-bass found no seed; keeping the RANDOM seed");
            continue;
        };
        applied.push(OneBassApplied { side, target, seed, previous });
    }
    // Write only after every side is decided so a later failure leaves nothing half done.
    for item in &applied {
        let value = Some(i64::from(item.seed.value()));
        match item.side {
            OneBassSide::P1 => options.arrange_seed = value,
            OneBassSide::P2 => options.arrange_seed_2p = value,
        }
        if item.changed() {
            options.legacy_arrange_seed = false;
        }
    }
    applied
}

/// The full decision from the held keys: `None` when one-bass does not act at all.
/// `options` is updated in place; the returned list says which sides were pinned.
pub(super) fn plan_one_bass(
    gate: &OneBassGate,
    options: &mut PlayStartOptions,
    pressed: &HashSet<(DeviceId, PhysicalControl)>,
    input: &PlayOptionInput,
    entropy: &mut dyn FnMut() -> RandomOptionSeed,
) -> Vec<OneBassApplied> {
    if !gate.allows(options) || !one_bass_start_held(pressed, input) {
        return Vec::new();
    }
    let lanes = one_bass_pressed_lanes(pressed, input);
    apply_one_bass(options, gate.key_mode, &lanes, entropy)
}

/// Whether to read the input now. Called each frame at the end of the decide screen;
/// true only the first time, so the keys are read exactly once per decide.
pub(super) fn take_one_bass_check(checked: &mut bool) -> bool {
    !std::mem::replace(checked, true)
}

/// A reload is needed only if some side's seed actually changed.
pub(super) fn one_bass_needs_reload(applied: &[OneBassApplied]) -> bool {
    applied.iter().any(OneBassApplied::changed)
}

impl WinitApp {
    pub(super) fn one_bass_gate(&self, chart_id: i64, plain_launch: bool) -> OneBassGate {
        OneBassGate {
            enabled: self.boot.profile_config.play.one_bass_input,
            key_mode: self.key_mode_for_chart(chart_id),
            plain_launch,
            in_course: self.play.active_course.is_some(),
            random_trainer: self.select.random_trainer.is_enabled(),
            cli_layout: self.cli_chart_layout_override_active(),
        }
    }

    /// The input layout the play screen will use, built before the play scene exists.
    fn one_bass_play_input(&self, key_mode: KeyMode) -> PlayOptionInput {
        let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids(
            resolve_gamepad_runtime_slots(&self.boot.app_config.input, self.gamepad.as_ref()),
        );
        let binding = crate::config::play::lane_binding_for_chart_with_slots(
            &self.boot.profile_config.input,
            key_mode,
            slots,
        );
        PlayOptionInput::new(key_mode, binding, &self.boot.profile_config.input, slots)
    }

    /// Reads the held keys once, when the decide exit has finished, and pins the bass.
    /// Returns true when the seed changed and the chart load was restarted; the caller then
    /// waits for the new load as it would for any unfinished Play skin.
    pub(super) fn apply_one_bass_at_decide_end(&mut self) -> bool {
        let Some(decide) = self.play.pending_decide.as_mut() else {
            return false;
        };
        if !take_one_bass_check(&mut decide.one_bass_checked) || decide.cancel {
            return false;
        }
        let chart_id = decide.chart_id;
        let plain_launch = matches!(decide.launch, DecideLaunch::Play);
        let mut options = decide.options.clone();
        // OFF (and any unsupported launch) returns before the input is touched.
        let gate = self.one_bass_gate(chart_id, plain_launch);
        if !gate.enabled || !gate.allows(&options) {
            return false;
        }
        let input = self.one_bass_play_input(gate.key_mode);
        let mut entropy = || crate::random_option_seed::RandomOptionSeeds::fresh(false).p1;
        let applied = plan_one_bass(
            &gate,
            &mut options,
            &self.input.pressed_play_inputs,
            &input,
            &mut entropy,
        );
        for item in &applied {
            tracing::info!(
                side = ?item.side,
                lane = ?item.target,
                seed = item.seed.value(),
                changed = item.changed(),
                "one-bass applied"
            );
        }
        if !one_bass_needs_reload(&applied) {
            return false;
        }
        if let Some(decide) = self.play.pending_decide.as_mut() {
            decide.options = options.clone();
        }
        // The seed is part of the load: drop the old result and start again.
        self.invalidate_play_preload();
        self.start_play_preload(chart_id, options.clone());
        self.respawn_play_skin_decode(chart_id, &options);
        true
    }
}

#[cfg(test)]
#[path = "one_bass_tests.rs"]
mod tests;

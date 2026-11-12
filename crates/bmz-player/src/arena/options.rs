//! Arena play options: the `play_option` number and the one-song overrides
//! (spec section 4, "Arena の曲で当てはめるもの").
//!
//! Everything here is a pure function of the server's values. The overrides are
//! written into the [`PlayStartOptions`] of the one Arena song only; the
//! profile and the select-screen options are never modified, so there is
//! nothing to put back when the song ends.

use bmz_gameplay::rule::RuleMode;

use crate::config::profile_config::AssistOptionConfig;
use crate::ln_policy::LnPolicySetting;
use crate::screens::play_start::PlayStartOptions;
use crate::select_options::{ArrangeOption, DoubleOption, SessionMode};

use super::protocol::RulesetProfile;

/// `play_option = 1P + 10 * 2P + 100 * FLIP`. 0..=5 are NORMAL, MIRROR, RANDOM,
/// R-RANDOM, S-RANDOM, SPIRAL; every other BMZ arrange is sent as NORMAL.
fn arrange_code(option: ArrangeOption) -> u32 {
    match option {
        ArrangeOption::Normal => 0,
        ArrangeOption::Mirror => 1,
        ArrangeOption::Random => 2,
        ArrangeOption::RRandom => 3,
        ArrangeOption::SRandom => 4,
        ArrangeOption::Spiral => 5,
        // H-RANDOM, ALL-SCR, RANDOM-EX, S-RANDOM-EX, F-RANDOM, MF-RANDOM
        _ => 0,
    }
}

fn arrange_from_code(code: u32) -> ArrangeOption {
    match code {
        1 => ArrangeOption::Mirror,
        2 => ArrangeOption::Random,
        3 => ArrangeOption::RRandom,
        4 => ArrangeOption::SRandom,
        5 => ArrangeOption::Spiral,
        _ => ArrangeOption::Normal,
    }
}

/// Doubles (10KEY / 14KEY / 50) carry a 2P digit and FLIP; singles send the 1P digit only.
pub fn is_double_play_mode(play_mode: u32) -> bool {
    matches!(play_mode, 10 | 14 | 50)
}

/// The `option_ready.play_option` for the player's current select-screen options.
/// BATTLE and BATTLE AS are not Arena options and count as Off.
pub fn encode_play_option(
    arrange: ArrangeOption,
    arrange_2p: ArrangeOption,
    double_option: DoubleOption,
    play_mode: u32,
) -> u32 {
    let mut value = arrange_code(arrange);
    if is_double_play_mode(play_mode) {
        value += 10 * arrange_code(arrange_2p);
        if double_option == DoubleOption::Flip {
            value += 100;
        }
    }
    value
}

/// The arrange options a confirmed `play_option` stands for: `(1P, 2P, FLIP)`.
/// Out-of-range digits fall back to NORMAL.
pub fn decode_play_option(value: u32) -> (ArrangeOption, ArrangeOption, bool) {
    (arrange_from_code(value % 10), arrange_from_code(value / 10 % 10), value / 100 % 10 == 1)
}

/// `lr2` plays under `Lr2Oraja`, `oraja` under `Beatoraja`. `Dx` is never used.
pub fn rule_mode_for_ruleset(ruleset: RulesetProfile) -> RuleMode {
    match ruleset {
        RulesetProfile::Lr2 => RuleMode::Lr2Oraja,
        RulesetProfile::Oraja => RuleMode::Beatoraja,
    }
}

pub fn ruleset_from_text(text: &str) -> Option<RulesetProfile> {
    match text {
        "lr2" => Some(RulesetProfile::Lr2),
        "oraja" => Some(RulesetProfile::Oraja),
        _ => None,
    }
}

/// The foundation plays long notes as LN only; CN and HCN come with step 5.
pub fn ln_policy_for_ln_mode(ln_mode: &str) -> Option<LnPolicySetting> {
    (ln_mode == "LN").then_some(LnPolicySetting::ForceLn)
}

/// The policy that decides how many notes a room's `ln_mode` counts (operator
/// decision 2): LN counts a long note once, CN and HCN count start and end (2).
/// Unlike [`ln_policy_for_ln_mode`] this also knows CN and HCN, which are only
/// counted here (such rooms are still not played).
pub fn counting_policy_for_ln_mode(ln_mode: &str) -> Option<LnPolicySetting> {
    match ln_mode {
        "LN" => Some(LnPolicySetting::ForceLn),
        "CN" => Some(LnPolicySetting::ForceCn),
        "HCN" => Some(LnPolicySetting::ForceHcn),
        _ => None,
    }
}

/// `totalnotes` for `play_ready` and `chart_check`: the chart's score-target
/// note count in the room's LN mode (`BMSModel.getTotalNotes()` of the server's
/// reference client).
pub fn arena_total_notes(chart: &bmz_chart::model::PlayableChart, ln_mode: &str) -> Option<u32> {
    counting_policy_for_ln_mode(ln_mode)
        .map(|policy| crate::ln_policy::expected_scored_note_count(chart, policy))
}

/// What the Arena song replaces for the one launch: rule and long-note policy
/// (used in place of the profile's wherever the score identity is built).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaRules {
    pub rule_mode: RuleMode,
    pub ln_policy: LnPolicySetting,
}

/// Writes the Arena overrides into `options`. Start from the normal
/// [`PlayStartOptions`] of the profile, so gauge, HS, lane cover and display
/// options stay the player's own.
///
/// * NORMAL session, no autoplay, no replay, no G-BATTLE target;
/// * assists, key-mode conversion, Random Trainer and LN changes off;
/// * arrange and FLIP from the confirmed `play_option`;
/// * the server's `random_seed` for a side whose arrange is RANDOM.
pub fn apply_arena_overrides(options: &mut PlayStartOptions, play_option: u32, random_seed: i64) {
    let (arrange, arrange_2p, flip) = decode_play_option(play_option);
    options.session_mode = SessionMode::Normal;
    options.autoplay = false;
    options.score_save_disabled = false;
    options.assist = AssistOptionConfig::default();
    options.key_mode_conversion = Default::default();
    options.replay_player = None;
    options.battle_target = None;
    options.resolved_target = None;
    options.random_trainer_seed = None;
    options.ln_mode_override = None;
    options.arrange_pattern = None;
    options.legacy_arrange_seed = false;
    options.arrange = arrange;
    options.arrange_2p = arrange_2p;
    options.double_option = if flip { DoubleOption::Flip } else { DoubleOption::Off };
    if arrange == ArrangeOption::Random {
        options.arrange_seed = Some(random_seed);
    }
    if arrange_2p == ArrangeOption::Random {
        options.arrange_seed_2p = Some(random_seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_play_sends_only_the_1p_digit() {
        for (arrange, code) in [
            (ArrangeOption::Normal, 0),
            (ArrangeOption::Mirror, 1),
            (ArrangeOption::Random, 2),
            (ArrangeOption::RRandom, 3),
            (ArrangeOption::SRandom, 4),
            (ArrangeOption::Spiral, 5),
        ] {
            // 2P arrange and FLIP never leak into an SP value.
            assert_eq!(
                encode_play_option(arrange, ArrangeOption::Mirror, DoubleOption::Flip, 7),
                code
            );
        }
    }

    #[test]
    fn double_play_adds_the_2p_digit_and_flip() {
        assert_eq!(
            encode_play_option(
                ArrangeOption::Mirror,
                ArrangeOption::Random,
                DoubleOption::Flip,
                14
            ),
            121
        );
        assert_eq!(
            encode_play_option(ArrangeOption::Normal, ArrangeOption::Spiral, DoubleOption::Off, 10),
            50
        );
        // BATTLE is not an Arena option.
        assert_eq!(
            encode_play_option(
                ArrangeOption::Normal,
                ArrangeOption::Normal,
                DoubleOption::Battle,
                14
            ),
            0
        );
    }

    #[test]
    fn unsupported_arranges_become_normal() {
        for arrange in [
            ArrangeOption::HRandom,
            ArrangeOption::AllScratch,
            ArrangeOption::RandomEx,
            ArrangeOption::SRandomEx,
            ArrangeOption::FRandom,
            ArrangeOption::MFRandom,
        ] {
            assert_eq!(encode_play_option(arrange, arrange, DoubleOption::Off, 14), 0);
        }
    }

    #[test]
    fn decode_round_trips_and_clamps() {
        assert_eq!(decode_play_option(121), (ArrangeOption::Mirror, ArrangeOption::Random, true));
        assert_eq!(decode_play_option(4), (ArrangeOption::SRandom, ArrangeOption::Normal, false));
        // Digits above 5 and a FLIP digit other than 1 are not Arena values.
        assert_eq!(decode_play_option(97), (ArrangeOption::Normal, ArrangeOption::Normal, false));
    }

    #[test]
    fn rulesets_and_ln_modes_map_to_bmz_rules() {
        assert_eq!(rule_mode_for_ruleset(RulesetProfile::Lr2), RuleMode::Lr2Oraja);
        assert_eq!(rule_mode_for_ruleset(RulesetProfile::Oraja), RuleMode::Beatoraja);
        assert_eq!(ruleset_from_text("lr2"), Some(RulesetProfile::Lr2));
        assert_eq!(ruleset_from_text("dx"), None);
        assert_eq!(ln_policy_for_ln_mode("LN"), Some(LnPolicySetting::ForceLn));
        assert_eq!(ln_policy_for_ln_mode("CN"), None);
        assert_eq!(ln_policy_for_ln_mode("HCN"), None);
        assert_eq!(counting_policy_for_ln_mode("CN"), Some(LnPolicySetting::ForceCn));
        assert_eq!(counting_policy_for_ln_mode("HCN"), Some(LnPolicySetting::ForceHcn));
        assert_eq!(counting_policy_for_ln_mode("x"), None);
    }

    #[test]
    fn overrides_make_a_plain_normal_play_with_the_servers_seed() {
        let mut options = PlayStartOptions {
            session_mode: SessionMode::GBattle,
            autoplay: true,
            score_save_disabled: true,
            arrange: ArrangeOption::HRandom,
            double_option: DoubleOption::Battle,
            arrange_seed: Some(11),
            arrange_seed_2p: Some(22),
            random_trainer_seed: Some(5),
            ..PlayStartOptions::default()
        };
        options.assist.mark_note = true;
        apply_arena_overrides(&mut options, 2, 123_456);
        assert_eq!(options.session_mode, SessionMode::Normal);
        assert!(!options.autoplay && !options.score_save_disabled);
        assert_eq!(options.assist, AssistOptionConfig::default());
        assert_eq!(options.arrange, ArrangeOption::Random);
        assert_eq!(options.double_option, DoubleOption::Off);
        assert_eq!(options.arrange_seed, Some(123_456));
        // The 2P side is not RANDOM, so its own seed stays.
        assert_eq!(options.arrange_seed_2p, Some(22));
        assert_eq!(options.random_trainer_seed, None);
    }

    #[test]
    fn the_servers_seed_is_not_used_for_non_random_sides() {
        let mut options =
            PlayStartOptions { arrange_seed: Some(11), ..PlayStartOptions::default() };
        apply_arena_overrides(&mut options, 3, 999);
        assert_eq!(options.arrange, ArrangeOption::RRandom);
        assert_eq!(options.arrange_seed, Some(11));
    }

    #[test]
    fn flip_maps_to_double_option_flip() {
        let mut options = PlayStartOptions::default();
        apply_arena_overrides(&mut options, 100, 1);
        assert_eq!(options.double_option, DoubleOption::Flip);
    }
}

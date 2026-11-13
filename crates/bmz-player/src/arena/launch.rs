//! Decisions for starting an Arena song, free of the app: which rules a
//! `prepare` asks for, and when the server's `start_at` has come.

use super::options::{ArenaRules, ln_policy_for_ln_mode, rule_mode_for_ruleset, ruleset_from_text};
use super::protocol::Prepare;

/// Why a `prepare` cannot be played by this build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRefusal {
    /// CN and HCN rooms come with step 5.
    UnsupportedLnMode(String),
    /// The match `rules` carry no `lr2` / `oraja` ruleset.
    UnknownRuleset(String),
    /// Not a 7KEY match (the foundation is 7KEY only).
    UnsupportedPlayMode(Option<u32>),
}

/// The rule and long-note policy the song is played under.
pub fn rules_for_prepare(prepare: &Prepare) -> Result<ArenaRules, LaunchRefusal> {
    let Some(ln_policy) = ln_policy_for_ln_mode(&prepare.ln_mode) else {
        return Err(LaunchRefusal::UnsupportedLnMode(prepare.ln_mode.clone()));
    };
    let Some(ruleset) = ruleset_from_text(&prepare.ruleset_profile) else {
        return Err(LaunchRefusal::UnknownRuleset(prepare.ruleset_profile.clone()));
    };
    // `play_mode` is null when the players' modes differ; the foundation allows only 7.
    if prepare.play_mode.is_some_and(|mode| mode != 7) {
        return Err(LaunchRefusal::UnsupportedPlayMode(prepare.play_mode));
    }
    Ok(ArenaRules { rule_mode: rule_mode_for_ruleset(ruleset), ln_policy })
}

/// Whether READY may give way to the song (operator decision 1).
///
/// The song's second 0 is `start_at`, as in Arena oraja where PLAY begins at
/// `start_at` and READY (`playstart`) is before it. BMZ starts its audio clock
/// `playstart_lead_us` before the chart's 0 (the skin's `playstart` pre-roll),
/// so the clock starts `playstart_lead_us` before `start_at`. All of these
/// must hold:
///
/// * `play_ready` has been sent (`loaded_reported`);
/// * the READY presentation is over (the caller only asks after it);
/// * the server clock (local UNIX time plus the synchronised offset) has
///   reached `start_at − playstart_lead_us`.
///
/// Without a clock offset nothing is started: the song would begin at an
/// unknown moment.
pub fn may_leave_ready(
    start_at: f64,
    playstart_lead_us: i64,
    clock_offset_secs: Option<f64>,
    local_unix: f64,
    loaded_reported: bool,
) -> bool {
    let lead_secs = playstart_lead_us.max(0) as f64 / 1_000_000.0;
    loaded_reported
        && clock_offset_secs.is_some_and(|offset| local_unix + offset >= start_at - lead_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::protocol::Prepare;
    use bmz_gameplay::rule::RuleMode;

    fn prepare(ln_mode: &str, ruleset: &str, play_mode: Option<u32>) -> Prepare {
        Prepare {
            match_id: "m".into(),
            ln_mode: ln_mode.into(),
            ruleset_profile: ruleset.into(),
            play_mode,
            ..Prepare::default()
        }
    }

    #[test]
    fn ln_rooms_play_under_the_rooms_ruleset() {
        let rules = rules_for_prepare(&prepare("LN", "oraja", Some(7))).unwrap();
        assert_eq!(rules.rule_mode, RuleMode::Beatoraja);
        assert_eq!(rules.ln_policy, crate::ln_policy::LnPolicySetting::ForceLn);
        assert_eq!(
            rules_for_prepare(&prepare("LN", "lr2", None)).unwrap().rule_mode,
            RuleMode::Lr2Oraja
        );
    }

    #[test]
    fn cn_hcn_unknown_rulesets_and_other_key_counts_are_refused() {
        assert_eq!(
            rules_for_prepare(&prepare("CN", "lr2", Some(7))),
            Err(LaunchRefusal::UnsupportedLnMode("CN".into()))
        );
        assert_eq!(
            rules_for_prepare(&prepare("HCN", "lr2", Some(7))),
            Err(LaunchRefusal::UnsupportedLnMode("HCN".into()))
        );
        assert_eq!(
            rules_for_prepare(&prepare("LN", "", Some(7))),
            Err(LaunchRefusal::UnknownRuleset(String::new()))
        );
        assert_eq!(
            rules_for_prepare(&prepare("LN", "lr2", Some(14))),
            Err(LaunchRefusal::UnsupportedPlayMode(Some(14)))
        );
    }

    #[test]
    fn chart_zero_is_start_at_so_the_clock_starts_playstart_earlier() {
        // Server clock is 2 s ahead of the local one; playstart pre-roll is 1.5 s.
        let lead = 1_500_000;
        // Chart 0 = start_at 1010 -> the clock may start at 1008.5 server time (1006.5 local).
        assert!(!may_leave_ready(1_010.0, lead, Some(2.0), 1_006.4, true));
        assert!(may_leave_ready(1_010.0, lead, Some(2.0), 1_006.5, true));
        // No pre-roll: the clock starts exactly at start_at.
        assert!(!may_leave_ready(1_010.0, 0, Some(2.0), 1_007.9, true));
        assert!(may_leave_ready(1_010.0, 0, Some(2.0), 1_008.0, true));
        assert!(may_leave_ready(1_010.0, 0, Some(-1.0), 1_011.5, true));
    }

    #[test]
    fn ready_is_held_until_play_ready_was_sent_and_the_clock_is_known() {
        // The time has come but `play_ready` has not been sent: stay in READY.
        assert!(!may_leave_ready(1_010.0, 0, Some(0.0), 9_999.0, false));
        // No synchronised clock: never start.
        assert!(!may_leave_ready(1_010.0, 0, None, 9_999.0, true));
        // A negative lead (no skin pre-roll) is treated as none.
        assert!(!may_leave_ready(1_010.0, -5, Some(0.0), 1_009.9, true));
    }
}

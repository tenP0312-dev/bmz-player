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

/// `true` once the server clock (local UNIX time plus the synchronised offset)
/// reaches `start_at`. Without a clock offset nothing is started: the song
/// would begin at an unknown moment.
pub fn start_reached(start_at: f64, clock_offset_secs: Option<f64>, local_unix: f64) -> bool {
    clock_offset_secs.is_some_and(|offset| local_unix + offset >= start_at)
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
    fn the_song_starts_when_server_time_reaches_start_at() {
        // Server clock is 2 s ahead of the local one.
        assert!(!start_reached(1_010.0, Some(2.0), 1_007.9));
        assert!(start_reached(1_010.0, Some(2.0), 1_008.0));
        assert!(start_reached(1_010.0, Some(-1.0), 1_011.5));
        // No synchronised clock: never start.
        assert!(!start_reached(1_010.0, None, 9_999.0));
    }
}

//! BMS-IR Arena: starting the match's own song (stage B2-2).
//!
//! The Arena session (`crate::arena`) only follows the server. This file is the
//! one place where its decisions reach the play flow:
//!
//! * the select-screen options are handed to Arena as a `play_option`;
//! * a `prepare` starts the chart through `start_chart_with_options` (the same
//!   entry as retry and direct boot, so the READY presentation, preload and
//!   audio start are unchanged) with the Arena overrides in its
//!   `PlayStartOptions` and the Arena rule in `arena_play`;
//! * the finished load is reported (`play_ready`) and the audio start is held
//!   at the end of READY until the server clock reaches `start_at`.
//!
//! Nothing here touches the profile, so ending or cancelling the match needs
//! no restore step: `arena_play` is dropped and the next play is the player's own.

use super::*;

/// The Arena song being started or played.
#[derive(Debug, Clone)]
pub(super) struct ArenaPlayLaunch {
    pub(super) match_id: String,
    pub(super) md5: String,
    pub(super) rules: crate::arena::options::ArenaRules,
    /// `play_ready` has been reported for this launch.
    pub(super) loaded_reported: bool,
    /// The audio clock started (the song is playing).
    pub(super) started: bool,
}

/// What `maybe_start_ready_phase` does at the end of READY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArenaStartGate {
    /// Not an Arena song, or `start_at` has come.
    Go,
    /// Stay in READY.
    Hold,
}

impl WinitApp {
    /// The long-note policy and rule that decide this play's score identity:
    /// the course's, else the Arena match's, else the profile's.
    pub(super) fn effective_ln_policy_and_rule(
        &self,
    ) -> (crate::ln_policy::LnPolicySetting, bmz_gameplay::rule::RuleMode) {
        if let Some(course) = &self.play.active_course {
            return (course.ln_policy_setting, course.rule_mode);
        }
        if let Some(arena) = &self.play.arena_play {
            return (arena.rules.ln_policy, arena.rules.rule_mode);
        }
        (self.boot.profile_config.play.ln_mode_policy, self.boot.profile_config.play.rule_mode)
    }

    /// Per-frame Arena work for the play flow. Reads the Arena panel state and
    /// acts on it; the connection thread never calls into the app.
    pub(super) fn sync_arena_play(&mut self, scene_kind: AppSceneKind) {
        let handle = self.integrations.arena.handle();
        let panel = handle.panel();
        // The Arena song was played: the next play is the player's own again.
        if self.play.arena_play.as_ref().is_some_and(|play| play.started)
            && scene_kind != AppSceneKind::Play
        {
            self.play.arena_play = None;
        }
        let Some(current) = panel.current_match.as_ref() else {
            // The match is over (cancelled, released, disconnected).
            self.play.arena_play = None;
            self.integrations.arena_option_choice = None;
            return;
        };
        if self.play.arena_play.as_ref().is_some_and(|play| play.match_id != current.match_id) {
            self.play.arena_play = None;
        }
        if let Some(option) = &current.option
            && current.stage == crate::arena::session::MatchStage::Options
        {
            let play_mode = option.play_mode.unwrap_or(7);
            let choice = crate::arena::options::encode_play_option(
                self.select.arrange_option,
                self.select.arrange_option_2p,
                self.select.double_option,
                play_mode,
            );
            if self.integrations.arena_option_choice != Some(choice) {
                self.integrations.arena_option_choice = Some(choice);
                handle.set_option_choice(choice);
            }
        }
        if let Some(prepare) = panel.pending_prepare.clone()
            && self.arena_can_launch(scene_kind)
        {
            self.launch_arena_play(&handle, prepare);
        }
    }

    /// While an Arena match is reserved, ordinary songs cannot be started
    /// (`block_next_selection`, spec 3.4 step 2). The match's own song is
    /// started by [`Self::launch_arena_play`], which does not pass through here.
    pub(super) fn arena_blocks_normal_start(&mut self) -> bool {
        let handle = self.integrations.arena.handle();
        let panel = handle.panel();
        if panel.current_match.is_none() || self.play.arena_play.is_some() {
            return false;
        }
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        self.show_left_overlay_toast(
            text.format("toast-arena-match-blocks-play", &FluentArgs::new()),
        );
        true
    }

    /// A song cannot be started from a play, the decide screen or an unfinished start.
    fn arena_can_launch(&self, scene_kind: AppSceneKind) -> bool {
        matches!(scene_kind, AppSceneKind::Select | AppSceneKind::Result)
            && self.play.active_play.is_none()
            && self.play.pending_play_start.is_none()
            && self.play.pending_decide.is_none()
    }

    fn launch_arena_play(
        &mut self,
        handle: &crate::arena::connection::ArenaHandle,
        prepare: crate::arena::protocol::Prepare,
    ) {
        let match_id = prepare.match_id.clone();
        handle.prepare_taken(match_id.clone());
        let refuse = |app: &mut Self, reason: &str| {
            tracing::warn!(reason, "arena: cannot start the match chart");
            handle.load_failed(match_id.clone());
            app.play.arena_play = None;
        };
        let rules = match crate::arena::launch::rules_for_prepare(&prepare) {
            Ok(rules) => rules,
            Err(refusal) => return refuse(self, &format!("{refusal:?}")),
        };
        let Some(chart_id) = self.arena_chart_id(&prepare.md5) else {
            return refuse(self, "chart not in the library");
        };
        self.play.arena_play = Some(ArenaPlayLaunch {
            match_id,
            md5: prepare.md5.clone(),
            rules,
            loaded_reported: false,
            started: false,
        });
        let mut options = self.play_start_options();
        crate::arena::options::apply_arena_overrides(
            &mut options,
            prepare.play_option,
            prepare.random_seed.unwrap_or(0),
        );
        tracing::info!(
            chart_id,
            play_option = prepare.play_option,
            "arena: starting the match chart"
        );
        self.start_chart_with_options(chart_id, options);
    }

    /// The library row of a 7KEY chart with this MD5.
    fn arena_chart_id(&self, md5_hex: &str) -> Option<i64> {
        let md5 = crate::storage::common::hex_to_hash::<16>(md5_hex).ok()?;
        let sha256 = self.boot.library_db.chart_sha256_by_md5(md5).ok().flatten()?;
        self.boot
            .library_db
            .list_charts_by_sha256(sha256)
            .ok()?
            .into_iter()
            .find(|chart| {
                crate::arena::chart_lookup::play_mode_for_key_mode(&chart.mode) == Some(7)
            })
            .map(|chart| chart.chart_id)
    }

    /// Reports the finished load (`play_ready`) once. `totalnotes` is what the
    /// loaded session scores.
    pub(super) fn arena_report_loaded(&mut self) {
        let Some(arena_play) = self.play.arena_play.as_mut() else {
            return;
        };
        let totalnotes = self
            .play
            .active_play
            .as_ref()
            .and_then(|play| play.running.gameplay.prepared_session())
            .map(|session| session.scored_total_notes);
        if !arena_play.loaded_reported
            && let Some(totalnotes) = totalnotes
        {
            arena_play.loaded_reported = true;
            self.integrations.arena.handle().play_loaded(
                arena_play.match_id.clone(),
                arena_play.md5.clone(),
                totalnotes,
            );
        }
    }

    /// Whether READY may end: always for a normal play, and for an Arena song
    /// only once the server clock has reached `start_at`.
    pub(super) fn arena_start_gate(&mut self) -> ArenaStartGate {
        let Some(arena_play) = self.play.arena_play.as_mut() else {
            return ArenaStartGate::Go;
        };
        let handle = self.integrations.arena.handle();
        let panel = handle.panel();
        let gate = panel.start_gate.filter(|gate| gate.match_id == arena_play.match_id);
        let local_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        match gate {
            Some(gate)
                if crate::arena::launch::start_reached(
                    gate.start_at,
                    panel.clock_offset_secs,
                    local_unix,
                ) =>
            {
                arena_play.started = true;
                ArenaStartGate::Go
            }
            _ => ArenaStartGate::Hold,
        }
    }

    /// The unstarted Arena chart was abandoned (load error, back key during READY).
    pub(super) fn arena_play_abandoned(&mut self) {
        if let Some(arena_play) = self.play.arena_play.take()
            && !arena_play.started
        {
            self.integrations.arena.handle().load_failed(arena_play.match_id);
        }
    }
}

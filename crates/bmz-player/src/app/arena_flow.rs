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
    /// The room's `ln_mode`, which decides how `totalnotes` counts long notes.
    pub(super) ln_mode: String,
    /// `play_ready` has been reported for this launch.
    pub(super) loaded_reported: bool,
    /// The audio clock started (the song is playing).
    pub(super) started: bool,
    /// `final` was produced (or the play could not report it).
    pub(super) final_reported: bool,
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
        // `ready` on the result screen waits for the normal IR submission.
        let settled = (scene_kind == AppSceneKind::Result).then(|| self.result_ir_settled());
        if self.integrations.arena_result_settled != settled {
            self.integrations.arena_result_settled = settled;
            handle.set_result_settled(settled.unwrap_or(false));
        }
        // The Arena song was played: the next play is the player's own again.
        if self.play.arena_play.as_ref().is_some_and(|play| play.started)
            && scene_kind != AppSceneKind::Play
        {
            if self.play.arena_play.as_ref().is_some_and(|play| !play.final_reported) {
                tracing::warn!("arena: the play left the Play scene without `final`");
            }
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
        if scene_kind == AppSceneKind::Play {
            self.report_arena_progress(&handle);
        }
        if let Some(prepare) = panel.pending_prepare.clone() {
            // A repeated `prepare` (after a reconnect) for the song already being
            // loaded or played is not started again.
            if self.play.arena_play.as_ref().is_some_and(|play| play.match_id == prepare.match_id) {
                handle.prepare_taken(prepare.match_id);
            } else if self.arena_can_launch(scene_kind) {
                self.launch_arena_play(&handle, prepare);
            }
        }
    }

    /// `live` while the match song plays and `final` once it has ended.
    fn report_arena_progress(&mut self, handle: &crate::arena::connection::ArenaHandle) {
        let Some(arena_play) = self.play.arena_play.as_ref() else {
            return;
        };
        if !arena_play.started || arena_play.final_reported {
            return;
        }
        let match_id = arena_play.match_id.clone();
        let Some(ending) = &self.play.play_ending else {
            // Playing: the latest values of the play, from the render snapshot.
            if let Some(snapshot) = &self.play.last_play_snapshot {
                handle.live_sample(match_id, snapshot_play_values(snapshot));
            }
            return;
        };
        if let Some(finished) = &ending.finished {
            let score = &finished.result.score;
            let values = crate::arena::session::PlayValues {
                exscore: score.ex_score(),
                processed_notes: score.past_notes,
                minbp: score.bp(),
                combo_break: score.cb(),
                max_combo: score.max_combo,
            };
            handle.play_finished(match_id, values, ending.failed, finished.result.clear_type as u8);
        } else if self
            .play
            .active_play
            .as_ref()
            .is_some_and(|play| play.running.finish_error.is_some())
        {
            // The result could not be built, so nothing true can be said: give up the match.
            tracing::error!("arena: the play result failed; forfeiting the match");
            handle.withdraw();
        } else {
            // The result is still being saved; `final` follows as soon as it is built.
            return;
        }
        if let Some(arena_play) = self.play.arena_play.as_mut() {
            arena_play.final_reported = true;
        }
    }

    /// Ending a match song early is not possible (spec 2, "ESC と START+SELECT
    /// でプレイを抜けられません"). READY is still free to leave: that forfeits.
    pub(super) fn arena_locks_play_exit(&mut self) -> bool {
        if !self.play.arena_play.as_ref().is_some_and(|play| play.started) {
            return false;
        }
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        self.show_left_overlay_toast(text.format("toast-arena-play-locked", &FluentArgs::new()));
        true
    }

    /// Whether the normal IR submission of the result screen has finished
    /// (spec 3.3: `ready` on `result`).
    pub(super) fn result_ir_settled(&self) -> bool {
        self.result.result_ir.as_ref().is_none_or(|state| {
            matches!(state.submit, crate::screens::result_ir::IrSubmitState::Done { .. })
        })
    }

    /// The Arena `state` for the current scene.
    pub(super) fn arena_screen(&self, scene_kind: AppSceneKind) -> crate::arena::ScreenState {
        let running = self.play.active_play.as_ref().map(|play| &play.running);
        arena_screen_for(
            scene_kind,
            ArenaPlayKind {
                course: self.play.active_course.is_some(),
                practice: self.play.practice_session.is_some()
                    || running.is_some_and(|running| running.practice_mode),
                autoplay: running.is_some_and(|running| running.session.autoplay.is_some()),
                replay: running.is_some_and(|running| running.session.replay_player.is_some()),
            },
        )
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
            ln_mode: prepare.ln_mode.clone(),
            loaded_reported: false,
            started: false,
            final_reported: false,
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
            // LN counts once, CN / HCN twice (operator decision 2), from the loaded chart.
            .and_then(|session| {
                crate::arena::options::arena_total_notes(&session.chart, &arena_play.ln_mode)
            });
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
    /// only once `play_ready` was sent and the chart's 0 (`start_at`) is
    /// `playstart_lead_us` away, the skin's pre-roll after the clock starts.
    pub(super) fn arena_start_gate(&mut self, playstart_lead_us: i64) -> ArenaStartGate {
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
                if crate::arena::launch::may_leave_ready(
                    gate.start_at,
                    playstart_lead_us,
                    panel.clock_offset_secs,
                    local_unix,
                    arena_play.loaded_reported,
                ) =>
            {
                if !arena_play.started {
                    arena_play.started = true;
                    handle.play_started(arena_play.match_id.clone());
                }
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

/// What kind of play the app is in, for the Arena `state`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ArenaPlayKind {
    pub(super) course: bool,
    pub(super) practice: bool,
    pub(super) autoplay: bool,
    pub(super) replay: bool,
}

/// Arena `state` for a scene (spec 3.3). `play` is the ordinary manual play
/// (including G-BATTLE); the others are told apart so the server never counts
/// them as the match song. A course also owns its result screens.
pub(super) fn arena_screen_for(
    scene_kind: AppSceneKind,
    kind: ArenaPlayKind,
) -> crate::arena::ScreenState {
    use crate::arena::ScreenState;
    match scene_kind {
        AppSceneKind::Select => ScreenState::Select,
        AppSceneKind::Decide => ScreenState::Decide,
        AppSceneKind::Play if kind.course => ScreenState::Course,
        AppSceneKind::Play if kind.practice => ScreenState::Practice,
        AppSceneKind::Play if kind.autoplay => ScreenState::Autoplay,
        AppSceneKind::Play if kind.replay => ScreenState::Replay,
        AppSceneKind::Play => ScreenState::Play,
        AppSceneKind::Result if kind.course => ScreenState::Course,
        AppSceneKind::Result => ScreenState::Result,
    }
}

/// The values of a running play, from its render snapshot.
fn snapshot_play_values(
    snapshot: &bmz_render::snapshot::RenderSnapshot,
) -> crate::arena::session::PlayValues {
    let counts = &snapshot.judge_counts;
    crate::arena::session::PlayValues {
        exscore: snapshot.ex_score,
        processed_notes: snapshot.past_notes,
        minbp: counts.bad + counts.poor + counts.empty_poor,
        combo_break: counts.bad + counts.poor,
        max_combo: snapshot.max_combo,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::ScreenState;

    fn kind(course: bool, practice: bool, autoplay: bool, replay: bool) -> ArenaPlayKind {
        ArenaPlayKind { course, practice, autoplay, replay }
    }

    #[test]
    fn plays_are_told_apart_for_the_server() {
        let none = ArenaPlayKind::default();
        assert_eq!(arena_screen_for(AppSceneKind::Select, none), ScreenState::Select);
        assert_eq!(arena_screen_for(AppSceneKind::Decide, none), ScreenState::Decide);
        assert_eq!(arena_screen_for(AppSceneKind::Play, none), ScreenState::Play);
        assert_eq!(arena_screen_for(AppSceneKind::Result, none), ScreenState::Result);
        assert_eq!(
            arena_screen_for(AppSceneKind::Play, kind(false, true, false, false)),
            ScreenState::Practice
        );
        assert_eq!(
            arena_screen_for(AppSceneKind::Play, kind(false, false, true, false)),
            ScreenState::Autoplay
        );
        assert_eq!(
            arena_screen_for(AppSceneKind::Play, kind(false, false, false, true)),
            ScreenState::Replay
        );
        // A course owns its play and its result screens.
        assert_eq!(
            arena_screen_for(AppSceneKind::Play, kind(true, false, false, false)),
            ScreenState::Course
        );
        assert_eq!(
            arena_screen_for(AppSceneKind::Result, kind(true, false, false, false)),
            ScreenState::Course
        );
        // The course wins over an autoplayed or replayed stage.
        assert_eq!(
            arena_screen_for(AppSceneKind::Play, kind(true, false, true, false)),
            ScreenState::Course
        );
    }

    #[test]
    fn snapshot_values_split_bp_and_combo_break() {
        let snapshot = bmz_render::snapshot::RenderSnapshot {
            ex_score: 12,
            past_notes: 9,
            max_combo: 7,
            judge_counts: bmz_render::snapshot::DisplayJudgeCounts {
                bad: 1,
                poor: 2,
                empty_poor: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let values = snapshot_play_values(&snapshot);
        assert_eq!(values.exscore, 12);
        assert_eq!(values.processed_notes, 9);
        assert_eq!(values.minbp, 7);
        assert_eq!(values.combo_break, 3);
        assert_eq!(values.max_combo, 7);
    }
}

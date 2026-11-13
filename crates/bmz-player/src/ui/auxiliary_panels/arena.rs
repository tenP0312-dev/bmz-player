use super::*;
use crate::arena::session::{ArenaPanelState, MatchEnd, MatchStage};
use crate::arena::{ConnectionStatus, connection::ArenaHandle};

/// Select-screen Arena panel (rooms and nominations, stage B2). Shown only while
/// connected. It sends commands to the connection thread and reads its state; it
/// never starts a play and takes no key input of its own. On the result screen
/// (`results_only`) it shows the match's end and result, without room controls.
pub(super) fn build_arena_select_panel(
    ctx: &egui::Context,
    arena: &ArenaHandle,
    text: Localizer,
    results_only: bool,
) {
    let panel = arena.panel();
    if panel.status != ConnectionStatus::Connected {
        return;
    }
    let code_id = egui::Id::new("arena_join_code");
    let password_id = egui::Id::new("arena_join_password");
    let mut code = ctx.data(|data| data.get_temp::<String>(code_id)).unwrap_or_default();
    let mut password = ctx.data(|data| data.get_temp::<String>(password_id)).unwrap_or_default();
    egui::Window::new(tr!(text, "arena-panel-title"))
        .id(egui::Id::new("arena_select_panel"))
        .default_open(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
        .show(ctx, |ui| {
            if !results_only {
                build_room_controls(ui, arena, &panel, &mut code, &mut password, text);
            }
            build_match_status(ui, arena, &panel, text, results_only);
            build_match_result(ui, &panel, text);
        });
    ctx.data_mut(|data| {
        data.insert_temp(code_id, code);
        data.insert_temp(password_id, password);
    });
}

fn build_room_controls(
    ui: &mut egui::Ui,
    arena: &ArenaHandle,
    panel: &ArenaPanelState,
    code: &mut String,
    password: &mut String,
    text: Localizer,
) {
    let Some(room) = &panel.room else {
        if ui.button(tr!(text, "arena-room-create")).clicked() {
            arena.create_room();
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(code)
                    .hint_text(tr!(text, "arena-room-code"))
                    .desired_width(90.0),
            );
            ui.add(
                egui::TextEdit::singleline(password)
                    .hint_text(tr!(text, "arena-room-password"))
                    .password(true)
                    .desired_width(90.0),
            );
            if ui
                .add_enabled(
                    !code.trim().is_empty(),
                    egui::Button::new(tr!(text, "arena-room-join")),
                )
                .clicked()
            {
                arena.join_room(code.trim().to_owned(), Some(password.clone()));
            }
        });
        return;
    };
    match &room.code {
        Some(room_code) => ui.label(tr!(text, "arena-room-current", "code" => room_code.as_str())),
        None => ui.label(tr!(text, "arena-room-pending")),
    };
    let mut ready = room.ready;
    if ui.checkbox(&mut ready, tr!(text, "arena-ready")).changed() {
        arena.set_ready(ready);
    }
    for player in &room.players {
        let state = if player.ready {
            tr!(text, "arena-member-ready")
        } else {
            tr!(text, "arena-member-waiting")
        };
        let mut line =
            tr!(text, "arena-member-line", "name" => player.name.as_str(), "state" => state);
        if player.host {
            line = format!("{line} {}", tr!(text, "arena-host-tag"));
        }
        ui.label(line);
    }
    ui.horizontal(|ui| {
        if ui.button(tr!(text, "arena-room-leave")).clicked() {
            arena.leave_room();
        }
        let is_host = room.host_id.is_some_and(|id| Some(id) == panel.own_player_id);
        if is_host && ui.button(tr!(text, "arena-room-settings-apply")).clicked() {
            arena.apply_room_settings();
        }
    });
}

fn build_match_status(
    ui: &mut egui::Ui,
    arena: &ArenaHandle,
    panel: &ArenaPanelState,
    text: Localizer,
    results_only: bool,
) {
    if let Some(end) = &panel.match_end {
        match end {
            MatchEnd::Cancelled(reason) => ui.label(cancel_reason_text(text, reason)),
            MatchEnd::Released => ui.label(tr!(text, "arena-end-released")),
            MatchEnd::Forfeited(reason) => ui.label(forfeit_reason_text(text, reason)),
            MatchEnd::RoomDisbanded => ui.label(tr!(text, "arena-end-room-disbanded")),
            MatchEnd::RoomKicked => ui.label(tr!(text, "arena-end-room-kicked")),
        };
    }
    let Some(current) = &panel.current_match else {
        return;
    };
    if current.finish_current_song {
        ui.label(tr!(text, "arena-finish-current-song"));
    }
    let stage = match current.stage {
        MatchStage::Reserved => tr!(text, "arena-stage-reserved"),
        MatchStage::Filling => tr!(
            text,
            "arena-stage-filling",
            "count" => current.player_count.unwrap_or(0),
            "max" => current.max_players.unwrap_or(0)
        ),
        MatchStage::Nominating => tr!(text, "arena-stage-nominating"),
        MatchStage::Revealed => tr!(text, "arena-stage-revealed"),
        MatchStage::ChartChosen => tr!(text, "arena-stage-chart"),
        MatchStage::Options => tr!(
            text,
            "arena-stage-options",
            "ready" => current.option.as_ref().and_then(|o| o.ready_count).unwrap_or(0),
            "count" => current.option.as_ref().and_then(|o| o.player_count).unwrap_or(0)
        ),
        MatchStage::Loading => tr!(text, "arena-stage-loading"),
        MatchStage::Countdown => tr!(text, "arena-stage-countdown"),
        MatchStage::Playing => tr!(text, "arena-stage-playing"),
        MatchStage::Finished => tr!(text, "arena-stage-finished"),
    };
    ui.label(stage);
    // Giving up is possible until the song's `final` is made (after that the
    // server settles the match).
    if !results_only
        && current.pending_final.is_none()
        && ui.button(tr!(text, "arena-withdraw")).clicked()
    {
        arena.withdraw();
    }
    for player in &current.live_players {
        let line = tr!(
            text,
            "arena-live-line",
            "name" => player.name.as_str(),
            "ex" => player.exscore,
            "done" => player.processed_notes,
            "bp" => player.minbp
        );
        if player.connected {
            ui.label(line);
        } else {
            ui.label(format!("{line} {}", tr!(text, "arena-live-disconnected")));
        }
    }
    if let (Some(deadline), Some(offset)) = (current.deadline, panel.clock_offset_secs) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        let left = (deadline - (now + offset)).max(0.0).ceil() as u64;
        ui.label(tr!(text, "arena-time-left", "seconds" => left));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    if current.stage == MatchStage::Nominating {
        if !current.retry_reason.is_empty() {
            ui.label(retry_reason_text(text, &current.retry_reason));
        }
        // `can_nominate` is false when the server wants none from this player
        // (host or rotate policy).
        if !current.can_nominate {
        } else if current.nominated {
            ui.label(tr!(text, "arena-nominated"));
        } else {
            ui.horizontal(|ui| {
                let selected = panel.selected_chart_md5.clone();
                if ui
                    .add_enabled(selected.is_some(), egui::Button::new(tr!(text, "arena-nominate")))
                    .clicked()
                    && let Some(md5) = selected
                {
                    arena.nominate(md5);
                }
                if ui.button(tr!(text, "arena-skip")).clicked() {
                    arena.skip_nomination();
                }
            });
        }
    }
    if current.stage == MatchStage::Options
        && let Some(option) = &current.option
    {
        if option.ready {
            ui.label(tr!(text, "arena-option-sent"));
        } else if ui.button(tr!(text, "arena-option-ready")).clicked() {
            arena.option_ready();
        }
    }
    if let Some(chart) = &current.chart {
        ui.label(format!("{} / {}", chart.title, chart.artist));
        if panel.pending_chart_check.is_some() {
            ui.label(tr!(text, "arena-checking-chart"));
        }
    }
}

/// The sentence for a `match_cancelled.reason`. A reason this build does not
/// know (the server may add more) is shown as a plain cancellation.
fn cancel_reason_text(text: Localizer, reason: &str) -> String {
    match reason {
        "not_enough_players_with_chart" => tr!(text, "arena-cancel-not-enough-players-with-chart"),
        "load_timeout" => tr!(text, "arena-cancel-load-timeout"),
        "loaded_longnote_contract_mismatch" => {
            tr!(text, "arena-cancel-loaded-longnote-contract-mismatch")
        }
        "not_enough_ready_players" => tr!(text, "arena-cancel-not-enough-ready-players"),
        "not_enough_option_ready_players" => {
            tr!(text, "arena-cancel-not-enough-option-ready-players")
        }
        "not_enough_connected_players" => tr!(text, "arena-cancel-not-enough-connected-players"),
        "not_enough_players_after_withdrawal" => {
            tr!(text, "arena-cancel-not-enough-players-after-withdrawal")
        }
        "reservation_timeout" => tr!(text, "arena-cancel-reservation-timeout"),
        "server_fault" => tr!(text, "arena-cancel-server-fault"),
        crate::arena::session::ROOM_DISBANDED_REASON => {
            tr!(text, "arena-cancel-private-room-disbanded")
        }
        _ => tr!(text, "arena-end-cancelled-unknown"),
    }
}

/// The sentence for a `forfeit_accepted.reason`; an unknown one is a plain
/// "you forfeited" (the code is not shown).
fn forfeit_reason_text(text: Localizer, reason: &str) -> String {
    match reason {
        "player_withdrew" => tr!(text, "arena-end-forfeited-player-withdrew"),
        "play_aborted" => tr!(text, "arena-end-forfeited-play-aborted"),
        "disconnect_timeout" => tr!(text, "arena-end-forfeited-disconnect-timeout"),
        "final_result_timeout" => tr!(text, "arena-end-forfeited-final-result-timeout"),
        "private_room_kicked" => tr!(text, "arena-end-forfeited-private-room-kicked"),
        _ => tr!(text, "arena-end-forfeited-unknown"),
    }
}

/// The sentence for a nomination `retry_reason` (and the `chart_missing` reason
/// of `chart_candidate_rejected`); an unknown one asks to nominate again.
fn retry_reason_text(text: Localizer, reason: &str) -> String {
    match reason {
        "free_selection_requires_nomination" => {
            tr!(text, "arena-nomination-retry-free-selection-requires-nomination")
        }
        "free_selection_requires_unique_nomination" => {
            tr!(text, "arena-nomination-retry-free-selection-requires-unique-nomination")
        }
        "no_shared_free_selection_candidate" => {
            tr!(text, "arena-nomination-retry-no-shared-free-selection-candidate")
        }
        "chart_missing" => tr!(text, "arena-nomination-retry-chart-missing"),
        _ => tr!(text, "arena-nomination-retry-unknown"),
    }
}

/// The last match's result: placement, name, EX and clear (`DNF` for a forfeit).
fn build_match_result(ui: &mut egui::Ui, panel: &ArenaPanelState, text: Localizer) {
    let Some(result) = &panel.result else {
        return;
    };
    ui.separator();
    ui.label(tr!(text, "arena-result-title"));
    for player in &result.players {
        let mut line = tr!(
            text,
            "arena-result-line",
            "place" => player.placement.map_or_else(|| "-".to_owned(), |place| place.to_string()),
            "name" => player.name.as_str(),
            "ex" => player.exscore,
            "clear" => player.clear_label.as_str()
        );
        if Some(player.player_id) == panel.own_player_id {
            line = format!("> {line}");
        }
        ui.label(line);
    }
}

#[cfg(test)]
mod cancel_reason_tests {
    use super::*;
    use crate::i18n::AppLocale;

    const KNOWN: [&str; 10] = [
        "not_enough_players_with_chart",
        "load_timeout",
        "loaded_longnote_contract_mismatch",
        "not_enough_ready_players",
        "not_enough_option_ready_players",
        "not_enough_connected_players",
        "not_enough_players_after_withdrawal",
        "reservation_timeout",
        "server_fault",
        "private_room_disbanded",
    ];
    const FORFEIT: [&str; 5] = [
        "player_withdrew",
        "play_aborted",
        "disconnect_timeout",
        "final_result_timeout",
        "private_room_kicked",
    ];
    const RETRY: [&str; 4] = [
        "free_selection_requires_nomination",
        "free_selection_requires_unique_nomination",
        "no_shared_free_selection_candidate",
        "chart_missing",
    ];
    const LOCALES: [AppLocale; 6] = [
        AppLocale::Ja,
        AppLocale::En,
        AppLocale::Ko,
        AppLocale::ZhCn,
        AppLocale::ZhTw,
        AppLocale::ZhHk,
    ];

    #[test]
    fn forfeit_and_retry_reasons_have_sentences_and_unknown_ones_are_generic() {
        for locale in LOCALES {
            let text = Localizer::new(locale);
            let forfeit: fn(Localizer, &str) -> String = forfeit_reason_text;
            for (reasons, sentence, generic_text) in [
                (&FORFEIT[..], forfeit, tr!(text, "arena-end-forfeited-unknown")),
                (&RETRY[..], retry_reason_text, tr!(text, "arena-nomination-retry-unknown")),
            ] {
                let mut seen = std::collections::BTreeSet::new();
                for reason in reasons {
                    let line = sentence(text, reason);
                    assert_ne!(line, generic_text, "{locale:?} {reason}");
                    assert!(!line.contains(reason), "{locale:?} {reason}");
                    assert!(seen.insert(line), "{locale:?} {reason} duplicated");
                }
                let unknown = sentence(text, "brand_new_reason");
                assert_eq!(unknown, generic_text, "{locale:?}");
                assert!(!unknown.contains("brand_new_reason"));
            }
        }
    }

    #[test]
    fn every_known_reason_has_its_own_sentence_in_every_language() {
        for locale in [
            AppLocale::Ja,
            AppLocale::En,
            AppLocale::Ko,
            AppLocale::ZhCn,
            AppLocale::ZhTw,
            AppLocale::ZhHk,
        ] {
            let text = Localizer::new(locale);
            let generic = cancel_reason_text(text, "some_future_reason");
            let mut seen = std::collections::BTreeSet::new();
            for reason in KNOWN {
                let sentence = cancel_reason_text(text, reason);
                assert_ne!(sentence, generic, "{locale:?} {reason}");
                assert!(!sentence.contains(reason), "{locale:?} {reason}");
                assert!(seen.insert(sentence), "{locale:?} {reason} is not unique");
            }
        }
    }

    #[test]
    fn unknown_reason_is_the_generic_sentence_without_the_code() {
        let text = Localizer::new(AppLocale::Ja);
        let sentence = cancel_reason_text(text, "brand_new_reason");
        assert_eq!(sentence, tr!(text, "arena-end-cancelled-unknown"));
        assert!(!sentence.contains("brand_new_reason"));
    }
}

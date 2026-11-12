use super::*;
use crate::arena::session::{ArenaPanelState, MatchEnd, MatchStage};
use crate::arena::{ConnectionStatus, connection::ArenaHandle};

/// Select-screen Arena panel (rooms and nominations, stage B2). Shown only while
/// connected. It sends commands to the connection thread and reads its state; it
/// never starts a play and takes no key input of its own.
pub(super) fn build_arena_select_panel(ctx: &egui::Context, arena: &ArenaHandle, text: Localizer) {
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
            build_room_controls(ui, arena, &panel, &mut code, &mut password, text);
            build_match_status(ui, arena, &panel, text);
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
) {
    if let Some(end) = &panel.match_end {
        match end {
            MatchEnd::Cancelled(reason) => {
                ui.label(tr!(text, "arena-end-cancelled", "reason" => reason.as_str()))
            }
            MatchEnd::Released => ui.label(tr!(text, "arena-end-released")),
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
    };
    ui.label(stage);
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
            ui.label(
                tr!(text, "arena-nomination-retry", "reason" => current.retry_reason.as_str()),
            );
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

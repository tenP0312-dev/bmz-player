use super::*;
use crate::arena::session::DisconnectReason;
use crate::arena::{ConnectionStatus, RefusalKind};

/// BMS-IR Arena: the on/off switch (off by default) and the connection state.
pub(in crate::ui::profile_panel) fn build_profile_arena_section(
    ui: &mut egui::Ui,
    section: &mut ProfileSectionContext<'_>,
) {
    let text = section.text;
    let unrestricted = section.unrestricted;
    let arena = section.arena;
    let panel = arena.panel();
    let profile = &mut *section.profile;
    let mut changed = false;
    SettingsSection::new(SettingsPage::Ir, tr!(text, "arena-title"))
        .scope(tr!(text, "settings-scope-profile"))
        .id_salt("profile_arena")
        .show(ui, |ui| {
            if !unrestricted {
                ui.disable();
            }
            ui.label(tr!(text, "arena-help"));
            changed |=
                ui.checkbox(&mut profile.arena.enabled, tr!(text, "arena-enabled")).changed();

            let (status_text, is_problem) = match panel.status {
                ConnectionStatus::Disabled => (tr!(text, "arena-status-disabled"), false),
                ConnectionStatus::NoCredentials => {
                    (tr!(text, "arena-status-no-credentials"), true)
                }
                ConnectionStatus::UnsupportedRule => {
                    (tr!(text, "arena-status-unsupported-rule"), true)
                }
                ConnectionStatus::Connecting => (tr!(text, "arena-status-connecting"), false),
                ConnectionStatus::Connected => (tr!(text, "arena-status-connected"), false),
                ConnectionStatus::Reconnecting => {
                    let seconds = panel
                        .retry_at_unix
                        .map(|at| (at - unix_now()).max(0.0).ceil() as u64)
                        .unwrap_or(0);
                    let reason = match panel.last_disconnect {
                        Some(DisconnectReason::IdleTimeout) => tr!(text, "arena-reason-idle"),
                        _ => tr!(text, "arena-reason-lost"),
                    };
                    (tr!(text, "arena-status-reconnecting", "seconds" => seconds, "reason" => reason), true)
                }
                ConnectionStatus::Refused(RefusalKind::UpdateRequired) => {
                    (tr!(text, "arena-status-refused-update"), true)
                }
                ConnectionStatus::Refused(RefusalKind::AuthenticationFailed) => {
                    (tr!(text, "arena-status-refused-auth"), true)
                }
                ConnectionStatus::Refused(RefusalKind::FlavorConflict) => {
                    (tr!(text, "arena-status-refused-conflict"), true)
                }
                ConnectionStatus::Replaced => (tr!(text, "arena-status-replaced"), true),
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(tr!(text, "arena-status"));
                if is_problem {
                    ui.colored_label(egui::Color32::from_rgb(0xe0, 0x8a, 0x2e), status_text);
                } else {
                    ui.label(status_text);
                }
            });
            if matches!(panel.status, ConnectionStatus::Refused(_) | ConnectionStatus::Replaced)
                && ui.button(tr!(text, "arena-reconnect")).clicked()
            {
                arena.reconnect();
            }
            if let Some(notice) = &panel.notice {
                ui.label(tr!(text, "arena-notice", "code" => notice.code.as_str()));
            }
            if panel.status == ConnectionStatus::Connected {
                if let (Some(offset), Some(round_trip)) =
                    (panel.clock_offset_secs, panel.round_trip_secs)
                {
                    ui.label(tr!(
                        text,
                        "arena-clock",
                        "offset" => format!("{:+.0}", offset * 1000.0),
                        "rtt" => format!("{:.0}", round_trip * 1000.0)
                    ));
                }
                if let Some(player) = panel.player {
                    if let Some(rating) = player.rating {
                        ui.label(tr!(text, "arena-rating", "rating" => rating));
                    }
                    if let Some(matches) = player.matches_played {
                        ui.label(tr!(text, "arena-matches-played", "count" => matches));
                    }
                }
                if panel.active_match_id.is_some() {
                    ui.label(tr!(text, "arena-active-match"));
                }
            }
        });
    if changed {
        section.save_clicked = true;
    }
}

fn unix_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

//! B3: `live`, `final`, resend after a reconnect, forfeit, result and the
//! `state` / `ready` rules (spec 3.3, 3.5, 3.6).

use super::load_start_tests::{in_loading, prepare};
use super::match_setup_tests::{MD5, at, connected, hello_ok, json, recv, reserved, sent};
use super::*;
use crate::arena::protocol::{ArenaResult, LivePlayer, LiveSnapshot, ResultPlayer, Start};

fn values(ex: u32, processed: u32, bp: u32, cb: u32, combo: u32) -> PlayValues {
    PlayValues {
        exscore: ex,
        processed_notes: processed,
        minbp: bp,
        combo_break: cb,
        max_combo: combo,
    }
}

/// `m1` is playing: loaded with 100 notes, started, the clock running.
fn in_playing(session: &mut ArenaSession) {
    in_loading(session);
    session.handle(
        at(3.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 100 },
    );
    recv(
        session,
        ServerMessage::Start(Start {
            match_id: "m1".into(),
            start_at: Some(1_010.0),
            ln_mode: "LN".into(),
        }),
    );
    session.handle(at(10.0), Event::PlayStarted { match_id: "m1".into() });
}

fn sample(session: &mut ArenaSession, secs: f64, v: PlayValues) -> Vec<ClientMessage> {
    sent(&session.handle(at(secs), Event::LiveSample { match_id: "m1".into(), values: v }))
}

fn finish(
    session: &mut ArenaSession,
    secs: f64,
    v: PlayValues,
    failed: bool,
    clear: u8,
) -> Vec<ClientMessage> {
    sent(&session.handle(
        at(secs),
        Event::PlayFinished { match_id: "m1".into(), values: v, failed, clear_type: clear },
    ))
}

fn result_for(match_id: &str) -> ServerMessage {
    ServerMessage::Result(ArenaResult {
        match_id: match_id.into(),
        players: vec![
            ResultPlayer {
                player_id: 7,
                name: "me".into(),
                placement: Some(1),
                exscore: 180,
                final_state: "complete".into(),
                clear_label: "HARD".into(),
                ..ResultPlayer::default()
            },
            ResultPlayer {
                player_id: 8,
                name: "you".into(),
                placement: Some(2),
                final_state: "forfeit".into(),
                clear_label: "DNF".into(),
                ..ResultPlayer::default()
            },
        ],
        rated: false,
        room_retained_player_ids: vec![7, 8],
    })
}

#[test]
fn live_is_sent_once_a_second_with_a_growing_seq_and_the_confirmed_option() {
    let mut session = connected();
    in_playing(&mut session);
    let first = sample(&mut session, 11.0, values(10, 5, 1, 1, 4));
    assert_eq!(
        json(&first[0]),
        serde_json::json!({"type":"live","match_id":"m1","seq":1,"exscore":10,
            "processed_notes":5,"minbp":1,"combo_break":1,"max_combo":4,
            "play_option":2,"play_mode":7,"ln_mode":"LN"})
    );
    // Closer than a second: nothing.
    assert!(sample(&mut session, 11.5, values(12, 6, 1, 1, 5)).is_empty());
    let second = sample(&mut session, 12.0, values(14, 7, 1, 1, 6));
    assert_eq!(json(&second[0])["seq"], 2);
    assert_eq!(json(&second[0])["exscore"], 14);
}

#[test]
fn live_never_goes_back_and_stays_inside_what_the_server_accepts() {
    let mut session = connected();
    in_playing(&mut session);
    sample(&mut session, 11.0, values(20, 10, 2, 2, 8));
    // Values that fell, and a note count above the song's.
    let next = sample(&mut session, 12.0, values(5, 500, 0, 0, 3));
    let live = json(&next[0]);
    assert_eq!(live["processed_notes"], 100);
    assert_eq!(live["minbp"], 2);
    assert_eq!(live["combo_break"], 2);
    assert_eq!(live["max_combo"], 8);
    // EX never falls below the last 20.
    assert_eq!(live["exscore"], 20);
}

#[test]
fn live_ex_grows_by_at_most_two_per_new_note() {
    let mut session = connected();
    in_playing(&mut session);
    sample(&mut session, 11.0, values(0, 0, 0, 0, 0));
    // 10 new notes allow at most +20, whatever the sample says.
    let next = sample(&mut session, 12.0, values(1_000, 10, 0, 0, 10));
    assert_eq!(json(&next[0])["exscore"], 20);
    let next = sample(&mut session, 13.0, values(30, 100, 0, 0, 100));
    assert_eq!(json(&next[0])["exscore"], 30);
    assert_eq!(json(&next[0])["max_combo"], 100);
}

#[test]
fn live_waits_for_the_clock_the_right_match_and_a_loaded_chart() {
    let mut session = connected();
    in_loading(&mut session);
    // Not loaded, not started.
    assert!(sample(&mut session, 5.0, values(0, 0, 0, 0, 0)).is_empty());
    session.handle(
        at(6.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 100 },
    );
    recv(
        &mut session,
        ServerMessage::Start(Start {
            match_id: "m1".into(),
            start_at: Some(1_010.0),
            ln_mode: "LN".into(),
        }),
    );
    assert!(sample(&mut session, 7.0, values(0, 0, 0, 0, 0)).is_empty());
    session.handle(at(10.0), Event::PlayStarted { match_id: "other".into() });
    assert!(sample(&mut session, 11.0, values(0, 0, 0, 0, 0)).is_empty());
    session.handle(at(10.0), Event::PlayStarted { match_id: "m1".into() });
    assert!(
        sent(&session.handle(
            at(12.0),
            Event::LiveSample { match_id: "other".into(), values: values(0, 0, 0, 0, 0) }
        ))
        .is_empty()
    );
    assert_eq!(sample(&mut session, 13.0, values(0, 0, 0, 0, 0)).len(), 1);
}

#[test]
fn a_complete_play_sends_final_with_every_note_processed_and_stops_live() {
    let mut session = connected();
    in_playing(&mut session);
    sample(&mut session, 11.0, values(100, 50, 0, 0, 50));
    // The play sampled 98 judged notes at the end; a complete play reports all 100.
    let messages = finish(&mut session, 12.0, values(190, 98, 3, 2, 60), false, 6);
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"final","match_id":"m1","seq":2,"exscore":190,
            "processed_notes":100,"minbp":3,"combo_break":2,"max_combo":60,
            "play_option":2,"play_mode":7,"ln_mode":"LN","state":"complete","clear_type":6})
    );
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Finished);
    // One `final` only, and no `live` after it.
    assert!(finish(&mut session, 12.1, values(190, 100, 3, 2, 60), false, 6).is_empty());
    assert!(sample(&mut session, 14.0, values(190, 100, 3, 2, 60)).is_empty());
}

#[test]
fn a_failed_play_is_hard_fail_with_clear_1() {
    let mut session = connected();
    in_playing(&mut session);
    let messages = finish(&mut session, 12.0, values(40, 30, 9, 9, 10), true, 1);
    let report = json(&messages[0]);
    assert_eq!(report["state"], "hard_fail");
    assert_eq!(report["clear_type"], 1);
    // The real count, not the song's.
    assert_eq!(report["processed_notes"], 30);
}

#[test]
fn final_state_and_clear_type_follow_what_the_server_accepts() {
    // complete needs clear >= 2, hard_fail needs clear == 1.
    assert_eq!(final_state_for(false, 6), (FinalState::Complete, 6));
    assert_eq!(final_state_for(false, 2), (FinalState::Complete, 2));
    assert_eq!(final_state_for(false, 10), (FinalState::Complete, 10));
    assert_eq!(final_state_for(false, 99), (FinalState::Complete, 10));
    // Finished with the gauge under the border: FAILED clear, so hard_fail.
    assert_eq!(final_state_for(false, 1), (FinalState::HardFail, 1));
    assert_eq!(final_state_for(false, 0), (FinalState::HardFail, 1));
    assert_eq!(final_state_for(true, 8), (FinalState::HardFail, 1));
}

#[test]
fn final_is_sent_again_after_a_reconnect_before_state() {
    let mut session = connected();
    in_playing(&mut session);
    // The connection drops, then the song ends while offline.
    session.handle(at(11.0), Event::SocketClosed);
    assert!(finish(&mut session, 12.0, values(190, 100, 3, 2, 60), false, 6).is_empty());
    let kept = session.panel().current_match.unwrap();
    assert!(kept.pending_final.is_some());
    // Reconnect: the server still has the match.
    session.handle(at(17.0), Event::Tick);
    session.handle(at(17.0), Event::SocketOpened);
    let messages = sent(&session.handle(at(17.1), Event::Message(hello_ok("m1"))));
    let types: Vec<_> =
        messages.iter().map(|m| json(m)["type"].as_str().unwrap().to_owned()).collect();
    assert_eq!(types[0], "final");
    assert_eq!(types[1], "state");
    assert_eq!(json(&messages[0])["seq"], 1);
    // The match survives and keeps its confirmed values.
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Finished);
    // `result` ends it and drops the stored `final`.
    recv(&mut session, result_for("m1"));
    assert!(session.panel().current_match.is_none());
    assert!(session.panel().result.is_some());
}

#[test]
fn play_ready_is_sent_again_after_a_reconnect_while_waiting_for_the_start() {
    let mut session = connected();
    in_loading(&mut session);
    session.handle(at(2.5), Event::PrepareTaken { match_id: "m1".into() });
    session.handle(
        at(3.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 321 },
    );
    session.handle(at(4.0), Event::SocketClosed);
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Loading);
    session.handle(at(9.0), Event::Tick);
    session.handle(at(9.0), Event::SocketOpened);
    let messages = sent(&session.handle(at(9.1), Event::Message(hello_ok("m1"))));
    assert_eq!(json(&messages[0])["type"], "play_ready");
    assert_eq!(json(&messages[0])["totalnotes"], 321);
    assert_eq!(json(&messages[1])["type"], "state");
    // The server repeats its snapshot: nothing starts again.
    recv(&mut session, reserved("m1"));
    recv(&mut session, prepare(2));
    recv(&mut session, super::match_setup_tests::chart(Some("m1"), MD5));
    let panel = session.panel();
    assert!(panel.pending_prepare.is_none());
    assert!(panel.pending_chart_check.is_none());
    assert_eq!(panel.current_match.unwrap().stage, MatchStage::Loading);
}

#[test]
fn a_match_the_server_no_longer_has_is_dropped_on_reconnect() {
    let mut session = connected();
    in_playing(&mut session);
    session.handle(at(11.0), Event::SocketClosed);
    session.handle(at(16.0), Event::Tick);
    session.handle(at(16.0), Event::SocketOpened);
    let messages = sent(&session.handle(at(16.1), Event::Message(hello_ok(""))));
    assert!(messages.iter().all(|m| json(m)["type"] != "final"));
    assert!(session.panel().current_match.is_none());
    // Another match id is not ours either.
    let mut session = connected();
    in_playing(&mut session);
    session.handle(at(11.0), Event::SocketClosed);
    session.handle(at(16.0), Event::Tick);
    session.handle(at(16.0), Event::SocketOpened);
    session.handle(at(16.1), Event::Message(hello_ok("m2")));
    assert!(session.panel().current_match.is_none());
}

fn reconnect(session: &mut ArenaSession, closed: f64, active: &str) -> Vec<ClientMessage> {
    session.handle(at(closed), Event::SocketClosed);
    session.handle(at(closed + 5.0), Event::Tick);
    session.handle(at(closed + 5.0), Event::SocketOpened);
    sent(&session.handle(at(closed + 5.1), Event::Message(hello_ok(active))))
}

#[test]
fn a_load_the_app_began_is_kept_across_a_reconnect() {
    // Dropping it would leave READY running without the start gate.
    let mut session = connected();
    in_loading(&mut session);
    session.handle(at(4.0), Event::SocketClosed);
    let kept = session.panel();
    assert_eq!(kept.current_match.unwrap().stage, MatchStage::Loading);
    assert_eq!(kept.pending_prepare.unwrap().match_id, "m1");
}

#[test]
fn a_load_that_finishes_while_offline_says_play_ready_after_the_reconnect() {
    let mut session = connected();
    in_loading(&mut session);
    session.handle(at(3.0), Event::PrepareTaken { match_id: "m1".into() });
    session.handle(at(4.0), Event::SocketClosed);
    let offline = session.handle(
        at(5.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 250 },
    );
    assert!(sent(&offline).is_empty());
    session.handle(at(9.0), Event::Tick);
    session.handle(at(9.0), Event::SocketOpened);
    let messages = sent(&session.handle(at(9.1), Event::Message(hello_ok("m1"))));
    assert_eq!(json(&messages[0])["type"], "play_ready");
    assert_eq!(json(&messages[0])["totalnotes"], 250);
    // The repeated `prepare` does not start the song again.
    recv(&mut session, prepare(2));
    assert!(session.panel().pending_prepare.is_none());
}

#[test]
fn match_resume_playing_opens_the_start_gate_after_a_long_outage() {
    let mut session = connected();
    in_loading(&mut session);
    session.handle(
        at(3.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 100 },
    );
    reconnect(&mut session, 4.0, "m1");
    recv(
        &mut session,
        ServerMessage::MatchResume {
            match_id: "m1".into(),
            state: "playing".into(),
            start_at: Some(1_006.0),
        },
    );
    let panel = session.panel();
    assert_eq!(panel.start_gate.unwrap().start_at, 1_006.0);
    assert_eq!(panel.current_match.unwrap().stage, MatchStage::Countdown);
}

#[test]
fn cancel_and_release_for_another_match_are_ignored() {
    let mut session = connected();
    in_playing(&mut session);
    recv(
        &mut session,
        ServerMessage::MatchCancelled { match_id: "old".into(), reason: "load_timeout".into() },
    );
    recv(&mut session, ServerMessage::MatchReleased { match_id: "old".into() });
    let panel = session.panel();
    assert_eq!(panel.current_match.unwrap().stage, MatchStage::Playing);
    assert!(panel.match_end.is_none());
}

#[test]
fn live_waits_until_one_second_before_start_at() {
    let mut session = connected();
    in_loading(&mut session);
    session.handle(
        at(3.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 100 },
    );
    recv(
        &mut session,
        ServerMessage::Start(Start {
            match_id: "m1".into(),
            start_at: Some(1_010.0),
            ln_mode: "LN".into(),
        }),
    );
    // The clock runs a pre-roll (here 4 s) before the song's 0.
    session.handle(at(6.0), Event::PlayStarted { match_id: "m1".into() });
    assert!(sample(&mut session, 6.5, values(0, 0, 0, 0, 0)).is_empty());
    assert!(sample(&mut session, 8.9, values(0, 0, 0, 0, 0)).is_empty());
    assert_eq!(sample(&mut session, 9.0, values(0, 0, 0, 0, 0)).len(), 1);
}

#[test]
fn the_result_is_shown_and_the_ready_request_stays() {
    let mut session = connected();
    in_playing(&mut session);
    session.handle(at(10.5), Event::SetReady(true));
    finish(&mut session, 12.0, values(190, 100, 3, 2, 60), false, 6);
    recv(&mut session, result_for("m1"));
    let panel = session.panel();
    assert!(panel.current_match.is_none());
    assert!(panel.match_end.is_none());
    let result = panel.result.unwrap();
    assert_eq!(result.players[0].name, "me");
    assert_eq!(result.players[1].clear_label, "DNF");
    assert!(panel.room.unwrap().ready);
    // A result for some other match is ignored while one is in view.
    recv(&mut session, reserved("m2"));
    recv(&mut session, result_for("m1"));
    assert!(session.panel().current_match.is_some());
    // The next match clears the old result.
    assert!(session.panel().result.is_none());
}

#[test]
fn forfeit_accepted_ends_the_match_and_drops_ready() {
    let mut session = connected();
    in_playing(&mut session);
    session.handle(at(10.5), Event::SetReady(true));
    recv(
        &mut session,
        ServerMessage::ForfeitAccepted { match_id: "m1".into(), reason: "player_withdrew".into() },
    );
    let panel = session.panel();
    assert!(panel.current_match.is_none());
    assert_eq!(panel.match_end, Some(MatchEnd::Forfeited("player_withdrew".into())));
    assert!(!panel.room.unwrap().ready);
}

#[test]
fn withdraw_sends_forfeit_until_final_was_produced() {
    let mut session = connected();
    assert!(sent(&session.handle(at(1.0), Event::Withdraw)).is_empty());
    in_playing(&mut session);
    let messages = sent(&session.handle(at(11.0), Event::Withdraw));
    assert_eq!(json(&messages[0]), serde_json::json!({"type":"forfeit","match_id":"m1"}));
    finish(&mut session, 12.0, values(190, 100, 3, 2, 60), false, 6);
    assert!(sent(&session.handle(at(13.0), Event::Withdraw)).is_empty());
}

#[test]
fn leaving_the_play_for_select_forfeits_before_state() {
    let mut session = connected();
    in_playing(&mut session);
    session.handle(at(10.0), Event::SetScreen(ScreenState::Play));
    let messages = sent(&session.handle(at(11.0), Event::SetScreen(ScreenState::Select)));
    assert_eq!(json(&messages[0])["type"], "forfeit");
    assert_eq!(json(&messages[1])["type"], "state");
}

#[test]
fn ready_follows_the_screen_and_the_ir_submission() {
    let mut session = connected();
    session.handle(at(1.0), Event::RoomCreate);
    let ready_of = |messages: Vec<ClientMessage>| {
        messages
            .iter()
            .map(json)
            .filter(|m| m["type"] == "state")
            .map(|m| (m["state"].as_str().unwrap().to_owned(), m["ready"].as_bool().unwrap()))
            .next_back()
    };
    let screen = |session: &mut ArenaSession, s| {
        ready_of(sent(&session.handle(at(2.0), Event::SetScreen(s))))
    };
    assert_eq!(
        ready_of(sent(&session.handle(at(2.0), Event::SetReady(true)))),
        Some(("select".into(), true))
    );
    // Only select (and a settled result) can be ready; the other screens never are.
    assert_eq!(screen(&mut session, ScreenState::Play), Some(("play".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Practice), Some(("practice".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Autoplay), Some(("autoplay".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Replay), Some(("replay".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Course), Some(("course".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Decide), Some(("decide".into(), false)));
    assert_eq!(screen(&mut session, ScreenState::Result), Some(("result".into(), false)));
    // Result: not ready until the normal IR submission is done.
    assert_eq!(
        ready_of(sent(&session.handle(at(3.0), Event::SetResultSettled(true)))),
        Some(("result".into(), true))
    );
    assert_eq!(screen(&mut session, ScreenState::Select), Some(("select".into(), true)));
    // Leaving the result screen forgets the settled flag.
    assert_eq!(screen(&mut session, ScreenState::Result), Some(("result".into(), false)));
}

#[test]
fn the_server_room_is_followed_after_a_reconnect() {
    use crate::arena::protocol::PlayerStatus;
    let mut session = connected();
    let status = |code: Option<&str>| {
        ServerMessage::ArenaStatus(PlayerStatus {
            queue_room_code: code.map(str::to_owned),
            queue_known: true,
            ..PlayerStatus::default()
        })
    };
    // After a reconnect the room is gone locally but the server still has it.
    assert!(session.panel().room.is_none());
    recv(&mut session, status(Some("ABC123")));
    assert_eq!(session.panel().room.unwrap().code.as_deref(), Some("ABC123"));
    // Gone on the server: gone here (no match in progress).
    recv(&mut session, status(None));
    assert!(session.panel().room.is_none());
    // An `arena_status` without `queue` says nothing about the room.
    recv(&mut session, status(Some("ABC123")));
    recv(&mut session, ServerMessage::ArenaStatus(PlayerStatus::default()));
    assert!(session.panel().room.is_some());
}

#[test]
fn room_disbanded_and_kicked_clear_the_room() {
    let mut session = connected();
    session.handle(at(1.0), Event::RoomCreate);
    recv(&mut session, ServerMessage::RoomDisbanded);
    assert!(session.panel().room.is_none());
    assert_eq!(session.panel().match_end, Some(MatchEnd::RoomDisbanded));
    session.handle(at(2.0), Event::RoomCreate);
    recv(&mut session, ServerMessage::RoomKicked);
    assert!(session.panel().room.is_none());
    assert_eq!(session.panel().match_end, Some(MatchEnd::RoomKicked));
}

#[test]
fn the_servers_live_snapshot_is_kept_for_the_panel() {
    let mut session = connected();
    in_playing(&mut session);
    recv(
        &mut session,
        ServerMessage::LiveSnapshot(LiveSnapshot {
            match_id: "m1".into(),
            state: "playing".into(),
            players: vec![LivePlayer { player_id: 8, exscore: 30, ..LivePlayer::default() }],
        }),
    );
    assert_eq!(session.panel().current_match.unwrap().live_players[0].exscore, 30);
    // Another match's snapshot is not shown.
    recv(
        &mut session,
        ServerMessage::LiveSnapshot(LiveSnapshot {
            match_id: "zz".into(),
            state: "playing".into(),
            players: Vec::new(),
        }),
    );
    assert_eq!(session.panel().current_match.unwrap().live_players.len(), 1);
}

#[test]
fn a_cancelled_match_while_playing_clears_the_stored_final() {
    let mut session = connected();
    in_playing(&mut session);
    finish(&mut session, 12.0, values(190, 100, 3, 2, 60), false, 6);
    recv(
        &mut session,
        ServerMessage::MatchCancelled { match_id: "m1".into(), reason: "server_fault".into() },
    );
    let panel = session.panel();
    assert!(panel.current_match.is_none());
    assert_eq!(panel.match_end, Some(MatchEnd::Cancelled("server_fault".into())));
}

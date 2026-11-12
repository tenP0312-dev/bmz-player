//! B2 (rooms and nominations): the match set-up flow of spec 3.4, steps 0-7.

use super::*;
use crate::arena::protocol::{
    ARENA_CLIENT_VERSION, CLIENT_FLAVOR, ChartAnnounced, HelloOk, MatchReserved, PROTOCOL_VERSION,
    RulesetProfile, passmd5,
};

const MD5: &str = "0123456789abcdef0123456789abcdef";

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        player_id: 7,
        passmd5: passmd5("token"),
        client_version: ARENA_CLIENT_VERSION.into(),
        build_hash: "a".repeat(40),
        body_version: "0.5.1".into(),
        client_flavor: CLIENT_FLAVOR.into(),
        ruleset_profile: RulesetProfile::Oraja,
        arena_enabled: true,
    }
}

fn at(secs: f64) -> Now {
    Now { mono: Duration::from_secs_f64(secs), unix: 1_000.0 + secs }
}

fn hello_ok(active_match_id: &str) -> ServerMessage {
    ServerMessage::HelloOk(HelloOk {
        session_id: "s1".into(),
        server_protocol: Some(8),
        heartbeat_seconds: Some(10.0),
        active_match_id: active_match_id.into(),
        server_time: Some(1_000.0),
    })
}

fn connected() -> ArenaSession {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    session.handle(at(0.0), Event::SocketOpened);
    session.handle(at(0.0), Event::Message(hello_ok("")));
    session
}

fn sent(actions: &[Action]) -> Vec<ClientMessage> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Send(message) => Some(message.clone()),
            Action::Close => None,
        })
        .collect()
}

fn recv(session: &mut ArenaSession, message: ServerMessage) -> Vec<ClientMessage> {
    sent(&session.handle(at(1.0), Event::Message(message)))
}

fn json(message: &ClientMessage) -> serde_json::Value {
    serde_json::from_str(&message.to_json().unwrap()).unwrap()
}

fn reserved(id: &str) -> ServerMessage {
    ServerMessage::MatchReserved(MatchReserved {
        match_id: id.into(),
        ruleset_profile: "oraja".into(),
        ln_mode: "LN".into(),
        finish_current_song: false,
        reservation_timeout: Some(30.0),
    })
}

fn chart(id: Option<&str>, md5: &str) -> ServerMessage {
    ServerMessage::Chart(ChartAnnounced {
        match_id: id.map(str::to_owned),
        md5: md5.into(),
        ..ChartAnnounced::default()
    })
}

fn in_nomination(session: &mut ArenaSession, id: &str) {
    session.handle(at(1.0), Event::RoomCreate);
    recv(session, reserved(id));
    recv(
        session,
        ServerMessage::FillStarted {
            deadline: Some(1_030.0),
            player_count: Some(1),
            max_players: Some(4),
        },
    );
    recv(session, ServerMessage::NominationStarted { deadline: Some(1_060.0) });
}

#[test]
fn creating_and_joining_a_room_send_room_entry() {
    let mut session = connected();
    let create = sent(&session.handle(at(1.0), Event::RoomCreate));
    assert_eq!(
        json(&create[0]),
        serde_json::json!({"type":"room_entry","match_mode":"private","series_format":"single"})
    );
    assert_eq!(session.panel().room, Some(RoomView::default()));

    let mut session = connected();
    let join =
        sent(&session.handle(
            at(1.0),
            Event::RoomJoin { code: " AB12 ".into(), password: Some(String::new()) },
        ));
    assert_eq!(
        json(&join[0]),
        serde_json::json!({"type":"room_entry","match_mode":"private","room_code":"AB12"})
    );
    // An empty code does nothing.
    assert!(
        sent(&session.handle(at(2.0), Event::RoomJoin { code: " ".into(), password: None }))
            .is_empty()
    );
}

#[test]
fn room_commands_need_a_connection() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    assert!(session.handle(at(0.0), Event::RoomCreate).is_empty());
    assert!(session.handle(at(0.0), Event::SetReady(true)).is_empty());
    assert_eq!(session.panel().room, None);
}

#[test]
fn room_status_sets_the_code_and_ready_goes_into_state() {
    let mut session = connected();
    session.handle(at(1.0), Event::RoomCreate);
    recv(&mut session, ServerMessage::RoomStatus { room_code: Some("ZZ99".into()) });
    assert_eq!(session.panel().room.unwrap().code.as_deref(), Some("ZZ99"));

    let messages = sent(&session.handle(at(2.0), Event::SetReady(true)));
    assert_eq!(json(&messages[0]), serde_json::json!({"type":"room_ready","ready":true}));
    assert_eq!(
        json(&messages[1]),
        serde_json::json!({"type":"state","state":"select","arena_enabled":true,"ready":true})
    );
    // `ready` is true only on the select screen.
    let messages = sent(&session.handle(at(3.0), Event::SetScreen(ScreenState::Decide)));
    assert_eq!(json(&messages[0])["ready"], false);
}

#[test]
fn ready_needs_a_room() {
    let mut session = connected();
    assert!(session.handle(at(1.0), Event::SetReady(true)).is_empty());
}

#[test]
fn the_flow_up_to_the_chosen_chart() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    let view = session.panel().current_match.unwrap();
    assert_eq!(
        (view.stage, view.player_count, view.max_players),
        (MatchStage::Nominating, Some(1), Some(4))
    );
    assert_eq!(view.deadline, Some(1_060.0));

    let messages = sent(&session.handle(at(2.0), Event::Nominate(MD5.to_uppercase())));
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"chart_nominate","match_id":"m1","chart_hash":MD5})
    );
    recv(&mut session, ServerMessage::NominationAccepted);
    assert!(session.panel().current_match.unwrap().nominated);
    // Only one nomination per round.
    assert!(sent(&session.handle(at(3.0), Event::Nominate(MD5.into()))).is_empty());
    assert!(sent(&session.handle(at(3.0), Event::SkipNomination)).is_empty());

    recv(&mut session, ServerMessage::NominationsRevealed);
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Revealed);

    recv(&mut session, chart(Some("m1"), MD5));
    let panel = session.panel();
    assert_eq!(panel.current_match.unwrap().stage, MatchStage::ChartChosen);
    assert_eq!(
        panel.pending_chart_check,
        Some(ChartCheckRequest { match_id: "m1".into(), md5: MD5.into() })
    );

    let messages = sent(&session.handle(
        at(4.0),
        Event::ChartChecked {
            match_id: "m1".into(),
            md5: MD5.into(),
            available: true,
            totalnotes: 900,
            play_mode: 7,
        },
    ));
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"chart_check","match_id":"m1","chart_hash":MD5,
            "available":true,"totalnotes":900,"play_mode":7})
    );
    assert_eq!(session.panel().pending_chart_check, None);
}

#[test]
fn skipping_sends_the_skip_message() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    let messages = sent(&session.handle(at(2.0), Event::SkipNomination));
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"chart_nomination_skip","match_id":"m1"})
    );
}

#[test]
fn nominations_are_refused_outside_the_nomination_stage_or_for_bad_hashes() {
    let mut session = connected();
    // No match.
    assert!(sent(&session.handle(at(1.0), Event::Nominate(MD5.into()))).is_empty());
    session.handle(at(1.0), Event::RoomCreate);
    recv(&mut session, reserved("m1"));
    // Reserved but not yet nominating.
    assert!(sent(&session.handle(at(2.0), Event::Nominate(MD5.into()))).is_empty());
    recv(&mut session, ServerMessage::NominationStarted { deadline: None });
    assert!(sent(&session.handle(at(3.0), Event::Nominate("xyz".into()))).is_empty());
    assert!(sent(&session.handle(at(3.0), Event::Nominate(MD5[..31].into()))).is_empty());
    assert_eq!(sent(&session.handle(at(3.0), Event::Nominate(MD5.into()))).len(), 1);
}

#[test]
fn a_stale_chart_check_answer_is_dropped() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    recv(&mut session, chart(None, MD5));
    let other = "f".repeat(32);
    // The candidate was re-rolled before the answer came back.
    recv(&mut session, chart(None, &other));
    let stale = Event::ChartChecked {
        match_id: "m1".into(),
        md5: MD5.into(),
        available: true,
        totalnotes: 1,
        play_mode: 7,
    };
    assert!(sent(&session.handle(at(2.0), stale)).is_empty());
    // An answer for another match is dropped too.
    let wrong_match = Event::ChartChecked {
        match_id: "m0".into(),
        md5: other.clone(),
        available: true,
        totalnotes: 1,
        play_mode: 7,
    };
    assert!(sent(&session.handle(at(2.0), wrong_match)).is_empty());
    let current = Event::ChartChecked {
        match_id: "m1".into(),
        md5: other,
        available: false,
        totalnotes: 0,
        play_mode: 0,
    };
    assert_eq!(sent(&session.handle(at(2.0), current)).len(), 1);
}

#[test]
fn a_chart_for_another_match_is_ignored() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    recv(&mut session, chart(Some("other"), MD5));
    assert_eq!(session.panel().pending_chart_check, None);
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Nominating);
}

#[test]
fn cancel_and_release_clear_the_match_and_the_ready_flag() {
    for end in [
        (
            ServerMessage::MatchCancelled { reason: "load_timeout".into() },
            MatchEnd::Cancelled("load_timeout".into()),
        ),
        (ServerMessage::MatchReleased, MatchEnd::Released),
    ] {
        let mut session = connected();
        in_nomination(&mut session, "m1");
        session.handle(at(1.5), Event::SetReady(true));
        recv(&mut session, chart(None, MD5));
        let messages = recv(&mut session, end.0);
        let panel = session.panel();
        assert_eq!(panel.current_match, None);
        assert_eq!(panel.pending_chart_check, None);
        assert_eq!(panel.match_end, Some(end.1));
        assert!(!panel.room.unwrap().ready);
        // The room stays; `state` reports ready=false again.
        assert_eq!(json(&messages[0])["ready"], false);
    }
}

#[test]
fn a_new_match_clears_the_previous_end_notice() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    recv(&mut session, ServerMessage::MatchReleased);
    recv(&mut session, reserved("m2"));
    assert_eq!(session.panel().match_end, None);
    assert_eq!(session.panel().current_match.unwrap().match_id, "m2");
}

#[test]
fn reconnecting_without_an_active_match_drops_the_local_match() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    session.handle(at(2.0), Event::SocketClosed);
    assert_eq!(session.panel().current_match, None);
    session.handle(at(8.0), Event::Tick);
    session.handle(at(8.0), Event::SocketOpened);
    session.handle(at(8.0), Event::Message(hello_ok("")));
    assert_eq!(session.panel().current_match, None);
    // The server resends the match it still holds.
    recv(&mut session, reserved("m1"));
    assert_eq!(session.panel().current_match.unwrap().match_id, "m1");
}

#[test]
fn replaced_and_refusals_forget_the_room() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    recv(&mut session, ServerMessage::Replaced);
    assert_eq!(session.panel().room, None);
    assert_eq!(session.panel().current_match, None);
}

#[test]
fn the_selected_chart_is_kept_for_the_panel() {
    let mut session = connected();
    session.handle(at(1.0), Event::SetSelectedChart(Some(MD5.into())));
    assert_eq!(session.panel().selected_chart_md5.as_deref(), Some(MD5));
    session.handle(at(2.0), Event::SetSelectedChart(None));
    assert_eq!(session.panel().selected_chart_md5, None);
}

#[test]
fn a_room_cannot_be_entered_during_a_match() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    assert!(sent(&session.handle(at(2.0), Event::RoomCreate)).is_empty());
}

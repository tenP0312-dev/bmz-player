use super::*;
use crate::arena::protocol::{
    ARENA_CLIENT_VERSION, CLIENT_FLAVOR, HelloOk, PROTOCOL_VERSION, RulesetProfile, passmd5,
};

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

fn hello_ok(server_time: f64) -> ServerMessage {
    ServerMessage::HelloOk(HelloOk {
        session_id: "s1".into(),
        server_protocol: Some(8),
        heartbeat_seconds: Some(10.0),
        active_match_id: String::new(),
        server_time: Some(server_time),
    })
}

fn error(code: &str) -> ServerMessage {
    ServerMessage::Error { code: code.into(), message: format!("{code} text") }
}

fn connected(screen: ScreenState) -> ArenaSession {
    let mut session = ArenaSession::new(hello(), screen);
    session.handle(at(0.0), Event::SocketOpened);
    session.handle(at(0.0), Event::Message(hello_ok(1_000.0)));
    session
}

fn sent(actions: &[Action]) -> Vec<&ClientMessage> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Send(message) => Some(message),
            Action::Close => None,
        })
        .collect()
}

#[test]
fn hello_is_the_first_message_and_hello_ok_connects() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    assert_eq!(session.phase(), Phase::Connecting);
    assert_eq!(session.panel().status, ConnectionStatus::Connecting);

    let actions = session.handle(at(0.0), Event::SocketOpened);
    assert_eq!(actions, vec![Action::Send(ClientMessage::Hello(hello()))]);
    assert_eq!(session.phase(), Phase::Authenticating);

    let actions = session.handle(at(0.1), Event::Message(hello_ok(1_000.1)));
    assert_eq!(session.phase(), Phase::Connected);
    // `state` first, then the first ping.
    assert_eq!(
        actions,
        vec![
            Action::Send(ClientMessage::State {
                state: ScreenState::Select,
                arena_enabled: true,
                ready: false
            }),
            Action::Send(ClientMessage::Ping { client_time: 1_000.1 }),
        ]
    );
    let panel = session.panel();
    assert_eq!(panel.status, ConnectionStatus::Connected);
    assert_eq!(panel.session_id.as_deref(), Some("s1"));
    assert_eq!(panel.active_match_id, None);
}

#[test]
fn ping_is_sent_every_five_seconds() {
    let mut session = connected(ScreenState::Select);
    assert!(session.handle(at(4.9), Event::Tick).is_empty());
    let actions = session.handle(at(5.0), Event::Tick);
    assert_eq!(sent(&actions), vec![&ClientMessage::Ping { client_time: 1_005.0 }]);
    assert!(session.handle(at(9.9), Event::Tick).is_empty());
    assert_eq!(sent(&session.handle(at(10.0), Event::Tick)).len(), 1);
}

#[test]
fn pong_sets_the_clock_from_the_round_trip_midpoint() {
    let mut session = connected(ScreenState::Select);
    session.handle(at(5.0), Event::Tick); // ping, client_time 1005.0
    // Reply after 0.2 s; the server stamped it 50.0 s ahead of the midpoint.
    let reply =
        ServerMessage::Pong { server_time: Some(1_005.1 + 50.0), client_time: Some(1_005.0) };
    session.handle(at(5.2), Event::Message(reply));
    let panel = session.panel();
    assert!((panel.round_trip_secs.unwrap() - 0.2).abs() < 1e-9);
    assert!((panel.clock_offset_secs.unwrap() - 50.0).abs() < 1e-6);
    let server_now = session.server_time(2_000.0).unwrap();
    assert!((server_now - 2_050.0).abs() < 1e-6);
}

#[test]
fn hello_ok_server_time_gives_a_first_clock_estimate() {
    let session = connected(ScreenState::Select);
    assert_eq!(session.panel().clock_offset_secs, Some(0.0));
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    session.handle(at(0.0), Event::SocketOpened);
    session.handle(at(0.0), Event::Message(hello_ok(1_030.0)));
    assert!((session.panel().clock_offset_secs.unwrap() - 30.0).abs() < 1e-9);
}

#[test]
fn pong_for_an_unknown_ping_is_ignored() {
    let mut session = connected(ScreenState::Select);
    let before = session.panel();
    session.handle(
        at(1.0),
        Event::Message(ServerMessage::Pong { server_time: Some(9.0), client_time: Some(123.0) }),
    );
    assert_eq!(session.panel(), before);
}

#[test]
fn idle_for_twenty_seconds_closes_and_schedules_a_reconnect() {
    let mut session = connected(ScreenState::Select);
    // Traffic keeps the connection alive.
    session.handle(at(15.0), Event::Message(ServerMessage::ArenaStatus(Default::default())));
    assert!(sent(&session.handle(at(34.0), Event::Tick)).len() <= 1);
    assert_eq!(session.phase(), Phase::Connected);

    let actions = session.handle(at(35.0), Event::Tick);
    assert_eq!(actions, vec![Action::Close]);
    assert_eq!(session.phase(), Phase::WaitingToReconnect);
    let panel = session.panel();
    assert_eq!(panel.status, ConnectionStatus::Reconnecting);
    assert_eq!(panel.last_disconnect, Some(DisconnectReason::IdleTimeout));
    assert_eq!(panel.retry_at_unix, Some(1_040.0));
    // The socket's own close notification afterwards changes nothing.
    session.handle(at(35.1), Event::SocketClosed);
    assert_eq!(session.phase(), Phase::WaitingToReconnect);
}

#[test]
fn a_silent_server_after_connect_is_also_cut_off() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    session.handle(at(0.0), Event::SocketOpened);
    assert!(session.handle(at(19.9), Event::Tick).is_empty());
    assert_eq!(session.handle(at(20.0), Event::Tick), vec![Action::Close]);
    assert_eq!(session.phase(), Phase::WaitingToReconnect);
}

#[test]
fn lost_connection_retries_after_five_seconds_then_slows_after_the_grace_period() {
    let mut session = connected(ScreenState::Select);
    session.handle(at(10.0), Event::SocketClosed);
    assert_eq!(session.phase(), Phase::WaitingToReconnect);
    assert_eq!(session.panel().retry_at_unix, Some(1_015.0));

    session.handle(at(14.9), Event::Tick);
    assert_eq!(session.phase(), Phase::WaitingToReconnect);
    session.handle(at(15.0), Event::Tick);
    assert_eq!(session.phase(), Phase::Connecting);

    // Attempts fail: 5 s apart while the 30 s grace lasts...
    session.handle(at(15.5), Event::SocketClosed);
    assert_eq!(session.panel().retry_at_unix, Some(1_020.5));
    session.handle(at(20.5), Event::Tick);
    session.handle(at(41.0), Event::SocketClosed);
    // ...and 30 s apart after it (first disconnect was at 10 s).
    assert_eq!(session.panel().retry_at_unix, Some(1_071.0));
}

#[test]
fn reconnect_resends_hello_then_state_and_restores_the_grace_clock() {
    let mut session = connected(ScreenState::Result);
    session.handle(at(10.0), Event::SocketClosed);
    session.handle(at(15.0), Event::Tick);
    let actions = session.handle(at(15.2), Event::SocketOpened);
    assert_eq!(actions, vec![Action::Send(ClientMessage::Hello(hello()))]);
    let actions = session.handle(at(15.3), Event::Message(hello_ok(1_015.3)));
    let messages = sent(&actions);
    assert_eq!(
        messages[0],
        &ClientMessage::State { state: ScreenState::Result, arena_enabled: true, ready: false }
    );
    assert!(matches!(messages[1], ClientMessage::Ping { .. }));
    // The next outage starts a new grace period.
    session.handle(at(100.0), Event::SocketClosed);
    assert_eq!(session.panel().retry_at_unix, Some(1_105.0));
}

#[test]
fn state_is_sent_on_screen_changes_only_when_connected_and_changed() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    // Not connected: remembered, not sent.
    assert!(session.handle(at(0.0), Event::SetScreen(ScreenState::Decide)).is_empty());
    session.handle(at(0.0), Event::SocketOpened);
    let actions = session.handle(at(0.1), Event::Message(hello_ok(1_000.1)));
    assert_eq!(
        sent(&actions)[0],
        &ClientMessage::State { state: ScreenState::Decide, arena_enabled: true, ready: false }
    );
    let actions = session.handle(at(1.0), Event::SetScreen(ScreenState::Play));
    assert_eq!(
        sent(&actions),
        vec![&ClientMessage::State { state: ScreenState::Play, arena_enabled: true, ready: false }]
    );
    assert!(session.handle(at(1.1), Event::SetScreen(ScreenState::Play)).is_empty());
}

#[test]
fn refusals_stop_reconnecting_and_keep_the_reason() {
    let cases = [
        ("protocol_mismatch", RefusalKind::UpdateRequired),
        ("unsupported_client", RefusalKind::UpdateRequired),
        ("unsupported_build", RefusalKind::UpdateRequired),
        ("unsupported_client_flavor", RefusalKind::UpdateRequired),
        ("authentication_failed", RefusalKind::AuthenticationFailed),
        ("client_flavor_conflict", RefusalKind::FlavorConflict),
    ];
    for (code, kind) in cases {
        let mut session = ArenaSession::new(hello(), ScreenState::Select);
        session.handle(at(0.0), Event::SocketOpened);
        let actions = session.handle(at(0.1), Event::Message(error(code)));
        assert_eq!(actions, vec![Action::Close], "{code}");
        assert_eq!(session.panel().status, ConnectionStatus::Refused(kind), "{code}");
        assert_eq!(session.panel().notice.unwrap().code, code);
        // The server's disconnect and any amount of time never restart it.
        session.handle(at(0.2), Event::SocketClosed);
        session.handle(at(600.0), Event::Tick);
        assert_eq!(session.phase(), Phase::Refused(kind), "{code}");
    }
}

#[test]
fn other_errors_are_shown_but_keep_the_connection() {
    let mut session = connected(ScreenState::Select);
    let actions = session.handle(at(1.0), Event::Message(error("room_entry_failed")));
    assert!(actions.is_empty());
    assert_eq!(session.phase(), Phase::Connected);
    assert_eq!(session.panel().notice.unwrap().code, "room_entry_failed");
}

#[test]
fn replaced_does_not_reconnect_until_asked() {
    let mut session = connected(ScreenState::Select);
    assert_eq!(
        session.handle(at(2.0), Event::Message(ServerMessage::Replaced)),
        vec![Action::Close]
    );
    assert_eq!(session.panel().status, ConnectionStatus::Replaced);
    session.handle(at(2.1), Event::SocketClosed);
    session.handle(at(300.0), Event::Tick);
    assert_eq!(session.phase(), Phase::Replaced);

    // The user can reconnect by hand.
    session.handle(at(301.0), Event::ManualReconnect);
    assert_eq!(session.phase(), Phase::Connecting);
    assert_eq!(session.panel().notice, None);
}

#[test]
fn manual_reconnect_does_nothing_while_connected() {
    let mut session = connected(ScreenState::Select);
    session.handle(at(1.0), Event::ManualReconnect);
    assert_eq!(session.phase(), Phase::Connected);
}

#[test]
fn unknown_messages_are_counted_and_ignored() {
    let mut session = connected(ScreenState::Select);
    let actions = session.handle(at(1.0), Event::Message(ServerMessage::Unknown("chart".into())));
    assert!(actions.is_empty());
    assert_eq!(session.unknown_message_count(), 1);
    assert_eq!(session.phase(), Phase::Connected);
}

#[test]
fn active_match_is_reported_but_not_acted_on() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    session.handle(at(0.0), Event::SocketOpened);
    session.handle(
        at(0.1),
        Event::Message(ServerMessage::HelloOk(HelloOk {
            session_id: "s".into(),
            server_protocol: Some(8),
            heartbeat_seconds: None,
            active_match_id: "m-1".into(),
            server_time: None,
        })),
    );
    assert_eq!(session.panel().active_match_id.as_deref(), Some("m-1"));
    assert_eq!(session.panel().clock_offset_secs, None);
}

#[test]
fn messages_before_the_socket_opens_are_ignored() {
    let mut session = ArenaSession::new(hello(), ScreenState::Select);
    assert!(session.handle(at(0.0), Event::Message(ServerMessage::Replaced)).is_empty());
    assert_eq!(session.phase(), Phase::Connecting);
}

#[test]
fn default_timing_matches_the_specification() {
    let timing = Timing::default();
    assert_eq!(timing.ping_interval, Duration::from_secs(5));
    assert_eq!(timing.idle_timeout, Duration::from_secs(20));
    assert_eq!(timing.reconnect_delay, Duration::from_secs(5));
    assert_eq!(timing.reconnect_grace, Duration::from_secs(30));
}

#[test]
fn arena_status_player_is_kept_until_the_connection_drops() {
    use crate::arena::protocol::PlayerStatus;
    let mut session = connected(ScreenState::Select);
    assert_eq!(session.panel().player, None);
    let player = PlayerStatus { rating: Some(1500), matches_played: Some(12) };
    session.handle(at(1.0), Event::Message(ServerMessage::ArenaStatus(player)));
    assert_eq!(session.panel().player, Some(player));
    session.handle(at(2.0), Event::SocketClosed);
    assert_eq!(session.panel().player, None);
}

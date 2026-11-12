//! The connection thread against a fake Arena server that lives inside the
//! test (loopback only; no network, no assets).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

use super::*;
use crate::arena::protocol::{
    ARENA_CLIENT_VERSION, CLIENT_FLAVOR, PROTOCOL_VERSION, RulesetProfile, passmd5,
};
use crate::arena::session::{DisconnectReason, RefusalKind};

type Server = WebSocketStream<TcpStream>;

const WAIT: Duration = Duration::from_secs(5);

/// Short pings and retries; a long idle limit so a slow machine never trips it
/// by accident (the idle test uses [`idle_timing`]).
fn timing() -> Timing {
    Timing {
        ping_interval: Duration::from_millis(200),
        idle_timeout: Duration::from_secs(30),
        reconnect_grace: Duration::from_secs(30),
        reconnect_delay: Duration::from_millis(300),
        reconnect_delay_after_grace: Duration::from_millis(300),
    }
}

fn idle_timing() -> Timing {
    Timing { idle_timeout: Duration::from_secs(1), ..timing() }
}

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        player_id: 4242,
        passmd5: passmd5("secret-token"),
        client_version: ARENA_CLIENT_VERSION.into(),
        build_hash: "b".repeat(40),
        body_version: "0.5.1".into(),
        client_flavor: CLIENT_FLAVOR.into(),
        ruleset_profile: RulesetProfile::Lr2,
        arena_enabled: true,
    }
}

async fn bind() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/new/arena/ws/client", listener.local_addr().unwrap());
    (listener, url)
}

async fn accept(listener: &TcpListener) -> Server {
    let (stream, _) = tokio::time::timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    accept_async(stream).await.unwrap()
}

async fn recv(server: &mut Server) -> Value {
    loop {
        let message = tokio::time::timeout(WAIT, server.next())
            .await
            .expect("timed out waiting for a client message")
            .expect("client closed")
            .expect("socket error");
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn reply(server: &mut Server, value: Value) {
    server.send(Message::Text(value.to_string())).await.unwrap();
}

fn now_unix() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
}

async fn accept_and_welcome(listener: &TcpListener) -> (Server, Value) {
    let mut server = accept(listener).await;
    let hello = recv(&mut server).await;
    reply(
        &mut server,
        json!({"type":"hello_ok","protocol":8,"server_protocol":8,"session_id":"sess","client_flavor":"bmz-player",
               "ruleset_profile":"lr2","heartbeat_seconds":10,"active_match_id":"","server_time":now_unix()}),
    )
    .await;
    (server, hello)
}

async fn wait_for(
    controller: &ArenaController,
    what: &str,
    done: impl Fn(&ArenaPanelState) -> bool,
) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let panel = controller.panel();
        if done(&panel) {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting for {what}: {panel:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn assert_no_connection(listener: &TcpListener, within: Duration) {
    assert!(
        tokio::time::timeout(within, listener.accept()).await.is_err(),
        "the client connected although it must not"
    );
}

#[tokio::test]
async fn hello_hello_ok_state_ping_and_pong() {
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());

    let (mut server, hello_json) = accept_and_welcome(&listener).await;
    assert_eq!(hello_json["type"], "hello");
    assert_eq!(hello_json["protocol"], 8);
    assert_eq!(hello_json["player_id"], 4242);
    assert_eq!(hello_json["client_flavor"], "bmz-player");
    assert_eq!(hello_json["passmd5"], passmd5("secret-token"));
    assert!(!hello_json.to_string().contains("secret-token"), "the raw token must never be sent");

    wait_for(&controller, "connected", |p| p.status == ConnectionStatus::Connected).await;
    assert_eq!(controller.panel().session_id.as_deref(), Some("sess"));

    // `state` comes straight after hello_ok, then the first ping.
    assert_eq!(
        recv(&mut server).await,
        json!({"type":"state","state":"select","arena_enabled":true,"ready":false})
    );
    let ping = recv(&mut server).await;
    assert_eq!(ping["type"], "ping");
    let client_time = ping["client_time"].as_f64().unwrap();
    reply(
        &mut server,
        json!({"type":"pong","client_time":client_time,"server_time":client_time + 100.0}),
    )
    .await;
    wait_for(&controller, "clock sync", |p| p.round_trip_secs.is_some()).await;
    let offset = controller.panel().clock_offset_secs.unwrap();
    assert!((offset - 100.0).abs() < 1.0, "offset {offset}");

    // Pings keep coming.
    assert_eq!(recv(&mut server).await["type"], "ping");

    // A screen change is announced.
    controller.set_screen(ScreenState::Play);
    loop {
        let message = recv(&mut server).await;
        if message["type"] == "state" {
            assert_eq!(message["state"], "play");
            break;
        }
    }
    drop(controller);
}

#[tokio::test]
async fn reconnects_after_the_server_drops_the_connection() {
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Result, timing());
    let (server, _) = accept_and_welcome(&listener).await;
    wait_for(&controller, "connected", |p| p.status == ConnectionStatus::Connected).await;
    drop(server);
    wait_for(&controller, "reconnecting", |p| p.status == ConnectionStatus::Reconnecting).await;
    assert_eq!(controller.panel().last_disconnect, Some(DisconnectReason::ConnectionLost));

    let (mut server, hello_json) = accept_and_welcome(&listener).await;
    assert_eq!(hello_json["type"], "hello");
    wait_for(&controller, "connected again", |p| p.status == ConnectionStatus::Connected).await;
    // The screen survives the reconnect.
    assert_eq!(recv(&mut server).await["state"], "result");
    drop(controller);
}

#[tokio::test]
async fn keeps_retrying_while_the_server_is_unreachable() {
    // Nothing listens on this port.
    let (listener, url) = bind().await;
    drop(listener);
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
    wait_for(&controller, "reconnecting", |p| p.status == ConnectionStatus::Reconnecting).await;
    assert!(controller.panel().retry_at_unix.is_some());
    drop(controller);
}

#[tokio::test]
async fn an_unresponsive_server_is_cut_off_and_reconnected() {
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, idle_timing());
    // Say hello_ok, then stay silent forever (keep the socket open, read nothing).
    let (_silent, _) = accept_and_welcome(&listener).await;
    wait_for(&controller, "idle cut-off", |p| {
        p.last_disconnect == Some(DisconnectReason::IdleTimeout)
    })
    .await;
    let (_second, hello_json) = accept_and_welcome(&listener).await;
    assert_eq!(hello_json["type"], "hello");
    drop(controller);
}

#[tokio::test]
async fn a_refusal_is_shown_and_never_retried() {
    for (code, kind) in [
        ("authentication_failed", RefusalKind::AuthenticationFailed),
        ("unsupported_client_flavor", RefusalKind::UpdateRequired),
        ("client_flavor_conflict", RefusalKind::FlavorConflict),
    ] {
        let (listener, url) = bind().await;
        let controller =
            ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
        let mut server = accept(&listener).await;
        assert_eq!(recv(&mut server).await["type"], "hello");
        reply(&mut server, json!({"type":"error","code":code,"message":"refused"})).await;
        drop(server);

        wait_for(&controller, "refusal", |p| p.status == ConnectionStatus::Refused(kind)).await;
        assert_eq!(controller.panel().notice.unwrap().code, code);
        assert_no_connection(&listener, Duration::from_millis(1500)).await;
        assert_eq!(controller.panel().status, ConnectionStatus::Refused(kind));
        drop(controller);
    }
}

#[tokio::test]
async fn replaced_is_not_retried_until_the_user_asks() {
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
    let (mut server, _) = accept_and_welcome(&listener).await;
    wait_for(&controller, "connected", |p| p.status == ConnectionStatus::Connected).await;
    reply(&mut server, json!({"type":"replaced"})).await;
    drop(server);

    wait_for(&controller, "replaced", |p| p.status == ConnectionStatus::Replaced).await;
    assert_no_connection(&listener, Duration::from_millis(1500)).await;

    controller.reconnect();
    let (_server, hello_json) = accept_and_welcome(&listener).await;
    assert_eq!(hello_json["type"], "hello");
    wait_for(&controller, "connected again", |p| p.status == ConnectionStatus::Connected).await;
    drop(controller);
}

#[tokio::test]
async fn unknown_and_malformed_messages_do_not_break_the_connection() {
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
    let (mut server, _) = accept_and_welcome(&listener).await;
    reply(&mut server, json!({"type":"nomination_started","deadline":1})).await;
    server.send(Message::Text("not json".into())).await.unwrap();
    reply(&mut server, json!({"type":"room_entry_failed_future_message"})).await;
    reply(&mut server, json!({"type":"error","code":"room_entry_failed","message":"no room"}))
        .await;
    wait_for(&controller, "notice", |p| p.notice.is_some()).await;
    assert_eq!(controller.panel().notice.unwrap().code, "room_entry_failed");
    assert_eq!(controller.panel().status, ConnectionStatus::Connected);
    // Still alive: pings continue.
    loop {
        if recv(&mut server).await["type"] == "ping" {
            break;
        }
    }
    drop(controller);
}

#[test]
fn an_inactive_controller_reports_why_and_ignores_commands() {
    let controller = ArenaController::inactive(ConnectionStatus::NoCredentials);
    assert_eq!(controller.panel().status, ConnectionStatus::NoCredentials);
    controller.set_screen(ScreenState::Play);
    controller.reconnect();
    controller.shutdown();
}

/// Reads client messages until one of `kind` arrives (pings and `state` are skipped).
async fn recv_kind(server: &mut Server, kind: &str) -> Value {
    loop {
        let message = recv(server).await;
        if message["type"] == kind {
            return message;
        }
    }
}

#[tokio::test]
async fn room_nomination_and_chart_check_over_the_wire() {
    let md5 = "0123456789abcdef0123456789abcdef";
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
    let (mut server, _) = accept_and_welcome(&listener).await;
    wait_for(&controller, "connected", |p| p.status == ConnectionStatus::Connected).await;

    controller.handle().create_room();
    let entry = recv_kind(&mut server, "room_entry").await;
    assert_eq!(entry["match_mode"], "private");
    assert_eq!(entry["room_name"], serde_json::Value::Null);
    assert_eq!(entry["ln_mode"], "LN");
    assert_eq!(entry["allowed_play_modes"], json!([7]));
    assert_eq!(entry["chart_scope"], "free");

    reply(
        &mut server,
        json!({"type":"room_status","room_code":"AB12","rules":{"room_host_id":4242},
               "players":[{"player_id":4242,"name":"me","host":true,"participating":true,
                           "pending":false,"ready":false}]}),
    )
    .await;
    wait_for(&controller, "room players", |p| {
        p.room.as_ref().is_some_and(|room| room.players.len() == 1 && room.host_id == Some(4242))
    })
    .await;
    controller.handle().set_ready(true);
    assert_eq!(recv_kind(&mut server, "room_ready").await["ready"], true);
    assert_eq!(recv_kind(&mut server, "state").await["ready"], true);

    reply(
        &mut server,
        json!({"type":"match_reserved","match_id":"m1","ruleset_profile":"lr2","ln_mode":"LN",
               "finish_current_song":false,"reservation_timeout":30}),
    )
    .await;
    reply(
        &mut server,
        json!({"type":"nomination_started","match_id":"m1","deadline":now_unix() + 30.0,
               "can_nominate":true,"required_count":1,"submitted_count":0,"players":[]}),
    )
    .await;
    wait_for(&controller, "nominating", |p| {
        p.current_match
            .as_ref()
            .is_some_and(|m| m.stage == crate::arena::session::MatchStage::Nominating)
    })
    .await;

    controller.handle().nominate(md5.into());
    let nominate = recv_kind(&mut server, "chart_nominate").await;
    assert_eq!(nominate["match_id"], "m1");
    assert_eq!(nominate["chart_hash"], md5);

    reply(&mut server, json!({"type":"nomination_accepted"})).await;
    reply(&mut server, json!({"type":"chart","match_id":"m1","chart":{"md5":md5,"title":"t"}}))
        .await;
    wait_for(&controller, "chart check request", |p| p.pending_chart_check.is_some()).await;
    controller.handle().report_chart_check("m1".into(), md5.into(), true, 800, 7);
    let check = recv_kind(&mut server, "chart_check").await;
    assert_eq!(
        check,
        json!({"type":"chart_check","match_id":"m1","chart_hash":md5,"available":true,
               "totalnotes":800,"play_mode":7})
    );

    reply(&mut server, json!({"type":"match_cancelled","reason":"load_timeout"})).await;
    wait_for(&controller, "match cleared", |p| p.current_match.is_none() && p.match_end.is_some())
        .await;

    controller.handle().leave_room();
    assert_eq!(recv_kind(&mut server, "queue_cancel").await, json!({"type":"queue_cancel"}));
    wait_for(&controller, "room left", |p| p.room.is_none()).await;
    controller.shutdown();
}

#[tokio::test]
async fn option_prepare_play_ready_and_start_over_the_wire() {
    let md5 = "0123456789abcdef0123456789abcdef";
    let (listener, url) = bind().await;
    let controller =
        ArenaController::spawn_with_timing(url, hello(), ScreenState::Select, timing());
    let (mut server, _) = accept_and_welcome(&listener).await;
    wait_for(&controller, "connected", |p| p.status == ConnectionStatus::Connected).await;

    controller.handle().create_room();
    recv_kind(&mut server, "room_entry").await;
    reply(
        &mut server,
        json!({"type":"match_reserved","match_id":"m1","ruleset_profile":"lr2","ln_mode":"LN",
               "finish_current_song":false,"reservation_timeout":30}),
    )
    .await;
    reply(&mut server, json!({"type":"chart","match_id":"m1","chart":{"md5":md5,"title":"t"}}))
        .await;
    reply(
        &mut server,
        json!({"type":"option_select","match_id":"m1","deadline":now_unix() + 30.0,"ready":false,
               "ready_count":0,"player_count":2,"play_mode":7,"ln_mode":"LN"}),
    )
    .await;
    wait_for(&controller, "option phase", |p| {
        p.current_match.as_ref().is_some_and(|m| m.option.is_some())
    })
    .await;

    controller.handle().set_option_choice(103);
    controller.handle().option_ready();
    let option = recv_kind(&mut server, "option_ready").await;
    assert_eq!(
        option,
        json!({"type":"option_ready","match_id":"m1","play_option":103,"play_mode":7,
               "ln_mode":"LN"})
    );

    reply(
        &mut server,
        json!({"type":"prepare","match_id":"m1","chart":{"md5":md5,"totalnotes":900},
               "load_deadline":now_unix() + 120.0,"ln_mode":"LN","random_seed":42,
               "play_option":103,"play_mode":7,"rules":{"ruleset_profile":"lr2"}}),
    )
    .await;
    wait_for(&controller, "prepare", |p| p.pending_prepare.is_some()).await;
    let prepare = controller.panel().pending_prepare.unwrap();
    assert_eq!((prepare.play_option, prepare.random_seed), (103, Some(42)));
    assert_eq!(prepare.ruleset_profile, "lr2");

    controller.handle().prepare_taken("m1".into());
    wait_for(&controller, "prepare taken", |p| p.pending_prepare.is_none()).await;
    controller.handle().play_loaded("m1".into(), md5.into(), 910);
    let ready = recv_kind(&mut server, "play_ready").await;
    assert_eq!(
        ready,
        json!({"type":"play_ready","match_id":"m1","chart_hash":md5,"ln_mode":"LN",
               "totalnotes":910})
    );

    let start_at = now_unix() + 5.0;
    reply(&mut server, json!({"type":"start","match_id":"m1","start_at":start_at,"ln_mode":"LN"}))
        .await;
    wait_for(&controller, "start gate", |p| p.start_gate.is_some()).await;
    assert_eq!(controller.panel().start_gate.unwrap().start_at, start_at);
    controller.shutdown();
}

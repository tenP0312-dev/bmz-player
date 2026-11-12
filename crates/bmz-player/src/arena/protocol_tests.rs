use super::*;
use serde_json::json;

fn sample_hello() -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        player_id: 1234,
        passmd5: passmd5("token"),
        client_version: ARENA_CLIENT_VERSION.to_owned(),
        build_hash: "0".repeat(40),
        body_version: "0.5.1".to_owned(),
        client_flavor: CLIENT_FLAVOR.to_owned(),
        ruleset_profile: RulesetProfile::Lr2,
        arena_enabled: true,
    }
}

#[test]
fn passmd5_is_lowercase_hex_md5_of_utf8() {
    assert_eq!(passmd5(""), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(passmd5("abc"), "900150983cd24fb0d6963f7d28e17f72");
    // Non-ASCII tokens hash their UTF-8 bytes.
    assert_eq!(passmd5("é"), format!("{:x}", md5::compute([0xc3, 0xa9])));
    assert_eq!(passmd5("abc").len(), 32);
}

#[test]
fn hello_json_has_the_documented_fields_only() {
    let value: Value =
        serde_json::from_str(&ClientMessage::Hello(sample_hello()).to_json().unwrap()).unwrap();
    let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "arena_enabled",
            "body_version",
            "build_hash",
            "client_flavor",
            "client_version",
            "passmd5",
            "player_id",
            "protocol",
            "ruleset_profile",
            "type"
        ]
    );
    assert_eq!(value["type"], "hello");
    assert_eq!(value["protocol"], 8);
    assert_eq!(value["player_id"], 1234);
    assert_eq!(value["client_flavor"], "bmz-player");
    assert_eq!(value["ruleset_profile"], "lr2");
    // B1 never claims server-side CPU support.
    assert!(value.get("server_cpu_v1").is_none());
}

#[test]
fn small_messages_serialize_as_specified() {
    let ping = ClientMessage::Ping { client_time: 12.5 }.to_json().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&ping).unwrap(),
        json!({"type":"ping","client_time":12.5})
    );
    let state =
        ClientMessage::State { state: ScreenState::Select, arena_enabled: true, ready: false }
            .to_json()
            .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&state).unwrap(),
        json!({"type":"state","state":"select","arena_enabled":true,"ready":false})
    );
    assert_eq!(
        serde_json::from_str::<Value>(&ClientMessage::ArenaStatus {}.to_json().unwrap()).unwrap(),
        json!({"type":"arena_status"})
    );
}

#[test]
fn every_screen_state_uses_the_wire_name() {
    let names = [
        (ScreenState::Select, "select"),
        (ScreenState::Decide, "decide"),
        (ScreenState::Play, "play"),
        (ScreenState::Result, "result"),
        (ScreenState::Course, "course"),
        (ScreenState::Practice, "practice"),
        (ScreenState::Autoplay, "autoplay"),
        (ScreenState::Replay, "replay"),
    ];
    for (state, name) in names {
        assert_eq!(serde_json::to_value(state).unwrap(), json!(name));
    }
}

#[test]
fn rule_mode_maps_to_the_ruleset_profile() {
    use bmz_gameplay::rule::RuleMode;
    assert_eq!(RulesetProfile::from_rule_mode(RuleMode::Lr2Oraja), Some(RulesetProfile::Lr2));
    assert_eq!(RulesetProfile::from_rule_mode(RuleMode::Beatoraja), Some(RulesetProfile::Oraja));
    assert_eq!(RulesetProfile::from_rule_mode(RuleMode::Dx), None);
}

#[test]
fn oversized_message_is_rejected() {
    let big = ClientMessage::Hello(Hello {
        client_version: "x".repeat(MAX_MESSAGE_BYTES),
        ..sample_hello()
    });
    assert!(big.to_json().is_err());
}

#[test]
fn hello_ok_and_pong_are_read() {
    let hello_ok = parse_server_message(
        r#"{"type":"hello_ok","protocol":8,"server_protocol":8,"session_id":"s1","client_flavor":"bmz-player",
            "ruleset_profile":"lr2","heartbeat_seconds":10,"active_match_id":"","server_time":1700000000.25}"#,
    )
    .unwrap();
    assert_eq!(
        hello_ok,
        ServerMessage::HelloOk(HelloOk {
            session_id: "s1".into(),
            server_protocol: Some(8),
            heartbeat_seconds: Some(10.0),
            active_match_id: String::new(),
            server_time: Some(1_700_000_000.25),
        })
    );
    assert_eq!(
        parse_server_message(r#"{"type":"pong","server_time":5.5,"client_time":5.0}"#).unwrap(),
        ServerMessage::Pong { server_time: Some(5.5), client_time: Some(5.0) }
    );
}

#[test]
fn error_replaced_and_status_are_read() {
    assert_eq!(
        parse_server_message(r#"{"type":"error","code":"authentication_failed","message":"no"}"#)
            .unwrap(),
        ServerMessage::Error { code: "authentication_failed".into(), message: "no".into() }
    );
    assert_eq!(parse_server_message(r#"{"type":"replaced"}"#).unwrap(), ServerMessage::Replaced);
    assert_eq!(
        parse_server_message(r#"{"type":"arena_status","queue":[],"extra":1}"#).unwrap(),
        ServerMessage::ArenaStatus(Default::default())
    );
}

#[test]
fn arena_status_reads_rating_and_matches_played_and_ignores_the_rest() {
    use super::PlayerStatus;
    let text = r#"{"type":"arena_status","player":{"rating":1523,"rating_exact":1522.6,
        "matches_played":42,"season":{"id":3,"name":"s"},"new_field":[1]},
        "waiting_players":[],"ranking":[],"public_rooms":[],"lobby_chat":[]}"#;
    assert_eq!(
        parse_server_message(text).unwrap(),
        ServerMessage::ArenaStatus(PlayerStatus { rating: Some(1523), matches_played: Some(42) })
    );
    // Mistyped or missing fields are ignored individually.
    let text = r#"{"type":"arena_status","player":{"rating":"high","matches_played":7}}"#;
    assert_eq!(
        parse_server_message(text).unwrap(),
        ServerMessage::ArenaStatus(PlayerStatus { rating: None, matches_played: Some(7) })
    );
    let text = r#"{"type":"arena_status","player":null}"#;
    assert_eq!(
        parse_server_message(text).unwrap(),
        ServerMessage::ArenaStatus(PlayerStatus::default())
    );
}

#[test]
fn unknown_types_and_fields_do_not_fail() {
    assert_eq!(
        parse_server_message(r#"{"type":"live","deadline":1}"#).unwrap(),
        ServerMessage::Unknown("live".into())
    );
    assert_eq!(parse_server_message("{}").unwrap(), ServerMessage::Unknown(String::new()));
    // Unknown extra fields on a known type are ignored.
    assert!(matches!(
        parse_server_message(r#"{"type":"pong","server_time":1,"future":{"a":1}}"#).unwrap(),
        ServerMessage::Pong { .. }
    ));
}

#[test]
fn invalid_json_is_an_error() {
    assert!(parse_server_message("not json").is_err());
    assert!(parse_server_message("[1,2]").is_err());
}

#[test]
fn match_setup_messages_are_read_with_only_the_named_fields() {
    use super::{ChartAnnounced, MatchReserved};
    assert_eq!(
        parse_server_message(
            r#"{"type":"match_reserved","match_id":"m1","ruleset_profile":"lr2","ln_mode":"LN",
            "finish_current_song":true,"rules":{"x":1},"reservation_timeout":30,"extra":0}"#
        )
        .unwrap(),
        ServerMessage::MatchReserved(MatchReserved {
            match_id: "m1".into(),
            ruleset_profile: "lr2".into(),
            ln_mode: "LN".into(),
            finish_current_song: true,
            reservation_timeout: Some(30.0),
        })
    );
    assert_eq!(
        parse_server_message(
            r#"{"type":"fill_started","deadline":1.5,"player_count":2,"max_players":4}"#
        )
        .unwrap(),
        ServerMessage::FillStarted {
            deadline: Some(1.5),
            player_count: Some(2),
            max_players: Some(4)
        }
    );
    assert_eq!(
        parse_server_message(r#"{"type":"nomination_started","deadline":9}"#).unwrap(),
        ServerMessage::NominationStarted { deadline: Some(9.0) }
    );
    assert_eq!(
        parse_server_message(r#"{"type":"nominations_revealed","candidates":[]}"#).unwrap(),
        ServerMessage::NominationsRevealed
    );
    assert_eq!(
        parse_server_message(
            r#"{"type":"chart","match_id":"m1","chart":{"md5":"ABCD","title":"t","artist":"a",
            "totalnotes":1200},"random_seed":5,"ln_mode":1,"load_deadline":3.5}"#
        )
        .unwrap(),
        ServerMessage::Chart(ChartAnnounced {
            match_id: Some("m1".into()),
            md5: "abcd".into(),
            title: "t".into(),
            artist: "a".into(),
            totalnotes: Some(1200),
            random_seed: Some(5),
            ln_mode: "1".into(),
            load_deadline: Some(3.5),
        })
    );
    assert_eq!(
        parse_server_message(r#"{"type":"match_cancelled","reason":"load_timeout"}"#).unwrap(),
        ServerMessage::MatchCancelled { reason: "load_timeout".into() }
    );
    assert_eq!(
        parse_server_message(r#"{"type":"match_released"}"#).unwrap(),
        ServerMessage::MatchReleased
    );
    assert_eq!(
        parse_server_message(r#"{"type":"room_status","room_code":"AB12","players":[]}"#).unwrap(),
        ServerMessage::RoomStatus { room_code: Some("AB12".into()) }
    );
}

#[test]
fn match_setup_messages_without_fields_do_not_fail() {
    assert_eq!(
        parse_server_message(r#"{"type":"chart"}"#).unwrap(),
        ServerMessage::Chart(Default::default())
    );
    assert_eq!(
        parse_server_message(r#"{"type":"room_status"}"#).unwrap(),
        ServerMessage::RoomStatus { room_code: None }
    );
}

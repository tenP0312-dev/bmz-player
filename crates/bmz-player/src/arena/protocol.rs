//! Arena protocol v8: message types and JSON conversion.
//!
//! One WebSocket text frame carries one JSON object. Only the B1 subset is
//! modelled (connect, keep-alive, `state`); every other message type is read as
//! [`ServerMessage::Unknown`] so a newer server never stops the client.

use serde::Serialize;
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 8;
pub const CLIENT_FLAVOR: &str = "bmz-player";
/// Path of the Arena WebSocket on the BMS-IR host.
pub const ARENA_WS_PATH: &str = "/new/arena/ws/client";
/// Largest frame the server accepts from a client.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Arena `client_version`. Raised whenever a change affects the Arena protocol.
pub const ARENA_CLIENT_VERSION: &str = "bmz-bmsir-0.1.0";

/// `hello.ruleset_profile` and the room/match rule family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RulesetProfile {
    Lr2,
    Oraja,
}

impl RulesetProfile {
    /// `Lr2Oraja` plays as `lr2`, `Beatoraja` as `oraja`. `Dx` has no Arena rule.
    pub fn from_rule_mode(rule_mode: bmz_gameplay::rule::RuleMode) -> Option<Self> {
        match rule_mode {
            bmz_gameplay::rule::RuleMode::Lr2Oraja => Some(Self::Lr2),
            bmz_gameplay::rule::RuleMode::Beatoraja => Some(Self::Oraja),
            bmz_gameplay::rule::RuleMode::Dx => None,
        }
    }
}

/// Values of `state.state`. Anything else is read as `unknown` by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenState {
    Select,
    Decide,
    Play,
    Result,
    Course,
    Practice,
    Autoplay,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hello {
    pub protocol: u32,
    pub player_id: u64,
    pub passmd5: String,
    pub client_version: String,
    pub build_hash: String,
    pub body_version: String,
    pub client_flavor: String,
    pub ruleset_profile: RulesetProfile,
    pub arena_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello(Hello),
    Ping { client_time: f64 },
    State { state: ScreenState, arena_enabled: bool, ready: bool },
    ArenaStatus {},
}

impl ClientMessage {
    /// JSON text for one frame. Fails only if the frame would exceed the server limit.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let text = serde_json::to_string(self)?;
        if text.len() > MAX_MESSAGE_BYTES {
            return Err(serde::ser::Error::custom("arena message exceeds 16 KiB"));
        }
        Ok(text)
    }
}

/// `passmd5`: lowercase hex MD5 of the game token's UTF-8 bytes (the same
/// conversion the BMS-IR HTTP API uses).
pub fn passmd5(game_token: &str) -> String {
    format!("{:x}", md5::compute(game_token.as_bytes()))
}

#[derive(Debug, Clone, PartialEq)]
pub struct HelloOk {
    pub session_id: String,
    pub server_protocol: Option<u32>,
    pub heartbeat_seconds: Option<f64>,
    /// Empty when no match is in progress.
    pub active_match_id: String,
    pub server_time: Option<f64>,
}

/// `arena_status.player`: the fields the panel shows. Missing or mistyped
/// fields stay `None`; unknown fields (`rating_exact`, `season`, ...) are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerStatus {
    /// Rounded rating (integer).
    pub rating: Option<i64>,
    pub matches_played: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    HelloOk(HelloOk),
    Pong {
        server_time: Option<f64>,
        client_time: Option<f64>,
    },
    Error {
        code: String,
        message: String,
    },
    Replaced,
    /// Lobby snapshot. Only the player's own rating is read; other fields
    /// (`waiting_players`, `ranking`, `public_rooms`, `lobby_chat`) are for later stages.
    ArenaStatus(PlayerStatus),
    /// Any other (or later-stage) message type.
    Unknown(String),
}

/// Reads one text frame. Invalid JSON, or a non-object, is an error; a missing or
/// unknown `type` is not (it becomes [`ServerMessage::Unknown`]).
pub fn parse_server_message(text: &str) -> Result<ServerMessage, serde_json::Error> {
    let value: Value = serde_json::from_str(text)?;
    let Some(object) = value.as_object() else {
        return Err(serde::de::Error::custom("arena message is not a JSON object"));
    };
    let kind = object.get("type").and_then(Value::as_str).unwrap_or_default();
    let string = |key: &str| object.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let number = |key: &str| object.get(key).and_then(Value::as_f64);
    Ok(match kind {
        "hello_ok" => ServerMessage::HelloOk(HelloOk {
            session_id: string("session_id"),
            server_protocol: number("server_protocol").map(|v| v as u32),
            heartbeat_seconds: number("heartbeat_seconds"),
            active_match_id: string("active_match_id"),
            server_time: number("server_time"),
        }),
        "pong" => ServerMessage::Pong {
            server_time: number("server_time"),
            client_time: number("client_time"),
        },
        "error" => ServerMessage::Error { code: string("code"), message: string("message") },
        "replaced" => ServerMessage::Replaced,
        "arena_status" => {
            let player = object.get("player").and_then(Value::as_object);
            ServerMessage::ArenaStatus(PlayerStatus {
                rating: player.and_then(|p| p.get("rating")).and_then(Value::as_i64),
                matches_played: player
                    .and_then(|p| p.get("matches_played"))
                    .and_then(Value::as_u64),
            })
        }
        other => ServerMessage::Unknown(other.to_owned()),
    })
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

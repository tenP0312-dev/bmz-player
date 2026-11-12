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
    Ping {
        client_time: f64,
    },
    State {
        state: ScreenState,
        arena_enabled: bool,
        ready: bool,
    },
    ArenaStatus {},
    /// Create (no `room_code`) or join (`room_code`) a private room.
    RoomEntry(RoomEntry),
    RoomReady {
        ready: bool,
    },
    ChartNominate {
        match_id: String,
        chart_hash: String,
    },
    ChartNominationSkip {
        match_id: String,
    },
    ChartCheck(ChartCheck),
}

/// `room_entry`. Only the fields the spec names are sent; the room settings of
/// section 2 ("fixed settings") have no field names in the spec and are not sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomEntry {
    pub match_mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_password: Option<String>,
    /// Only when creating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_format: Option<&'static str>,
}

impl RoomEntry {
    pub fn create() -> Self {
        Self {
            match_mode: "private",
            room_code: None,
            room_password: None,
            series_format: Some("single"),
        }
    }

    pub fn join(room_code: String, room_password: Option<String>) -> Self {
        Self {
            match_mode: "private",
            room_code: Some(room_code),
            room_password,
            series_format: None,
        }
    }
}

/// `chart_check`: whether this player has the candidate chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChartCheck {
    pub match_id: String,
    pub chart_hash: String,
    pub available: bool,
    /// 0 when unknown.
    pub totalnotes: u32,
    pub play_mode: u32,
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

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MatchReserved {
    pub match_id: String,
    pub ruleset_profile: String,
    /// The spec does not give the JSON type of `ln_mode`; strings and numbers are
    /// both kept as text.
    pub ln_mode: String,
    pub finish_current_song: bool,
    pub reservation_timeout: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChartAnnounced {
    /// Absent when the server does not repeat the match id.
    pub match_id: Option<String>,
    pub md5: String,
    pub title: String,
    pub artist: String,
    pub totalnotes: Option<u32>,
    pub random_seed: Option<i64>,
    pub ln_mode: String,
    pub load_deadline: Option<f64>,
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
    /// `room_status`. The spec does not name its fields; only `room_code` is read.
    RoomStatus {
        room_code: Option<String>,
    },
    MatchReserved(MatchReserved),
    FillStarted {
        deadline: Option<f64>,
        player_count: Option<u32>,
        max_players: Option<u32>,
    },
    NominationStarted {
        deadline: Option<f64>,
    },
    NominationAccepted,
    NominationsRevealed,
    /// The chosen chart (`chart`), sent when the match is already loading.
    Chart(ChartAnnounced),
    MatchCancelled {
        reason: String,
    },
    MatchReleased,
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
        "room_status" => ServerMessage::RoomStatus {
            room_code: Some(string("room_code")).filter(|code| !code.is_empty()),
        },
        "match_reserved" => ServerMessage::MatchReserved(MatchReserved {
            match_id: string("match_id"),
            ruleset_profile: string("ruleset_profile"),
            ln_mode: text_or_number(object.get("ln_mode")),
            finish_current_song: object
                .get("finish_current_song")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            reservation_timeout: number("reservation_timeout"),
        }),
        "fill_started" => ServerMessage::FillStarted {
            deadline: number("deadline"),
            player_count: number("player_count").map(|v| v as u32),
            max_players: number("max_players").map(|v| v as u32),
        },
        "nomination_started" => ServerMessage::NominationStarted { deadline: number("deadline") },
        "nomination_accepted" => ServerMessage::NominationAccepted,
        "nominations_revealed" => ServerMessage::NominationsRevealed,
        "chart" => {
            let chart = object.get("chart").and_then(Value::as_object);
            let chart_text = |key: &str| {
                chart
                    .and_then(|c| c.get(key))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            ServerMessage::Chart(ChartAnnounced {
                match_id: Some(string("match_id")).filter(|id| !id.is_empty()),
                md5: chart_text("md5").to_ascii_lowercase(),
                title: chart_text("title"),
                artist: chart_text("artist"),
                totalnotes: chart
                    .and_then(|c| c.get("totalnotes"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32),
                random_seed: object.get("random_seed").and_then(Value::as_i64),
                ln_mode: text_or_number(object.get("ln_mode")),
                load_deadline: number("load_deadline"),
            })
        }
        "match_cancelled" => ServerMessage::MatchCancelled { reason: string("reason") },
        "match_released" => ServerMessage::MatchReleased,
        other => ServerMessage::Unknown(other.to_owned()),
    })
}

fn text_or_number(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

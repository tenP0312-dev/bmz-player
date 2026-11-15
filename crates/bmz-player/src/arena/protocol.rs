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
    /// Host only. The server resets every omitted timing/series field to its
    /// default, so [`RoomRules`] always carries all of them.
    RoomSettings(RoomSettings),
    /// Leaves the room (cancels the room entry).
    QueueCancel {},
    OptionReady(OptionReady),
    PlayReady(PlayReady),
    /// Before the countdown this leaves the match (`match_released`); the
    /// loading step uses it when the chart cannot be started.
    Forfeit {
        match_id: String,
    },
}

/// `option_ready`: this player's arrange option (`1P + 10 * 2P + 100 * FLIP`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OptionReady {
    pub match_id: String,
    pub play_option: u32,
    pub play_mode: u32,
    pub ln_mode: String,
}

/// `play_ready`: the chart is loaded. `totalnotes` is the loaded note count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayReady {
    pub match_id: String,
    pub chart_hash: String,
    pub ln_mode: String,
    pub totalnotes: u32,
}

/// The rule fields shared by `room_entry` (create) and `room_settings`
/// (`ARENA_SERVER_MESSAGES.md` 2.1 / 2.2). Names are the server's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomRules {
    pub score_rule: &'static str,
    pub forced_gauge: &'static str,
    pub chart_scope: &'static str,
    pub ruleset_profile: RulesetProfile,
    pub nomination_policy: &'static str,
    pub nomination_seconds: u32,
    pub option_seconds: u32,
    pub intermission_seconds: u32,
    pub series_format: &'static str,
    pub first_to_wins: u32,
    pub spectator_public: bool,
    pub force_host_option: bool,
    pub allowed_play_modes: Vec<u32>,
}

impl RoomRules {
    /// The fixed foundation room (spec section 2): code-only, one song, EX SCORE,
    /// free gauge, free chart choice, everyone nominates, 7KEY only. The timing
    /// values are the server defaults, sent explicitly so a later
    /// `room_settings` cannot silently change them.
    pub fn foundation(ruleset_profile: RulesetProfile) -> Self {
        Self {
            score_rule: "exscore",
            forced_gauge: "free",
            chart_scope: "free",
            ruleset_profile,
            nomination_policy: "all",
            nomination_seconds: 60,
            option_seconds: 10,
            intermission_seconds: 0,
            series_format: "single",
            first_to_wins: 2,
            spectator_public: false,
            force_host_option: false,
            allowed_play_modes: vec![7],
        }
    }
}

/// `room_entry`: create (no `room_code`) or join (`room_code`). When joining,
/// the server takes every rule from the room, so only the code and password go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomEntry {
    pub match_mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_password: Option<String>,
    /// Create only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ln_mode: Option<&'static str>,
    /// Create only.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Box<RoomRules>>,
}

impl RoomEntry {
    pub fn create(ruleset_profile: RulesetProfile) -> Self {
        Self {
            match_mode: "private",
            room_code: None,
            room_password: None,
            ln_mode: Some("LN"),
            rules: Some(Box::new(RoomRules::foundation(ruleset_profile))),
        }
    }

    pub fn join(room_code: String, room_password: Option<String>) -> Self {
        Self {
            match_mode: "private",
            room_code: Some(room_code),
            room_password,
            ln_mode: None,
            rules: None,
        }
    }
}

/// `room_settings` (host only). `ln_mode` cannot be changed after creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomSettings {
    #[serde(flatten)]
    pub rules: Box<RoomRules>,
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
    /// `"LN"`, `"CN"` or `"HCN"` (the server always answers in upper case).
    pub ln_mode: String,
    pub finish_current_song: bool,
    pub reservation_timeout: Option<f64>,
}

/// One entry of `room_status.players`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomPlayer {
    pub player_id: u64,
    pub name: String,
    pub host: bool,
    pub participating: bool,
    /// Joins from the next series on.
    pub pending: bool,
    /// The server-side ready flag.
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomStatusInfo {
    pub room_code: Option<String>,
    pub room_host_id: Option<u64>,
    pub ln_mode: String,
    pub players: Vec<RoomPlayer>,
}

/// One entry of `nomination_*.players`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NominationPlayer {
    pub player_id: u64,
    pub name: String,
    pub submitted: bool,
    pub host: bool,
    pub selector: bool,
}

/// Payload of `nomination_started` and `nomination_status`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NominationInfo {
    pub match_id: String,
    /// Server time (UNIX seconds); `null` when there is no deadline.
    pub deadline: Option<f64>,
    pub can_nominate: Option<bool>,
    pub required_count: Option<u32>,
    pub submitted_count: Option<u32>,
    pub retry_reason: String,
    pub players: Vec<NominationPlayer>,
    /// MD5 of this player's nomination, if any.
    pub your_nomination: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RevealedNomination {
    pub player_id: u64,
    pub name: String,
    /// `player_selected`, `room_candidate`, `server_random` or `series_pool`.
    pub source: String,
    pub md5: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NominationsRevealed {
    pub match_id: String,
    pub nominations: Vec<RevealedNomination>,
    /// 0 when the server chose.
    pub selected_player_id: Option<u64>,
    pub md5: String,
    pub reroll_count: Option<u32>,
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
    pub reroll_count: Option<u32>,
    /// `null` (absent here) when the players' modes differ.
    pub play_mode: Option<u32>,
}

/// `option_select`: the option phase (sent again at every `option_ready`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionSelect {
    pub match_id: String,
    pub deadline: Option<f64>,
    /// This player's own `option_ready` was accepted.
    pub ready: bool,
    pub ready_count: Option<u32>,
    pub player_count: Option<u32>,
    pub play_mode: Option<u32>,
    pub ln_mode: String,
}

/// `prepare`: load the chart and start it yourself with the confirmed option.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Prepare {
    pub match_id: String,
    pub md5: String,
    pub totalnotes: Option<u32>,
    pub load_deadline: Option<f64>,
    pub ln_mode: String,
    pub random_seed: Option<i64>,
    /// `1P + 10 * 2P + 100 * FLIP`; the host's value when the host forces options.
    pub play_option: u32,
    pub play_mode: Option<u32>,
    /// `lr2` or `oraja` from the match `rules`.
    pub ruleset_profile: String,
}

/// `start`: begin the song when the server clock reaches `start_at`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Start {
    pub match_id: String,
    pub start_at: Option<f64>,
    pub ln_mode: String,
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
    RoomStatus(RoomStatusInfo),
    MatchReserved(MatchReserved),
    PlayersUpdated {
        match_id: String,
        player_ids: Vec<u64>,
    },
    FillStarted {
        deadline: Option<f64>,
        player_count: Option<u32>,
        max_players: Option<u32>,
    },
    NominationStarted(NominationInfo),
    NominationStatus(NominationInfo),
    NominationAccepted,
    NominationsRevealed(NominationsRevealed),
    /// The chosen chart (`chart`), sent when the match is already loading.
    Chart(ChartAnnounced),
    OptionSelect(OptionSelect),
    Prepare(Prepare),
    Start(Start),
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
        "room_status" => {
            let rules = object.get("rules").and_then(Value::as_object);
            ServerMessage::RoomStatus(RoomStatusInfo {
                room_code: Some(string("room_code")).filter(|code| !code.is_empty()),
                room_host_id: rules.and_then(|r| r.get("room_host_id")).and_then(Value::as_u64),
                ln_mode: rules.and_then(|r| r.get("ln_mode")).map(ln_mode_text).unwrap_or_default(),
                players: object
                    .get("players")
                    .and_then(Value::as_array)
                    .map(|players| players.iter().filter_map(room_player).collect())
                    .unwrap_or_default(),
            })
        }
        "match_reserved" => ServerMessage::MatchReserved(MatchReserved {
            match_id: string("match_id"),
            ruleset_profile: string("ruleset_profile"),
            ln_mode: ln_mode_of(object),
            finish_current_song: flag(object.get("finish_current_song")),
            reservation_timeout: number("reservation_timeout"),
        }),
        "players_updated" => ServerMessage::PlayersUpdated {
            match_id: string("match_id"),
            player_ids: object
                .get("player_ids")
                .and_then(Value::as_array)
                .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
                .unwrap_or_default(),
        },
        "fill_started" => ServerMessage::FillStarted {
            deadline: number("deadline"),
            player_count: number("player_count").map(|v| v as u32),
            max_players: number("max_players").map(|v| v as u32),
        },
        "nomination_started" => ServerMessage::NominationStarted(nomination_info(object)),
        "nomination_status" => ServerMessage::NominationStatus(nomination_info(object)),
        "nomination_accepted" => ServerMessage::NominationAccepted,
        "nominations_revealed" => {
            let chart = object.get("chart");
            ServerMessage::NominationsRevealed(NominationsRevealed {
                match_id: string("match_id"),
                nominations: object
                    .get("nominations")
                    .and_then(Value::as_array)
                    .map(|items| items.iter().filter_map(revealed_nomination).collect())
                    .unwrap_or_default(),
                selected_player_id: object.get("selected_player_id").and_then(Value::as_u64),
                md5: chart_md5(chart),
                reroll_count: number("reroll_count").map(|v| v as u32),
            })
        }
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
                ln_mode: ln_mode_of(object),
                load_deadline: number("load_deadline"),
                reroll_count: number("reroll_count").map(|v| v as u32),
                play_mode: object.get("play_mode").and_then(Value::as_u64).map(|v| v as u32),
            })
        }
        "option_select" => ServerMessage::OptionSelect(OptionSelect {
            match_id: string("match_id"),
            deadline: number("deadline"),
            ready: flag(object.get("ready")),
            ready_count: number("ready_count").map(|v| v as u32),
            player_count: number("player_count").map(|v| v as u32),
            play_mode: object.get("play_mode").and_then(Value::as_u64).map(|v| v as u32),
            ln_mode: ln_mode_of(object),
        }),
        "prepare" => {
            let chart = object.get("chart").and_then(Value::as_object);
            ServerMessage::Prepare(Prepare {
                match_id: string("match_id"),
                md5: chart_md5(object.get("chart")),
                totalnotes: chart
                    .and_then(|c| c.get("totalnotes"))
                    .and_then(Value::as_u64)
                    .map(|v| v as u32),
                load_deadline: number("load_deadline"),
                ln_mode: ln_mode_of(object),
                random_seed: object.get("random_seed").and_then(Value::as_i64),
                play_option: object.get("play_option").and_then(Value::as_u64).unwrap_or(0) as u32,
                play_mode: object.get("play_mode").and_then(Value::as_u64).map(|v| v as u32),
                ruleset_profile: object
                    .get("rules")
                    .and_then(|r| r.get("ruleset_profile"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        }
        "start" => ServerMessage::Start(Start {
            match_id: string("match_id"),
            start_at: number("start_at"),
            ln_mode: ln_mode_of(object),
        }),
        "match_cancelled" => ServerMessage::MatchCancelled { reason: string("reason") },
        "match_released" => ServerMessage::MatchReleased,
        other => ServerMessage::Unknown(other.to_owned()),
    })
}

/// Only `true` counts (`ARENA_SERVER_MESSAGES.md` section 0).
fn flag(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

/// `ln_mode` is an upper-case string; a number is not expected from the server.
fn ln_mode_text(value: &Value) -> String {
    value.as_str().map(str::to_ascii_uppercase).unwrap_or_default()
}

fn ln_mode_of(object: &serde_json::Map<String, Value>) -> String {
    object.get("ln_mode").map(ln_mode_text).unwrap_or_default()
}

fn chart_md5(chart: Option<&Value>) -> String {
    chart
        .and_then(|c| c.get("md5"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn room_player(value: &Value) -> Option<RoomPlayer> {
    let player = value.as_object()?;
    Some(RoomPlayer {
        player_id: player.get("player_id")?.as_u64()?,
        name: player.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        host: flag(player.get("host")),
        // `participating` is false only when it says so (default true).
        participating: player.get("participating").and_then(Value::as_bool).unwrap_or(true),
        pending: flag(player.get("pending")),
        ready: flag(player.get("ready")),
    })
}

fn nomination_info(object: &serde_json::Map<String, Value>) -> NominationInfo {
    let count = |key: &str| object.get(key).and_then(Value::as_u64).map(|v| v as u32);
    NominationInfo {
        match_id: object.get("match_id").and_then(Value::as_str).unwrap_or_default().to_owned(),
        deadline: object.get("deadline").and_then(Value::as_f64),
        can_nominate: object.get("can_nominate").and_then(Value::as_bool),
        required_count: count("required_count"),
        submitted_count: count("submitted_count"),
        retry_reason: object
            .get("retry_reason")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        players: object
            .get("players")
            .and_then(Value::as_array)
            .map(|players| {
                players
                    .iter()
                    .filter_map(|value| {
                        let player = value.as_object()?;
                        Some(NominationPlayer {
                            player_id: player.get("player_id")?.as_u64()?,
                            name: player
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            submitted: flag(player.get("submitted")),
                            host: flag(player.get("host")),
                            selector: flag(player.get("selector")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        // `your_nomination` is a chart object, or `{}` when there is none.
        your_nomination: Some(chart_md5(object.get("your_nomination")))
            .filter(|md5| !md5.is_empty()),
    }
}

fn revealed_nomination(value: &Value) -> Option<RevealedNomination> {
    let item = value.as_object()?;
    let chart = item.get("chart").and_then(Value::as_object);
    Some(RevealedNomination {
        player_id: item.get("player_id").and_then(Value::as_u64).unwrap_or(0),
        name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_owned(),
        source: item.get("source").and_then(Value::as_str).unwrap_or_default().to_owned(),
        md5: chart_md5(item.get("chart")),
        title: chart
            .and_then(|c| c.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;

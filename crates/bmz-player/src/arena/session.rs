//! Arena connection state machine.
//!
//! No I/O and no clock of its own: the connection thread feeds it events with
//! the current time and performs the returned actions. That keeps every rule of
//! the foundation (hello, keep-alive, idle cut-off, reconnect, refusals,
//! `replaced`) testable without a network.

use std::collections::VecDeque;
use std::time::Duration;

use super::protocol::{
    ArenaResult, ChartAnnounced, ChartCheck, ClientMessage, FinalReport, FinalState, Hello,
    LivePlayer, LiveReport, NominationInfo, NominationsRevealed, OptionReady, OptionSelect,
    PlayReady, PlayerStatus, Prepare, RoomEntry, RoomPlayer, RoomRules, RoomSettings,
    RoomStatusInfo, ScreenState, ServerMessage,
};

/// `ping` every 5 seconds.
pub const PING_INTERVAL: Duration = Duration::from_secs(5);
/// The server drops a client it has not heard from for 20 seconds; the client
/// applies the same limit to the server.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(20);
/// The server keeps a dropped player's match for 30 seconds.
pub const RECONNECT_GRACE: Duration = Duration::from_secs(30);
/// Delay before a reconnect attempt while the grace period lasts.
pub const RECONNECT_DELAY: Duration = Duration::from_secs(5);
/// After the grace period nothing is left to rejoin, so retry slowly.
pub const RECONNECT_DELAY_AFTER_GRACE: Duration = Duration::from_secs(30);
const MAX_PENDING_PINGS: usize = 8;

/// Intervals used by the state machine. Production uses [`Timing::default`];
/// tests with a fake server shorten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub ping_interval: Duration,
    pub idle_timeout: Duration,
    pub reconnect_grace: Duration,
    pub reconnect_delay: Duration,
    pub reconnect_delay_after_grace: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            ping_interval: PING_INTERVAL,
            idle_timeout: IDLE_TIMEOUT,
            reconnect_grace: RECONNECT_GRACE,
            reconnect_delay: RECONNECT_DELAY,
            reconnect_delay_after_grace: RECONNECT_DELAY_AFTER_GRACE,
        }
    }
}

/// A point in time: monotonic (for timeouts) and wall clock (for the protocol).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    pub mono: Duration,
    pub unix: f64,
}

/// Why the server refused the client. None of these are retried automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
    /// `protocol_mismatch`, `unsupported_client`, `unsupported_build`,
    /// `unsupported_client_flavor`: this build needs an update.
    UpdateRequired,
    /// `authentication_failed`: check the BMS-IR login.
    AuthenticationFailed,
    /// `client_flavor_conflict`: the match in progress does not support this client.
    FlavorConflict,
}

pub fn refusal_kind(code: &str) -> Option<RefusalKind> {
    match code {
        "protocol_mismatch"
        | "unsupported_client"
        | "unsupported_build"
        | "unsupported_client_flavor" => Some(RefusalKind::UpdateRequired),
        "authentication_failed" => Some(RefusalKind::AuthenticationFailed),
        "client_flavor_conflict" => Some(RefusalKind::FlavorConflict),
        _ => None,
    }
}

/// What the panel shows as the connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    /// Arena is off in the profile (the default). Nothing is connected.
    #[default]
    Disabled,
    /// No BMS-IR login on this profile.
    NoCredentials,
    /// The profile rule has no Arena ruleset (`Dx`).
    UnsupportedRule,
    Connecting,
    Connected,
    /// Waiting to retry after a lost connection.
    Reconnecting,
    Refused(RefusalKind),
    /// Another client took over this account. Not retried automatically.
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// Connect failed or the socket closed.
    ConnectionLost,
    /// 20 seconds without anything from the server.
    IdleTimeout,
}

/// A non-fatal server error shown once in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: String,
    pub message: String,
}

/// Where the match is, as far as B2 follows it (up to the chosen chart).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStage {
    Reserved,
    Filling,
    Nominating,
    /// Nominations are revealed; the server is choosing.
    Revealed,
    /// The chart is decided and the match is loading.
    ChartChosen,
    /// Everyone has the chart; arrange options are being chosen.
    Options,
    /// `prepare` arrived: this player starts the chart and loads it.
    Loading,
    /// `start` arrived: the song begins at the server time `start_at`.
    Countdown,
    /// The song is being played; `live` goes out every second.
    Playing,
    /// The song ended and `final` was produced; waiting for `result`.
    Finished,
}

impl MatchStage {
    /// The chart is loaded or being played: a replay of the earlier messages
    /// after a reconnect must not restart anything.
    fn is_started(self) -> bool {
        matches!(self, Self::Loading | Self::Countdown | Self::Playing | Self::Finished)
    }
}

/// The score values of a play, as `live` and `final` carry them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayValues {
    pub exscore: u32,
    /// Judged notes (never above the song's note count).
    pub processed_notes: u32,
    /// BAD + POOR + empty POOR.
    pub minbp: u32,
    /// BAD + POOR.
    pub combo_break: u32,
    pub max_combo: u32,
}

impl PlayValues {
    /// Keeps what the server checks: nothing falls, nothing passes the note
    /// count, EX is at most 2 per processed note and grows by at most 2 per new
    /// note. The play is the truth; this only protects against a sampling quirk
    /// that would make the server drop the report.
    fn clamped(self, previous: Self, total_notes: u32) -> Self {
        let processed_notes = self.processed_notes.max(previous.processed_notes).min(total_notes);
        let new_notes = processed_notes.saturating_sub(previous.processed_notes);
        let exscore = self
            .exscore
            .max(previous.exscore)
            .min(processed_notes.saturating_mul(2))
            .min(previous.exscore.saturating_add(new_notes.saturating_mul(2)));
        Self {
            exscore,
            processed_notes,
            minbp: self.minbp.max(previous.minbp),
            combo_break: self.combo_break.max(previous.combo_break),
            max_combo: self.max_combo.max(previous.max_combo).min(processed_notes),
        }
    }
}

/// `final.state` and `final.clear_type` for a finished play. A failed play, or
/// one that ended with a FAILED clear (gauge below the border), is `hard_fail`
/// with clear 1; the server rejects any other combination.
pub fn final_state_for(failed: bool, clear_type: u8) -> (FinalState, u8) {
    if failed || clear_type <= 1 {
        (FinalState::HardFail, 1)
    } else {
        (FinalState::Complete, clear_type.min(10))
    }
}

/// The option phase (`option_select`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OptionPhase {
    /// Server time (UNIX seconds).
    pub deadline: Option<f64>,
    /// The server has this player's `option_ready`.
    pub ready: bool,
    pub ready_count: Option<u32>,
    pub player_count: Option<u32>,
    pub play_mode: Option<u32>,
    pub ln_mode: String,
}

/// When to begin the song, in server time.
#[derive(Debug, Clone, PartialEq)]
pub struct StartGate {
    pub match_id: String,
    pub start_at: f64,
}

/// `live` at most once a second (the server drops any closer than 0.2 s).
pub const LIVE_INTERVAL: Duration = Duration::from_secs(1);
/// `live` is sent from this many seconds before `start_at` (the server's limit is 2).
const LIVE_EARLIEST_LEAD: f64 = 1.0;

/// Auto-send of `option_ready` this long before the deadline (spec 3.4 step 8).
const OPTION_AUTO_SEND_LEAD: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct MatchView {
    pub match_id: String,
    pub stage: MatchStage,
    /// The match ends the current play first (`finish_current_song`).
    pub finish_current_song: bool,
    pub player_count: Option<u32>,
    pub max_players: Option<u32>,
    /// Server time (UNIX seconds) of the fill or nomination deadline.
    pub deadline: Option<f64>,
    /// The server accepted this player's nomination (or skip).
    pub nominated: bool,
    /// `false` when the server says this player has no nomination to make
    /// (for example `host` policy and not the host).
    pub can_nominate: bool,
    /// Why the server asked again (`retry_reason`), empty normally.
    pub retry_reason: String,
    pub submitted_count: Option<u32>,
    pub required_count: Option<u32>,
    pub revealed: Option<NominationsRevealed>,
    pub chart: Option<ChartAnnounced>,
    pub option: Option<OptionPhase>,
    /// The confirmed option and seed for this player's own play.
    pub prepare: Option<Prepare>,
    /// This player's `play_ready` was sent.
    pub play_ready_sent: bool,
    pub start_at: Option<f64>,
    /// The song's note count as `play_ready` reported it.
    pub total_notes: Option<u32>,
    /// The audio clock started: `live` may be sent.
    pub play_started: bool,
    /// Sequence number of the last `live` / `final` (shared).
    pub live_seq: u32,
    /// The values the last `live` / `final` carried.
    pub last_values: PlayValues,
    /// `final` not yet answered by `result`. It is sent again after a reconnect.
    pub pending_final: Option<FinalReport>,
    /// Everyone's progress from the server's `live`.
    pub live_players: Vec<LivePlayer>,
    last_live_at: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoomView {
    /// Known once `room_status` arrives.
    pub code: Option<String>,
    /// What this player asked for. The server accepts `ready` only while the
    /// reported state is `select`, so the flag sent with `state` also needs the screen.
    pub ready: bool,
    pub host_id: Option<u64>,
    pub ln_mode: String,
    /// Members in entry order, with the server-side ready flag.
    pub players: Vec<RoomPlayer>,
}

/// The chart check the window thread has to answer (it owns the song library).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartCheckRequest {
    pub match_id: String,
    pub md5: String,
}

/// The `match_cancelled` reason the server gives when the room is disbanded
/// during a match (also used for the same case in `room_disbanded`).
pub const ROOM_DISBANDED_REASON: &str = "private_room_disbanded";

/// Why the match in view went away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchEnd {
    Cancelled(String),
    Released,
    /// The server accepted this player's forfeit (`reason`).
    Forfeited(String),
    /// The host disbanded the room.
    RoomDisbanded,
    /// The host removed this player.
    RoomKicked,
}

/// Everything the panel needs, copied out after each step.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArenaPanelState {
    pub status: ConnectionStatus,
    pub session_id: Option<String>,
    /// This player's BMS-IR ID (the `hello` one).
    pub own_player_id: Option<u64>,
    /// Server time minus local time, in seconds.
    pub clock_offset_secs: Option<f64>,
    pub round_trip_secs: Option<f64>,
    /// A match the server says is in progress (shown only; B1 cannot rejoin it).
    pub active_match_id: Option<String>,
    pub notice: Option<Notice>,
    /// The player's own rating from the latest `arena_status`.
    pub player: Option<PlayerStatus>,
    pub room: Option<RoomView>,
    pub current_match: Option<MatchView>,
    pub pending_chart_check: Option<ChartCheckRequest>,
    /// `prepare` waiting for the app to start the chart ([`Event::PrepareTaken`]).
    pub pending_prepare: Option<Prepare>,
    pub start_gate: Option<StartGate>,
    pub match_end: Option<MatchEnd>,
    /// The last finished match's result, kept until the next match.
    pub result: Option<ArenaResult>,
    /// MD5 (lowercase hex) of the chart under the select cursor, if any.
    pub selected_chart_md5: Option<String>,
    pub last_disconnect: Option<DisconnectReason>,
    /// Wall-clock time of the next reconnect attempt, while `Reconnecting`.
    pub retry_at_unix: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The WebSocket is open.
    SocketOpened,
    Message(ServerMessage),
    /// The connect failed or the socket closed.
    SocketClosed,
    Tick,
    SetScreen(ScreenState),
    /// The user asked to connect again (after a refusal or `replaced`).
    ManualReconnect,
    RoomCreate,
    RoomJoin {
        code: String,
        password: Option<String>,
    },
    SetReady(bool),
    /// Leave the room (`queue_cancel`).
    RoomLeave,
    /// Host: send the fixed foundation rules again (`room_settings`, all fields).
    RoomSettingsApply,
    /// Nominate this chart (lowercase hex MD5) for the match.
    Nominate(String),
    SkipNomination,
    /// The window thread's answer to [`ArenaPanelState::pending_chart_check`].
    ChartChecked {
        match_id: String,
        md5: String,
        available: bool,
        totalnotes: u32,
        play_mode: u32,
    },
    SetSelectedChart(Option<String>),
    /// The `play_option` for the player's current options (kept up to date by
    /// the app); sent at the deadline when the player does not press READY.
    SetOptionChoice(u32),
    /// The player pressed READY in the option phase.
    OptionReadyNow,
    /// The app started the chart for the pending `prepare`.
    PrepareTaken {
        match_id: String,
    },
    /// The chart is loaded and waits for READY (`totalnotes` as loaded).
    PlayLoaded {
        match_id: String,
        md5: String,
        totalnotes: u32,
    },
    /// The chart could not be started or loaded: leave the match.
    LoadFailed {
        match_id: String,
    },
    /// The audio clock of the match song started.
    PlayStarted {
        match_id: String,
    },
    /// The play's current values (the app gives them every frame).
    LiveSample {
        match_id: String,
        values: PlayValues,
    },
    /// The song ended.
    PlayFinished {
        match_id: String,
        values: PlayValues,
        /// The play ended in the FAILED state.
        failed: bool,
        /// The play's clear type (0..=10).
        clear_type: u8,
    },
    /// The player gives up the match (also sent when the app closes).
    Withdraw,
    /// The result screen's normal IR submission is finished (spec 3.3 `ready`).
    SetResultSettled(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Send(ClientMessage),
    /// Close the socket; the session has already decided what happens next.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The driver should open a socket.
    Connecting,
    /// Socket open, `hello` sent.
    Authenticating,
    Connected,
    WaitingToReconnect,
    Refused(RefusalKind),
    Replaced,
}

pub struct ArenaSession {
    timing: Timing,
    hello: Hello,
    phase: Phase,
    screen: ScreenState,
    sent_state: Option<(ScreenState, bool)>,
    last_rx: Duration,
    next_ping: Duration,
    /// `(client_time sent, monotonic send time)` for pings without a pong yet.
    pending_pings: VecDeque<(f64, Duration)>,
    clock_offset: Option<f64>,
    round_trip: Option<f64>,
    session_id: Option<String>,
    active_match_id: Option<String>,
    notice: Option<Notice>,
    player: Option<PlayerStatus>,
    room: Option<RoomView>,
    current_match: Option<MatchView>,
    pending_chart_check: Option<ChartCheckRequest>,
    pending_prepare: Option<Prepare>,
    option_choice: Option<u32>,
    option_sent: bool,
    match_end: Option<MatchEnd>,
    result: Option<ArenaResult>,
    result_settled: bool,
    selected_chart_md5: Option<String>,
    last_disconnect: Option<DisconnectReason>,
    disconnected_since: Option<Duration>,
    retry_at: Option<(Duration, f64)>,
    unknown_messages: u64,
}

impl ArenaSession {
    pub fn new(hello: Hello, screen: ScreenState) -> Self {
        Self::with_timing(hello, screen, Timing::default())
    }

    pub fn with_timing(hello: Hello, screen: ScreenState, timing: Timing) -> Self {
        Self {
            timing,
            hello,
            phase: Phase::Connecting,
            screen,
            sent_state: None,
            last_rx: Duration::ZERO,
            next_ping: Duration::ZERO,
            pending_pings: VecDeque::new(),
            clock_offset: None,
            round_trip: None,
            session_id: None,
            active_match_id: None,
            notice: None,
            player: None,
            room: None,
            current_match: None,
            pending_chart_check: None,
            pending_prepare: None,
            option_choice: None,
            option_sent: false,
            match_end: None,
            result: None,
            result_settled: false,
            selected_chart_md5: None,
            last_disconnect: None,
            disconnected_since: None,
            retry_at: None,
            unknown_messages: 0,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn unknown_message_count(&self) -> u64 {
        self.unknown_messages
    }

    /// `ready` is true only inside a room, once the player pressed "ready", and
    /// only on the select screen or on the result screen after the normal IR
    /// submission finished (spec 3.3). Practice, autoplay, replay, course and
    /// play never say ready. (The server reads `ready` on `select` only; the
    /// result-screen value is for the spec's contract.)
    fn desired_ready(&self) -> bool {
        let screen_allows = match self.screen {
            ScreenState::Select => true,
            ScreenState::Result => self.result_settled,
            _ => false,
        };
        screen_allows && self.room.as_ref().is_some_and(|room| room.ready)
    }

    /// A message names a match, and it is not the one in view. (A missing id
    /// is not a reason to drop the message.)
    fn is_other_match(&self, match_id: &str) -> bool {
        !match_id.is_empty()
            && self.current_match.as_ref().is_some_and(|current| current.match_id != match_id)
    }

    fn forget_match(&mut self) {
        self.current_match = None;
        self.pending_chart_check = None;
        self.pending_prepare = None;
        self.option_sent = false;
    }

    fn reset_ready(&mut self) {
        if let Some(room) = self.room.as_mut() {
            room.ready = false;
        }
    }

    fn forget_room(&mut self) {
        self.room = None;
        self.forget_match();
    }

    /// Server time for a local wall-clock time, once the clock is synchronised.
    pub fn server_time(&self, local_unix: f64) -> Option<f64> {
        self.clock_offset.map(|offset| local_unix + offset)
    }

    pub fn panel(&self) -> ArenaPanelState {
        let status = match self.phase {
            Phase::Connecting | Phase::Authenticating => ConnectionStatus::Connecting,
            Phase::Connected => ConnectionStatus::Connected,
            Phase::WaitingToReconnect => ConnectionStatus::Reconnecting,
            Phase::Refused(kind) => ConnectionStatus::Refused(kind),
            Phase::Replaced => ConnectionStatus::Replaced,
        };
        ArenaPanelState {
            status,
            session_id: self.session_id.clone(),
            own_player_id: Some(self.hello.player_id),
            clock_offset_secs: self.clock_offset,
            round_trip_secs: self.round_trip,
            active_match_id: self.active_match_id.clone(),
            notice: self.notice.clone(),
            player: self.player.clone(),
            room: self.room.clone(),
            current_match: self.current_match.clone(),
            pending_chart_check: self.pending_chart_check.clone(),
            pending_prepare: self.pending_prepare.clone(),
            start_gate: self.current_match.as_ref().and_then(|current| {
                current
                    .start_at
                    .map(|start_at| StartGate { match_id: current.match_id.clone(), start_at })
            }),
            match_end: self.match_end.clone(),
            result: self.result.clone(),
            selected_chart_md5: self.selected_chart_md5.clone(),
            last_disconnect: self.last_disconnect,
            retry_at_unix: self.retry_at.map(|(_, unix)| unix),
        }
    }

    pub fn handle(&mut self, now: Now, event: Event) -> Vec<Action> {
        match event {
            Event::SocketOpened => self.on_opened(now),
            Event::Message(message) => self.on_message(now, message),
            Event::SocketClosed => {
                if matches!(
                    self.phase,
                    Phase::Connecting | Phase::Authenticating | Phase::Connected
                ) {
                    self.begin_reconnect(now, DisconnectReason::ConnectionLost);
                }
                Vec::new()
            }
            Event::Tick => self.on_tick(now),
            Event::SetScreen(screen) => {
                let mut actions = Vec::new();
                // Back on the select screen in the middle of the song, without
                // `final`: the server would end the match with `play_aborted`
                // anyway. Say so first (this should not happen: ESC is locked).
                if screen == ScreenState::Select
                    && self.screen != ScreenState::Select
                    && let Some(current) = self.current_match.as_ref()
                    && matches!(current.stage, MatchStage::Countdown | MatchStage::Playing)
                    && current.pending_final.is_none()
                    && self.phase == Phase::Connected
                {
                    actions.push(Action::Send(ClientMessage::Forfeit {
                        match_id: current.match_id.clone(),
                    }));
                }
                self.screen = screen;
                if screen != ScreenState::Result {
                    self.result_settled = false;
                }
                actions.extend(self.state_message().into_iter().map(Action::Send));
                actions
            }
            Event::SetResultSettled(settled) => {
                self.result_settled = settled;
                self.state_message().into_iter().map(Action::Send).collect()
            }
            Event::Withdraw => {
                if self.phase != Phase::Connected {
                    return Vec::new();
                }
                match self.current_match.as_ref() {
                    // After `final` the play is done; the server settles it.
                    Some(current) if current.pending_final.is_none() => {
                        vec![Action::Send(ClientMessage::Forfeit {
                            match_id: current.match_id.clone(),
                        })]
                    }
                    _ => Vec::new(),
                }
            }
            Event::PlayStarted { match_id } => {
                if let Some(current) =
                    self.current_match.as_mut().filter(|current| current.match_id == match_id)
                    && matches!(current.stage, MatchStage::Loading | MatchStage::Countdown)
                {
                    current.stage = MatchStage::Playing;
                    current.play_started = true;
                }
                Vec::new()
            }
            Event::LiveSample { match_id, values } => self.live_sample(now, &match_id, values),
            Event::PlayFinished { match_id, values, failed, clear_type } => {
                self.play_finished(&match_id, values, failed, clear_type)
            }
            Event::ManualReconnect => {
                if matches!(self.phase, Phase::Refused(_) | Phase::Replaced) {
                    self.phase = Phase::Connecting;
                    self.notice = None;
                    self.disconnected_since = None;
                    self.retry_at = None;
                }
                Vec::new()
            }
            Event::SetSelectedChart(md5) => {
                self.selected_chart_md5 = md5;
                Vec::new()
            }
            Event::RoomCreate => self.enter_room(RoomEntry::create(self.hello.ruleset_profile)),
            Event::RoomLeave => {
                if self.phase != Phase::Connected || self.room.is_none() {
                    return Vec::new();
                }
                // Before a match the server removes us at once. With a match the
                // server answers with `match_released` / `forfeit_accepted`, so the
                // match view stays until then.
                if self.current_match.is_none() {
                    self.room = None;
                }
                let mut actions = vec![Action::Send(ClientMessage::QueueCancel {})];
                actions.extend(self.state_message().into_iter().map(Action::Send));
                actions
            }
            Event::RoomSettingsApply => {
                let is_host = self
                    .room
                    .as_ref()
                    .is_some_and(|room| room.host_id == Some(self.hello.player_id));
                if self.phase != Phase::Connected || !is_host {
                    return Vec::new();
                }
                vec![Action::Send(ClientMessage::RoomSettings(RoomSettings {
                    rules: Box::new(RoomRules::foundation(self.hello.ruleset_profile)),
                }))]
            }
            Event::RoomJoin { code, password } => {
                let code = code.trim().to_owned();
                if code.is_empty() {
                    return Vec::new();
                }
                self.enter_room(RoomEntry::join(code, password.filter(|p| !p.is_empty())))
            }
            Event::SetReady(ready) => {
                if self.phase != Phase::Connected {
                    return Vec::new();
                }
                let Some(room) = self.room.as_mut() else {
                    return Vec::new();
                };
                room.ready = ready;
                let mut actions = vec![Action::Send(ClientMessage::RoomReady { ready })];
                actions.extend(self.state_message().into_iter().map(Action::Send));
                actions
            }
            Event::Nominate(md5) => {
                let md5 = md5.to_ascii_lowercase();
                if md5.len() != 32 || !md5.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Vec::new();
                }
                self.nomination(|match_id| ClientMessage::ChartNominate {
                    match_id,
                    chart_hash: md5,
                })
            }
            Event::SkipNomination => {
                self.nomination(|match_id| ClientMessage::ChartNominationSkip { match_id })
            }
            Event::SetOptionChoice(choice) => {
                self.option_choice = Some(choice);
                Vec::new()
            }
            Event::OptionReadyNow => {
                self.send_option_ready().into_iter().map(Action::Send).collect()
            }
            Event::PrepareTaken { match_id } => {
                if self.pending_prepare.as_ref().is_some_and(|p| p.match_id == match_id) {
                    self.pending_prepare = None;
                }
                Vec::new()
            }
            Event::PlayLoaded { match_id, md5, totalnotes } => {
                let md5 = md5.to_ascii_lowercase();
                let Some(current) = self.current_match.as_mut() else {
                    return Vec::new();
                };
                let Some(prepare) = current.prepare.as_ref() else {
                    return Vec::new();
                };
                // One `play_ready` per match, for the chart `prepare` named. v8
                // needs the loaded note count (> 0).
                if current.stage != MatchStage::Loading
                    || current.play_ready_sent
                    || current.match_id != match_id
                    || prepare.md5 != md5
                    || totalnotes == 0
                {
                    return Vec::new();
                }
                // Loaded while the connection is down: remembered, and said again
                // after the reconnect (`resend_after_reconnect`).
                current.play_ready_sent = true;
                current.total_notes = Some(totalnotes);
                if self.phase != Phase::Connected {
                    return Vec::new();
                }
                vec![Action::Send(ClientMessage::PlayReady(PlayReady {
                    match_id,
                    chart_hash: md5,
                    ln_mode: prepare.ln_mode.clone(),
                    totalnotes,
                }))]
            }
            Event::LoadFailed { match_id } => {
                if self.phase != Phase::Connected
                    || self
                        .current_match
                        .as_ref()
                        .is_none_or(|current| current.match_id != match_id)
                {
                    return Vec::new();
                }
                vec![Action::Send(ClientMessage::Forfeit { match_id })]
            }
            Event::ChartChecked { match_id, md5, available, totalnotes, play_mode } => {
                // An answer for an older candidate (re-roll) or an ended match is dropped.
                let current = ChartCheckRequest { match_id: match_id.clone(), md5: md5.clone() };
                if self.phase != Phase::Connected
                    || self.pending_chart_check.as_ref() != Some(&current)
                {
                    return Vec::new();
                }
                self.pending_chart_check = None;
                vec![Action::Send(ClientMessage::ChartCheck(ChartCheck {
                    match_id,
                    chart_hash: md5,
                    available,
                    totalnotes,
                    play_mode,
                }))]
            }
        }
    }

    /// `live`: at most once a second, only while the song plays, never after `final`.
    fn live_sample(&mut self, now: Now, match_id: &str, values: PlayValues) -> Vec<Action> {
        if self.phase != Phase::Connected {
            return Vec::new();
        }
        let Some(current) = self.current_match.as_mut() else {
            return Vec::new();
        };
        let (Some(total), Some(prepare)) = (current.total_notes, current.prepare.as_ref()) else {
            return Vec::new();
        };
        if current.match_id != match_id
            || current.stage != MatchStage::Playing
            || !current.play_started
            || current.pending_final.is_some()
            || current.last_live_at.is_some_and(|at| now.mono.saturating_sub(at) < LIVE_INTERVAL)
        {
            return Vec::new();
        }
        // The server refuses `live` more than 2 s before `start_at` (`too_early`).
        // The clock starts `playstart` before it, so wait for the last second.
        if let (Some(start_at), Some(offset)) = (current.start_at, self.clock_offset)
            && now.unix + offset < start_at - LIVE_EARLIEST_LEAD
        {
            return Vec::new();
        }
        let values = values.clamped(current.last_values, total);
        current.live_seq += 1;
        current.last_live_at = Some(now.mono);
        current.last_values = values;
        vec![Action::Send(ClientMessage::Live(live_report(
            current.match_id.clone(),
            current.live_seq,
            values,
            prepare,
        )))]
    }

    /// `final`: once, when the song ends. It is kept until `result` arrives so
    /// that a lost connection can send it again.
    fn play_finished(
        &mut self,
        match_id: &str,
        values: PlayValues,
        failed: bool,
        clear_type: u8,
    ) -> Vec<Action> {
        let connected = self.phase == Phase::Connected;
        let Some(current) = self.current_match.as_mut() else {
            return Vec::new();
        };
        let (Some(total), Some(prepare)) = (current.total_notes, current.prepare.as_ref()) else {
            return Vec::new();
        };
        if current.match_id != match_id
            || !matches!(current.stage, MatchStage::Playing | MatchStage::Countdown)
            || current.pending_final.is_some()
        {
            return Vec::new();
        }
        let (state, clear_type) = final_state_for(failed, clear_type);
        let mut values = values.clamped(current.last_values, total);
        if state == FinalState::Complete {
            // A complete play processed every note (the server requires it).
            values.processed_notes = total;
        }
        current.live_seq += 1;
        current.last_values = values;
        let report = FinalReport {
            live: live_report(current.match_id.clone(), current.live_seq, values, prepare),
            state,
            clear_type,
        };
        current.pending_final = Some(report.clone());
        current.stage = MatchStage::Finished;
        if connected { vec![Action::Send(ClientMessage::Final(report))] } else { Vec::new() }
    }

    /// What a reconnect has to say again, before `state`: the `final` the server
    /// may not have seen, and `play_ready` while the load is waiting for the start.
    fn resend_after_reconnect(&self) -> Vec<Action> {
        let Some(current) = self.current_match.as_ref() else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        if let Some(report) = &current.pending_final {
            actions.push(Action::Send(ClientMessage::Final(report.clone())));
        }
        if current.stage == MatchStage::Loading
            && current.play_ready_sent
            && let (Some(prepare), Some(totalnotes)) =
                (current.prepare.as_ref(), current.total_notes)
        {
            actions.push(Action::Send(ClientMessage::PlayReady(PlayReady {
                match_id: current.match_id.clone(),
                chart_hash: prepare.md5.clone(),
                ln_mode: prepare.ln_mode.clone(),
                totalnotes,
            })));
        }
        actions
    }

    fn enter_room(&mut self, entry: RoomEntry) -> Vec<Action> {
        if self.phase != Phase::Connected || self.current_match.is_some() {
            return Vec::new();
        }
        self.match_end = None;
        self.result = None;
        self.room = Some(RoomView::default());
        vec![Action::Send(ClientMessage::RoomEntry(entry))]
    }

    /// Sends a nomination message once, and only while nominations are open.
    fn nomination(&mut self, build: impl FnOnce(String) -> ClientMessage) -> Vec<Action> {
        if self.phase != Phase::Connected {
            return Vec::new();
        }
        let Some(current) = self.current_match.as_ref() else {
            return Vec::new();
        };
        if current.stage != MatchStage::Nominating || current.nominated || !current.can_nominate {
            return Vec::new();
        }
        vec![Action::Send(build(current.match_id.clone()))]
    }

    fn on_opened(&mut self, now: Now) -> Vec<Action> {
        if self.phase != Phase::Connecting {
            return Vec::new();
        }
        self.phase = Phase::Authenticating;
        self.last_rx = now.mono;
        self.sent_state = None;
        self.pending_pings.clear();
        self.retry_at = None;
        vec![Action::Send(ClientMessage::Hello(self.hello.clone()))]
    }

    /// The match in view if `match_id` is its id (an empty id matches: the
    /// server always sends it, but a missing one is not a reason to drop a message).
    fn current_match_for(&mut self, match_id: &str) -> Option<&mut MatchView> {
        self.current_match
            .as_mut()
            .filter(|current| match_id.is_empty() || current.match_id == match_id)
    }

    /// The option phase is open for this player and its auto-send time has come.
    fn option_deadline_near(&self, now: Now) -> bool {
        let (Some(current), Some(offset)) = (self.current_match.as_ref(), self.clock_offset) else {
            return false;
        };
        let Some(option) = current.option.as_ref() else {
            return false;
        };
        current.stage == MatchStage::Options
            && !option.ready
            && !self.option_sent
            && option
                .deadline
                .is_some_and(|deadline| now.unix + offset >= deadline - OPTION_AUTO_SEND_LEAD)
    }

    /// `option_ready` for the choice the app last gave (NORMAL when it gave none),
    /// once per match and only while the option phase is open.
    fn send_option_ready(&mut self) -> Option<ClientMessage> {
        if self.phase != Phase::Connected || self.option_sent {
            return None;
        }
        let current = self.current_match.as_mut()?;
        if current.stage != MatchStage::Options {
            return None;
        }
        let option = current.option.as_mut().filter(|option| !option.ready)?;
        self.option_sent = true;
        option.ready = true;
        Some(ClientMessage::OptionReady(OptionReady {
            match_id: current.match_id.clone(),
            play_option: self.option_choice.unwrap_or(0),
            // The chart check answered with 7KEY, so that is the fallback.
            play_mode: option.play_mode.unwrap_or(7),
            ln_mode: if option.ln_mode.is_empty() {
                "LN".to_owned()
            } else {
                option.ln_mode.clone()
            },
        }))
    }

    /// The server's own record of the room (`arena_status.player.queue`) is the
    /// truth after a reconnect: the room is back with its code (the member list
    /// follows with the next `room_status`), or it is gone.
    fn follow_server_room(&mut self, player: &PlayerStatus) {
        if !player.queue_known {
            return;
        }
        match (&player.queue_room_code, self.room.as_mut()) {
            (Some(code), Some(room)) => room.code = Some(code.clone()),
            (Some(code), None) => {
                self.room = Some(RoomView { code: Some(code.clone()), ..RoomView::default() });
            }
            (None, Some(room)) if room.code.is_some() && self.current_match.is_none() => {
                self.room = None;
            }
            (None, _) => {}
        }
    }

    fn apply_room_status(&mut self, status: RoomStatusInfo) {
        let room = self.room.get_or_insert_with(RoomView::default);
        if status.room_code.is_some() {
            room.code = status.room_code;
        }
        room.host_id = status.room_host_id;
        room.ln_mode = status.ln_mode;
        room.players = status.players;
    }

    fn on_message(&mut self, now: Now, message: ServerMessage) -> Vec<Action> {
        if !matches!(self.phase, Phase::Authenticating | Phase::Connected) {
            return Vec::new();
        }
        let self_id = self.hello.player_id;
        self.last_rx = now.mono;
        match message {
            ServerMessage::HelloOk(ok) => {
                if self.phase != Phase::Authenticating {
                    return Vec::new();
                }
                self.phase = Phase::Connected;
                self.disconnected_since = None;
                self.last_disconnect = None;
                self.notice = None;
                self.session_id = Some(ok.session_id).filter(|id| !id.is_empty());
                self.active_match_id = Some(ok.active_match_id).filter(|id| !id.is_empty());
                // The server has no match for us, or another one: drop the local
                // one (spec 3.6). A match kept across the reconnect stays.
                if self.current_match.as_ref().map(|current| &current.match_id)
                    != self.active_match_id.as_ref()
                {
                    self.forget_match();
                }
                if let Some(server_time) = ok.server_time {
                    // A first estimate; the first pong refines it with the round trip.
                    self.clock_offset = Some(server_time - now.unix);
                }
                // Spec 3.6: `final` first, then `play_ready`, then `state`.
                let mut actions = self.resend_after_reconnect();
                actions.extend(self.state_message().into_iter().map(Action::Send));
                actions.push(self.ping(now));
                actions
            }
            ServerMessage::Pong { server_time, client_time } => {
                self.on_pong(now, server_time, client_time);
                Vec::new()
            }
            ServerMessage::Error { code, message } => {
                if let Some(kind) = refusal_kind(&code) {
                    self.phase = Phase::Refused(kind);
                    self.forget_room();
                    self.notice = Some(Notice { code, message });
                    self.retry_at = None;
                    return vec![Action::Close];
                }
                tracing::warn!(%code, %message, "arena server error");
                self.notice = Some(Notice { code, message });
                Vec::new()
            }
            ServerMessage::Replaced => {
                self.phase = Phase::Replaced;
                self.forget_room();
                self.retry_at = None;
                vec![Action::Close]
            }
            ServerMessage::ArenaStatus(player) => {
                self.follow_server_room(&player);
                self.player = Some(player);
                Vec::new()
            }
            ServerMessage::RoomStatus(status) => {
                self.apply_room_status(status);
                Vec::new()
            }
            ServerMessage::MatchReserved(reserved) => {
                // After a reconnect the server repeats `match_reserved` of the match
                // that is already loaded or playing here: keep it as it is.
                if self.current_match.as_ref().is_some_and(|current| {
                    current.match_id == reserved.match_id && current.stage.is_started()
                }) {
                    return Vec::new();
                }
                self.match_end = None;
                self.result = None;
                self.pending_chart_check = None;
                self.current_match = Some(MatchView {
                    match_id: reserved.match_id,
                    stage: MatchStage::Reserved,
                    finish_current_song: reserved.finish_current_song,
                    player_count: None,
                    max_players: None,
                    deadline: None,
                    nominated: false,
                    can_nominate: true,
                    retry_reason: String::new(),
                    submitted_count: None,
                    required_count: None,
                    revealed: None,
                    chart: None,
                    option: None,
                    prepare: None,
                    play_ready_sent: false,
                    start_at: None,
                    total_notes: None,
                    play_started: false,
                    live_seq: 0,
                    last_values: PlayValues::default(),
                    pending_final: None,
                    live_players: Vec::new(),
                    last_live_at: None,
                });
                // The match fixes the "state" the server expects (spec 3.4 step 2).
                self.sent_state = None;
                self.state_message().into_iter().map(Action::Send).collect()
            }
            ServerMessage::FillStarted { deadline, player_count, max_players } => {
                if let Some(current) = self.current_match.as_mut() {
                    current.stage = MatchStage::Filling;
                    current.deadline = deadline;
                    current.player_count = player_count;
                    current.max_players = max_players;
                }
                Vec::new()
            }
            ServerMessage::PlayersUpdated { match_id, player_ids } => {
                if let Some(current) =
                    self.current_match.as_mut().filter(|current| current.match_id == match_id)
                {
                    current.player_count = Some(player_ids.len() as u32);
                }
                Vec::new()
            }
            ServerMessage::NominationStarted(info) => {
                if let Some(current) = self.current_match_for(&info.match_id) {
                    current.stage = MatchStage::Nominating;
                    apply_nomination(current, &info, self_id);
                }
                Vec::new()
            }
            ServerMessage::NominationStatus(info) => {
                if let Some(current) = self.current_match_for(&info.match_id)
                    && current.stage == MatchStage::Nominating
                {
                    apply_nomination(current, &info, self_id);
                }
                Vec::new()
            }
            ServerMessage::NominationAccepted => {
                if let Some(current) = self.current_match.as_mut() {
                    current.nominated = true;
                }
                Vec::new()
            }
            ServerMessage::NominationsRevealed(revealed) => {
                if let Some(current) = self.current_match_for(&revealed.match_id) {
                    current.stage = MatchStage::Revealed;
                    current.revealed = Some(revealed);
                }
                Vec::new()
            }
            ServerMessage::Chart(chart) => {
                let Some(current) = self.current_match.as_mut() else {
                    return Vec::new();
                };
                if chart.match_id.as_deref().is_some_and(|id| id != current.match_id) {
                    return Vec::new();
                }
                // The same chart repeated after a reconnect: already loaded or playing.
                if current.stage.is_started()
                    && current.chart.as_ref().is_some_and(|known| known.md5 == chart.md5)
                {
                    return Vec::new();
                }
                current.stage = MatchStage::ChartChosen;
                current.option = None;
                current.prepare = None;
                current.play_ready_sent = false;
                current.start_at = None;
                self.pending_prepare = None;
                self.option_sent = false;
                self.pending_chart_check = Some(ChartCheckRequest {
                    match_id: current.match_id.clone(),
                    md5: chart.md5.clone(),
                });
                current.chart = Some(chart);
                Vec::new()
            }
            ServerMessage::OptionSelect(select) => {
                let Some(current) = self.current_match_for(&select.match_id) else {
                    return Vec::new();
                };
                // Late copies (after `prepare`) are ignored.
                if current.stage.is_started() {
                    return Vec::new();
                }
                current.stage = MatchStage::Options;
                current.deadline = select.deadline;
                current.option = Some(option_phase(&select));
                Vec::new()
            }
            ServerMessage::Prepare(prepare) => {
                let Some(current) = self.current_match_for(&prepare.match_id) else {
                    return Vec::new();
                };
                // A repeated `prepare` (after a reconnect) must not start the song again.
                if matches!(
                    current.stage,
                    MatchStage::Countdown | MatchStage::Playing | MatchStage::Finished
                ) || (current.stage == MatchStage::Loading && current.prepare.is_some())
                {
                    return Vec::new();
                }
                current.stage = MatchStage::Loading;
                current.deadline = prepare.load_deadline;
                current.play_ready_sent = false;
                current.prepare = Some(prepare.clone());
                self.pending_prepare = Some(prepare);
                Vec::new()
            }
            ServerMessage::Start(start) => {
                let Some(current) = self.current_match_for(&start.match_id) else {
                    return Vec::new();
                };
                if matches!(current.stage, MatchStage::Playing | MatchStage::Finished) {
                    return Vec::new();
                }
                current.stage = MatchStage::Countdown;
                current.start_at = start.start_at;
                Vec::new()
            }
            ServerMessage::MatchResume { match_id, state, start_at } => {
                // Only a match this client still holds can be resumed; after a
                // restart of BMZ the song cannot be rejoined (it is forfeited by the server).
                let Some(current) = self.current_match_for(&match_id) else {
                    return Vec::new();
                };
                // `playing` too: the start may have passed while the connection was
                // down. `start_at` is then in the past and the READY gate opens at once.
                if matches!(state.as_str(), "countdown" | "playing")
                    && matches!(current.stage, MatchStage::Loading | MatchStage::Countdown)
                {
                    current.stage = MatchStage::Countdown;
                    current.start_at = start_at.or(current.start_at);
                }
                Vec::new()
            }
            ServerMessage::LiveSnapshot(snapshot) => {
                if let Some(current) = self.current_match_for(&snapshot.match_id) {
                    current.live_players = snapshot.players;
                }
                Vec::new()
            }
            ServerMessage::Result(result) => {
                if self
                    .current_match
                    .as_ref()
                    .is_some_and(|current| current.match_id != result.match_id)
                {
                    return Vec::new();
                }
                // The ready request stays: the player is still in the room.
                self.forget_match();
                self.match_end = None;
                self.result = Some(result);
                self.state_message().into_iter().map(Action::Send).collect()
            }
            ServerMessage::ForfeitAccepted { match_id, reason } => {
                if self.current_match.as_ref().is_some_and(|current| current.match_id != match_id) {
                    return Vec::new();
                }
                self.forget_match();
                self.reset_ready();
                self.match_end = Some(MatchEnd::Forfeited(reason));
                self.state_message().into_iter().map(Action::Send).collect()
            }
            ServerMessage::RoomDisbanded { match_id } => {
                // The server cancels the running match (`private_room_disbanded`)
                // and removes the room. That is a cancelled match: forgetting it
                // also drops a `final` waiting to be resent, and the app lifts the
                // play locks. The song itself goes on as a normal play.
                let had_match = self.current_match.is_some() || !match_id.is_empty();
                self.forget_room();
                self.match_end = Some(if had_match {
                    MatchEnd::Cancelled(ROOM_DISBANDED_REASON.to_owned())
                } else {
                    MatchEnd::RoomDisbanded
                });
                Vec::new()
            }
            ServerMessage::RoomKicked => {
                self.forget_room();
                self.match_end = Some(MatchEnd::RoomKicked);
                Vec::new()
            }
            ServerMessage::MatchCancelled { match_id, reason } => {
                // A late message about another match must not end this one.
                if self.is_other_match(&match_id) {
                    return Vec::new();
                }
                self.forget_match();
                self.reset_ready();
                self.match_end = Some(MatchEnd::Cancelled(reason));
                self.state_message().into_iter().map(Action::Send).collect()
            }
            ServerMessage::MatchReleased { match_id } => {
                if self.is_other_match(&match_id) {
                    return Vec::new();
                }
                self.forget_match();
                self.reset_ready();
                self.match_end = Some(MatchEnd::Released);
                self.state_message().into_iter().map(Action::Send).collect()
            }
            ServerMessage::Unknown(kind) => {
                self.unknown_messages += 1;
                tracing::debug!(%kind, "ignored unknown arena message");
                Vec::new()
            }
        }
    }

    fn on_tick(&mut self, now: Now) -> Vec<Action> {
        match self.phase {
            Phase::WaitingToReconnect => {
                if self.retry_at.is_some_and(|(at, _)| now.mono >= at) {
                    self.phase = Phase::Connecting;
                    self.retry_at = None;
                }
                Vec::new()
            }
            Phase::Authenticating | Phase::Connected => {
                if now.mono.saturating_sub(self.last_rx) >= self.timing.idle_timeout {
                    self.begin_reconnect(now, DisconnectReason::IdleTimeout);
                    return vec![Action::Close];
                }
                let mut actions = Vec::new();
                if self.phase == Phase::Connected && self.option_deadline_near(now) {
                    actions.extend(self.send_option_ready().map(Action::Send));
                }
                if self.phase == Phase::Connected && now.mono >= self.next_ping {
                    actions.push(self.ping(now));
                }
                actions
            }
            _ => Vec::new(),
        }
    }

    fn ping(&mut self, now: Now) -> Action {
        self.next_ping = now.mono + self.timing.ping_interval;
        if self.pending_pings.len() >= MAX_PENDING_PINGS {
            self.pending_pings.pop_front();
        }
        self.pending_pings.push_back((now.unix, now.mono));
        Action::Send(ClientMessage::Ping { client_time: now.unix })
    }

    /// `offset = server_time - (sent_time + round_trip / 2)`.
    fn on_pong(&mut self, now: Now, server_time: Option<f64>, client_time: Option<f64>) {
        let index = match client_time {
            Some(echo) => self.pending_pings.iter().position(|(sent, _)| *sent == echo),
            None => (!self.pending_pings.is_empty()).then_some(0),
        };
        let (Some(index), Some(server_time)) = (index, server_time) else {
            return;
        };
        let Some((sent_unix, sent_mono)) = self.pending_pings.remove(index) else {
            return;
        };
        // Older pings can no longer be matched once a newer one is answered.
        self.pending_pings.drain(..index.min(self.pending_pings.len()));
        let round_trip = now.mono.saturating_sub(sent_mono).as_secs_f64();
        self.round_trip = Some(round_trip);
        self.clock_offset = Some(server_time - (sent_unix + round_trip / 2.0));
    }

    fn state_message(&mut self) -> Option<ClientMessage> {
        if self.phase != Phase::Connected {
            return None;
        }
        let ready = self.desired_ready();
        if self.sent_state == Some((self.screen, ready)) {
            return None;
        }
        self.sent_state = Some((self.screen, ready));
        Some(ClientMessage::State {
            state: self.screen,
            arena_enabled: self.hello.arena_enabled,
            ready,
        })
    }

    fn begin_reconnect(&mut self, now: Now, reason: DisconnectReason) {
        let since = *self.disconnected_since.get_or_insert(now.mono);
        let delay = if now.mono.saturating_sub(since) < self.timing.reconnect_grace {
            self.timing.reconnect_delay
        } else {
            self.timing.reconnect_delay_after_grace
        };
        self.phase = Phase::WaitingToReconnect;
        self.last_disconnect = Some(reason);
        self.session_id = None;
        self.active_match_id = None;
        self.player = None;
        // A song in progress, or a load the app already began, is kept across
        // the reconnect: its `final` and `play_ready` have to be said again, and the
        // app keeps going (dropping it would let READY run without the start gate).
        let kept = self.current_match.take().filter(|current| {
            matches!(
                current.stage,
                MatchStage::Countdown | MatchStage::Playing | MatchStage::Finished
            ) || (current.stage == MatchStage::Loading && current.prepare.is_some())
        });
        // A `prepare` the app has not taken yet goes with its match.
        let pending_prepare = self.pending_prepare.take().filter(|prepare| {
            kept.as_ref().is_some_and(|current| current.match_id == prepare.match_id)
        });
        self.forget_room();
        self.current_match = kept;
        self.pending_prepare = pending_prepare;
        self.retry_at = Some((now.mono + delay, now.unix + delay.as_secs_f64()));
    }
}

fn live_report(match_id: String, seq: u32, values: PlayValues, prepare: &Prepare) -> LiveReport {
    LiveReport {
        match_id,
        seq,
        exscore: values.exscore,
        processed_notes: values.processed_notes,
        minbp: values.minbp,
        combo_break: values.combo_break,
        max_combo: values.max_combo,
        // The confirmed option from `prepare`; it must not change during the match.
        play_option: prepare.play_option,
        play_mode: prepare.play_mode.unwrap_or(7),
        ln_mode: prepare.ln_mode.clone(),
    }
}

fn option_phase(select: &OptionSelect) -> OptionPhase {
    OptionPhase {
        deadline: select.deadline,
        ready: select.ready,
        ready_count: select.ready_count,
        player_count: select.player_count,
        play_mode: select.play_mode,
        ln_mode: select.ln_mode.clone(),
    }
}

/// Copies a `nomination_started` / `nomination_status` payload into the view.
/// "Nominated" comes from the server's own bookkeeping (this player's
/// `submitted`, or the counts against the required count), so a restarted
/// nomination (`retry_reason`) opens again.
fn apply_nomination(current: &mut MatchView, info: &NominationInfo, self_id: u64) {
    current.deadline = info.deadline;
    current.can_nominate = info.can_nominate.unwrap_or(true);
    current.retry_reason = info.retry_reason.clone();
    current.submitted_count = info.submitted_count;
    current.required_count = info.required_count;
    let own = info.players.iter().find(|player| player.player_id == self_id);
    current.nominated = match (own, info.required_count, info.submitted_count) {
        (Some(player), _, _) => player.submitted,
        (None, Some(required), Some(submitted)) => required > 0 && submitted >= required,
        _ => info.your_nomination.is_some(),
    };
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "match_setup_tests.rs"]
mod match_setup_tests;

#[cfg(test)]
#[path = "load_start_tests.rs"]
mod load_start_tests;

#[cfg(test)]
#[path = "live_final_tests.rs"]
mod live_final_tests;

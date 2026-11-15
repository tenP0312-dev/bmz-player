//! Arena connection state machine.
//!
//! No I/O and no clock of its own: the connection thread feeds it events with
//! the current time and performs the returned actions. That keeps every rule of
//! the foundation (hello, keep-alive, idle cut-off, reconnect, refusals,
//! `replaced`) testable without a network.

use std::collections::VecDeque;
use std::time::Duration;

use super::protocol::{ClientMessage, Hello, PlayerStatus, ScreenState, ServerMessage};

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

/// Everything the panel needs, copied out after each step.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArenaPanelState {
    pub status: ConnectionStatus,
    pub session_id: Option<String>,
    /// Server time minus local time, in seconds.
    pub clock_offset_secs: Option<f64>,
    pub round_trip_secs: Option<f64>,
    /// A match the server says is in progress (shown only; B1 cannot rejoin it).
    pub active_match_id: Option<String>,
    pub notice: Option<Notice>,
    /// The player's own rating from the latest `arena_status`.
    pub player: Option<PlayerStatus>,
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

    /// `ready` is always false in the foundation: it needs a room (B2).
    fn desired_ready(&self) -> bool {
        false
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
            clock_offset_secs: self.clock_offset,
            round_trip_secs: self.round_trip,
            active_match_id: self.active_match_id.clone(),
            notice: self.notice.clone(),
            player: self.player,
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
                self.screen = screen;
                self.state_message().into_iter().map(Action::Send).collect()
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
        }
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

    fn on_message(&mut self, now: Now, message: ServerMessage) -> Vec<Action> {
        if !matches!(self.phase, Phase::Authenticating | Phase::Connected) {
            return Vec::new();
        }
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
                if let Some(server_time) = ok.server_time {
                    // A first estimate; the first pong refines it with the round trip.
                    self.clock_offset = Some(server_time - now.unix);
                }
                let mut actions: Vec<Action> =
                    self.state_message().into_iter().map(Action::Send).collect();
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
                self.retry_at = None;
                vec![Action::Close]
            }
            ServerMessage::ArenaStatus(player) => {
                self.player = Some(player);
                Vec::new()
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
                if self.phase == Phase::Connected && now.mono >= self.next_ping {
                    return vec![self.ping(now)];
                }
                Vec::new()
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
        self.retry_at = Some((now.mono + delay, now.unix + delay.as_secs_f64()));
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

//! Arena connection thread.
//!
//! One dedicated thread runs a current-thread tokio runtime, so neither the
//! window thread nor the gameplay thread ever waits on the network. The window
//! thread talks to it through a command channel and reads the panel state from
//! a shared snapshot. All rules live in [`ArenaSession`]; this file only moves
//! bytes and time.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use super::protocol::{Hello, ScreenState, parse_server_message};
use super::session::{
    Action, ArenaPanelState, ArenaSession, ConnectionStatus, Event, Now, Phase, Timing,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
const TICK_INTERVAL: Duration = Duration::from_millis(250);
/// The server never sends anything near this size to a foundation client.
const MAX_INCOMING_BYTES: usize = 1 << 20;

#[derive(Debug)]
enum Command {
    SetScreen(ScreenState),
    Reconnect,
    /// A room, nomination or chart-check event for the session.
    Session(Event),
    Shutdown,
}

type SharedPanel = Arc<Mutex<ArenaPanelState>>;

/// Cheap, cloneable view of the connection for the window thread and the UI.
#[derive(Clone)]
pub struct ArenaHandle {
    panel: SharedPanel,
    commands: Option<UnboundedSender<Command>>,
}

impl ArenaHandle {
    pub fn panel(&self) -> ArenaPanelState {
        self.panel.lock().unwrap_or_else(|error| error.into_inner()).clone()
    }

    pub fn set_screen(&self, screen: ScreenState) {
        self.send(Command::SetScreen(screen));
    }

    /// Connect again after a refusal or `replaced`.
    pub fn reconnect(&self) {
        self.send(Command::Reconnect);
    }

    /// Create a private room (the fixed foundation settings).
    pub fn create_room(&self) {
        self.send(Command::Session(Event::RoomCreate));
    }

    pub fn join_room(&self, code: String, password: Option<String>) {
        self.send(Command::Session(Event::RoomJoin { code, password }));
    }

    /// The `play_option` of the player's current options (for the option phase).
    pub fn set_option_choice(&self, play_option: u32) {
        self.send(Command::Session(Event::SetOptionChoice(play_option)));
    }

    /// READY in the option phase (`option_ready`).
    pub fn option_ready(&self) {
        self.send(Command::Session(Event::OptionReadyNow));
    }

    pub fn prepare_taken(&self, match_id: String) {
        self.send(Command::Session(Event::PrepareTaken { match_id }));
    }

    /// The chart is loaded and waits for READY (`play_ready`).
    pub fn play_loaded(&self, match_id: String, md5: String, totalnotes: u32) {
        self.send(Command::Session(Event::PlayLoaded { match_id, md5, totalnotes }));
    }

    /// The chart could not be started: leave the match (`forfeit`).
    pub fn load_failed(&self, match_id: String) {
        self.send(Command::Session(Event::LoadFailed { match_id }));
    }

    /// Leave the room (`queue_cancel`).
    pub fn leave_room(&self) {
        self.send(Command::Session(Event::RoomLeave));
    }

    /// Host: send all room settings again (`room_settings`).
    pub fn apply_room_settings(&self) {
        self.send(Command::Session(Event::RoomSettingsApply));
    }

    pub fn set_ready(&self, ready: bool) {
        self.send(Command::Session(Event::SetReady(ready)));
    }

    /// Nominate a chart (lowercase hex MD5) while nominations are open.
    pub fn nominate(&self, md5: String) {
        self.send(Command::Session(Event::Nominate(md5)));
    }

    pub fn skip_nomination(&self) {
        self.send(Command::Session(Event::SkipNomination));
    }

    /// Answers [`ArenaPanelState::pending_chart_check`].
    pub fn report_chart_check(
        &self,
        match_id: String,
        md5: String,
        available: bool,
        totalnotes: u32,
        play_mode: u32,
    ) {
        self.send(Command::Session(Event::ChartChecked {
            match_id,
            md5,
            available,
            totalnotes,
            play_mode,
        }));
    }

    /// The chart under the select cursor (lowercase hex MD5), for nominating.
    pub fn set_selected_chart(&self, md5: Option<String>) {
        self.send(Command::Session(Event::SetSelectedChart(md5)));
    }

    fn send(&self, command: Command) {
        if let Some(commands) = &self.commands {
            let _ = commands.send(command);
        }
    }
}

/// Owner of the Arena connection thread.
pub struct ArenaController {
    handle: ArenaHandle,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ArenaController {
    /// No connection. `status` says why (off, no login, unsupported rule).
    pub fn inactive(status: ConnectionStatus) -> Self {
        Self {
            handle: ArenaHandle {
                panel: Arc::new(Mutex::new(ArenaPanelState {
                    status,
                    ..ArenaPanelState::default()
                })),
                commands: None,
            },
            thread: None,
        }
    }

    /// Starts the connection thread. `url` must already have passed
    /// `fork_policy::ensure_arena_url_allowed`.
    pub fn spawn(url: String, hello: Hello, screen: ScreenState) -> Self {
        Self::spawn_with_timing(url, hello, screen, Timing::default())
    }

    pub(super) fn spawn_with_timing(
        url: String,
        hello: Hello,
        screen: ScreenState,
        timing: Timing,
    ) -> Self {
        let panel: SharedPanel = Arc::new(Mutex::new(ArenaPanelState {
            status: ConnectionStatus::Connecting,
            ..ArenaPanelState::default()
        }));
        let (commands, receiver) = unbounded_channel();
        let thread_panel = Arc::clone(&panel);
        let thread = std::thread::Builder::new().name("bmz-arena".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(%error, "failed to start the arena runtime");
                    return;
                }
            };
            runtime.block_on(Driver::new(url, hello, screen, timing, thread_panel).run(receiver));
        });
        match thread {
            Ok(thread) => Self {
                handle: ArenaHandle { panel, commands: Some(commands) },
                thread: Some(thread),
            },
            Err(error) => {
                tracing::error!(%error, "failed to spawn the arena thread");
                Self::inactive(ConnectionStatus::Disabled)
            }
        }
    }

    pub fn handle(&self) -> ArenaHandle {
        self.handle.clone()
    }

    pub fn panel(&self) -> ArenaPanelState {
        self.handle.panel()
    }

    pub fn set_screen(&self, screen: ScreenState) {
        self.handle.set_screen(screen);
    }

    pub fn reconnect(&self) {
        self.handle.reconnect();
    }

    /// Stops the thread and waits for it (it reacts within a second).
    pub fn shutdown(mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    fn stop(&mut self) {
        self.handle.send(Command::Shutdown);
    }
}

impl Drop for ArenaController {
    fn drop(&mut self) {
        // Never block the caller (usually the window thread); the thread ends by itself.
        self.stop();
    }
}

#[derive(Debug, PartialEq, Eq)]
enum IoExit {
    /// Back to the session's decision (reconnect, refusal, ...).
    Done,
    Shutdown,
}

struct Driver {
    url: String,
    session: ArenaSession,
    panel: SharedPanel,
    started: Instant,
}

impl Driver {
    fn new(
        url: String,
        hello: Hello,
        screen: ScreenState,
        timing: Timing,
        panel: SharedPanel,
    ) -> Self {
        Self {
            url,
            session: ArenaSession::with_timing(hello, screen, timing),
            panel,
            started: Instant::now(),
        }
    }

    fn now(&self) -> Now {
        let unix = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
        Now { mono: self.started.elapsed(), unix }
    }

    fn feed(&mut self, event: Event) -> Vec<Action> {
        let now = self.now();
        let actions = self.session.handle(now, event);
        *self.panel.lock().unwrap_or_else(|error| error.into_inner()) = self.session.panel();
        actions
    }

    /// Applies a command that does not need a socket. Returns false on shutdown.
    fn apply_idle_command(&mut self, command: Option<Command>) -> bool {
        match command {
            None | Some(Command::Shutdown) => false,
            Some(Command::SetScreen(screen)) => {
                self.feed(Event::SetScreen(screen));
                true
            }
            Some(Command::Reconnect) => {
                self.feed(Event::ManualReconnect);
                true
            }
            Some(Command::Session(event)) => {
                self.feed(event);
                true
            }
        }
    }

    async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        let mut tick = tokio::time::interval(TICK_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        *self.panel.lock().unwrap_or_else(|error| error.into_inner()) = self.session.panel();
        loop {
            match self.session.phase() {
                Phase::Connecting => {
                    let config = WebSocketConfig {
                        max_message_size: Some(MAX_INCOMING_BYTES),
                        max_frame_size: Some(MAX_INCOMING_BYTES),
                        ..WebSocketConfig::default()
                    };
                    let url = self.url.clone();
                    let socket = {
                        let connect = tokio::time::timeout(
                            CONNECT_TIMEOUT,
                            connect_async_with_config(url.as_str(), Some(config), false),
                        );
                        tokio::pin!(connect);
                        loop {
                            tokio::select! {
                                result = &mut connect => break result,
                                command = commands.recv() => {
                                    if !self.apply_idle_command(command) {
                                        return;
                                    }
                                }
                            }
                        }
                    };
                    match socket {
                        Ok(Ok((socket, _response))) => {
                            let actions = self.feed(Event::SocketOpened);
                            if self.drive_socket(socket, actions, &mut commands).await
                                == IoExit::Shutdown
                            {
                                return;
                            }
                        }
                        Ok(Err(error)) => {
                            tracing::info!(%error, "arena connect failed");
                            self.feed(Event::SocketClosed);
                        }
                        Err(_) => {
                            tracing::info!("arena connect timed out");
                            self.feed(Event::SocketClosed);
                        }
                    }
                }
                Phase::WaitingToReconnect | Phase::Refused(_) | Phase::Replaced => {
                    tokio::select! {
                        _ = tick.tick() => { self.feed(Event::Tick); }
                        command = commands.recv() => {
                            if !self.apply_idle_command(command) {
                                return;
                            }
                        }
                    }
                }
                // `drive_socket` only returns once the session has left these.
                Phase::Authenticating | Phase::Connected => {
                    self.feed(Event::SocketClosed);
                }
            }
        }
    }

    async fn drive_socket<S>(
        &mut self,
        socket: tokio_tungstenite::WebSocketStream<S>,
        first_actions: Vec<Action>,
        commands: &mut UnboundedReceiver<Command>,
    ) -> IoExit
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (mut sink, mut stream) = socket.split();
        let mut tick = tokio::time::interval(TICK_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut pending = first_actions;
        loop {
            // Perform what the session asked for before waiting again.
            for action in std::mem::take(&mut pending) {
                match action {
                    Action::Send(message) => {
                        let text = match message.to_json() {
                            Ok(text) => text,
                            Err(error) => {
                                tracing::error!(%error, "arena message not sent");
                                continue;
                            }
                        };
                        if let Err(error) = sink.send(Message::Text(text)).await {
                            tracing::info!(%error, "arena send failed");
                            self.feed(Event::SocketClosed);
                            return IoExit::Done;
                        }
                    }
                    Action::Close => {
                        let _ =
                            tokio::time::timeout(CLOSE_TIMEOUT, sink.send(Message::Close(None)))
                                .await;
                        return IoExit::Done;
                    }
                }
            }
            tokio::select! {
                incoming = stream.next() => match incoming {
                    Some(Ok(Message::Text(text))) => match parse_server_message(&text) {
                        Ok(message) => pending = self.feed(Event::Message(message)),
                        Err(error) => tracing::warn!(%error, "ignored malformed arena message"),
                    },
                    Some(Ok(Message::Close(_))) | None => {
                        self.feed(Event::SocketClosed);
                        return IoExit::Done;
                    }
                    Some(Err(error)) => {
                        tracing::info!(%error, "arena socket error");
                        self.feed(Event::SocketClosed);
                        return IoExit::Done;
                    }
                    // WebSocket ping/pong frames are answered by tungstenite; binary is not used.
                    Some(Ok(_)) => {}
                },
                command = commands.recv() => match command {
                    None | Some(Command::Shutdown) => {
                        let _ = tokio::time::timeout(CLOSE_TIMEOUT, sink.send(Message::Close(None))).await;
                        return IoExit::Shutdown;
                    }
                    Some(Command::SetScreen(screen)) => pending = self.feed(Event::SetScreen(screen)),
                    Some(Command::Reconnect) => { self.feed(Event::ManualReconnect); }
                    Some(Command::Session(event)) => pending = self.feed(event),
                },
                _ = tick.tick() => pending = self.feed(Event::Tick),
            }
        }
    }
}

#[cfg(test)]
#[path = "connection_tests.rs"]
mod tests;

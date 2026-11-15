//! BMS-IR Arena: connection foundation (stage B1).
//!
//! Arena is a persistent match service, so it is not an IR provider (those are
//! upload queues). This stage only connects, authenticates, keeps the link alive
//! and tells the server which screen the player is on. Rooms, nominations, chart
//! hand-over, live/final scores and results come in later stages.
//!
//! * [`protocol`]: message types and JSON conversion.
//! * [`session`]: the I/O-free state machine.
//! * [`connection`]: the dedicated tokio thread and its window-thread handle.
//!
//! Arena only ever connects to BMS-IR (`fork_policy::ensure_arena_url_allowed`).
//! It never touches the gameplay thread, score storage or the IR provider queue.

pub mod chart_lookup;
pub mod connection;
pub mod launch;
pub mod options;
pub mod protocol;
pub mod session;

use std::path::Path;

use anyhow::{Context, Result};
use bmz_gameplay::rule::RuleMode;

pub use connection::ArenaController;
pub use protocol::ScreenState;
pub use session::{ArenaPanelState, ConnectionStatus, RefusalKind};

use protocol::{
    ARENA_CLIENT_VERSION, ARENA_WS_PATH, CLIENT_FLAVOR, Hello, PROTOCOL_VERSION, RulesetProfile,
    passmd5,
};

const MAX_BUILD_HASH_LEN: usize = 128;

/// `wss://<bms-ir host>/new/arena/ws/client` for the compiled-in BMS-IR base URL.
/// A local integration build (`BMZ_BMS_IR_BASE_URL`) maps `http` to `ws`.
pub fn arena_ws_url(base_url: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(base_url.trim()).context("invalid BMS-IR base URL")?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        other => anyhow::bail!("unsupported BMS-IR URL scheme: {other}"),
    };
    url.set_scheme(scheme).map_err(|()| anyhow::anyhow!("invalid BMS-IR base URL"))?;
    url.set_path(ARENA_WS_PATH);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

/// The `hello` for a stored BMS-IR login. `account_id` is the numeric player ID
/// and `game_token` the BMS-IR game token; the token itself is never sent.
pub fn build_hello(
    account_id: &str,
    game_token: &str,
    rule_mode: RuleMode,
) -> Result<Hello, ConnectionStatus> {
    let ruleset_profile =
        RulesetProfile::from_rule_mode(rule_mode).ok_or(ConnectionStatus::UnsupportedRule)?;
    let player_id =
        account_id.trim().parse::<u64>().map_err(|_| ConnectionStatus::NoCredentials)?;
    if game_token.trim().is_empty() {
        return Err(ConnectionStatus::NoCredentials);
    }
    let mut build_hash = env!("BMZ_BUILD_COMMIT").to_owned();
    build_hash.truncate(MAX_BUILD_HASH_LEN);
    Ok(Hello {
        protocol: PROTOCOL_VERSION,
        player_id,
        passmd5: passmd5(game_token),
        client_version: ARENA_CLIENT_VERSION.to_owned(),
        build_hash,
        body_version: env!("CARGO_PKG_VERSION").to_owned(),
        client_flavor: CLIENT_FLAVOR.to_owned(),
        ruleset_profile,
        arena_enabled: true,
    })
}

/// Starts Arena for a profile, or returns an inactive controller saying why not.
/// Reads the BMS-IR login from the credential store; never writes it.
pub fn start(
    profile_root: &Path,
    enabled: bool,
    rule_mode: RuleMode,
    screen: ScreenState,
) -> ArenaController {
    if !enabled {
        return ArenaController::inactive(ConnectionStatus::Disabled);
    }
    let base_url = crate::ir::bms_ir::BMS_IR_DEFAULT_BASE_URL;
    let credentials = match crate::ir::credentials::load_credentials(
        profile_root,
        crate::ir::bms_ir::BMS_IR_PROVIDER,
        base_url,
    ) {
        Ok(Some(credentials)) => credentials,
        Ok(None) => return ArenaController::inactive(ConnectionStatus::NoCredentials),
        Err(error) => {
            tracing::warn!(%error, "arena: failed to read the BMS-IR login");
            return ArenaController::inactive(ConnectionStatus::NoCredentials);
        }
    };
    let hello = match build_hello(&credentials.account_id, &credentials.access_token, rule_mode) {
        Ok(hello) => hello,
        Err(status) => return ArenaController::inactive(status),
    };
    let url = match arena_ws_url(base_url) {
        Ok(url) => url,
        Err(error) => {
            tracing::error!(%error, "arena: no connection URL");
            return ArenaController::inactive(ConnectionStatus::Disabled);
        }
    };
    if let Err(error) = crate::fork_policy::ensure_arena_url_allowed(&url) {
        tracing::error!(%error, "arena: connection refused by the BMS-IR-only policy");
        return ArenaController::inactive(ConnectionStatus::Disabled);
    }
    ArenaController::spawn(url, hello, screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_url_is_wss_on_the_bms_ir_host() {
        assert_eq!(
            arena_ws_url("https://www.bms-ir.org").unwrap(),
            "wss://www.bms-ir.org/new/arena/ws/client"
        );
        assert_eq!(
            arena_ws_url("https://www.bms-ir.org/").unwrap(),
            "wss://www.bms-ir.org/new/arena/ws/client"
        );
        assert_eq!(
            arena_ws_url("http://127.0.0.1:8080").unwrap(),
            "ws://127.0.0.1:8080/new/arena/ws/client"
        );
        assert!(arena_ws_url("ftp://www.bms-ir.org").is_err());
        assert!(arena_ws_url("nope").is_err());
    }

    #[test]
    fn compiled_in_endpoint_passes_the_fork_policy() {
        let url = arena_ws_url(crate::ir::bms_ir::BMS_IR_DEFAULT_BASE_URL).unwrap();
        crate::fork_policy::ensure_arena_url_allowed(&url).unwrap();
    }

    #[test]
    fn hello_uses_the_documented_values_and_hides_the_token() {
        let hello = build_hello("123", "tok", RuleMode::Lr2Oraja).unwrap();
        assert_eq!(hello.protocol, 8);
        assert_eq!(hello.player_id, 123);
        assert_eq!(hello.passmd5, passmd5("tok"));
        assert_ne!(hello.passmd5, "tok");
        assert_eq!(hello.client_flavor, "bmz-player");
        assert_eq!(hello.ruleset_profile, RulesetProfile::Lr2);
        assert_eq!(hello.body_version, env!("CARGO_PKG_VERSION"));
        assert!(hello.client_version.len() <= 64);
        assert!(hello.build_hash.len() <= 128);
        assert!(hello.arena_enabled);
        assert_eq!(
            build_hello("123", "tok", RuleMode::Beatoraja).unwrap().ruleset_profile,
            RulesetProfile::Oraja
        );
    }

    #[test]
    fn unusable_logins_and_rules_are_reported_not_sent() {
        assert_eq!(build_hello("123", "tok", RuleMode::Dx), Err(ConnectionStatus::UnsupportedRule));
        assert_eq!(
            build_hello("not-a-number", "tok", RuleMode::Lr2Oraja),
            Err(ConnectionStatus::NoCredentials)
        );
        assert_eq!(
            build_hello("1", "  ", RuleMode::Lr2Oraja),
            Err(ConnectionStatus::NoCredentials)
        );
    }

    #[test]
    fn off_by_default_and_off_means_inactive() {
        assert!(!crate::config::profile_config::ArenaConfig::default().enabled);
        let root = std::env::temp_dir().join("bmz-arena-off-test");
        let controller = start(&root, false, RuleMode::Lr2Oraja, ScreenState::Select);
        assert_eq!(controller.panel().status, ConnectionStatus::Disabled);
    }

    #[test]
    fn no_login_means_no_connection() {
        let root = std::env::temp_dir().join(format!("bmz-arena-nologin-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let controller = start(&root, true, RuleMode::Lr2Oraja, ScreenState::Select);
        assert_eq!(controller.panel().status, ConnectionStatus::NoCredentials);
        std::fs::remove_dir_all(root).unwrap();
    }
}

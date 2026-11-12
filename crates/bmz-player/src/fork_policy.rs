//! BMS-IR fork: outbound-connection policy.
//!
//! Builds from this fork talk to BMS-IR only. Every IR client constructor checks
//! [`ensure_ir_origin_allowed`], and the upstream online features that send player
//! data or contact the upstream distribution channel are switched off at compile
//! time. These are constants, not settings: a profile or app config cannot turn
//! them back on. See `docs/bmsir/BMSIR_ONLY.md` for the full list.

use anyhow::{Result, bail};

/// Upstream release check / update download (GitHub `hyrorre/bmz-player`).
pub const ONLINE_UPDATES_ENABLED: bool = false;

/// Discord Rich Presence (publishes the song being played to Discord).
pub const DISCORD_PRESENCE_ENABLED: bool = false;

/// Whether `base_url` has the origin of the compiled-in BMS-IR endpoint.
pub fn is_bms_ir_origin(base_url: &str) -> bool {
    let (Ok(url), Ok(fixed)) = (
        reqwest::Url::parse(base_url.trim()),
        reqwest::Url::parse(crate::ir::bms_ir::BMS_IR_DEFAULT_BASE_URL),
    ) else {
        return false;
    };
    url.origin() == fixed.origin()
}

/// Loopback and reserved test hosts. Only reachable in unit tests, where mock
/// servers stand in for the IR; release builds never accept them.
#[cfg(test)]
fn is_test_host(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    match url.host_str() {
        Some(host) => {
            host == "localhost"
                || host == "127.0.0.1"
                || host == "[::1]"
                || host.ends_with(".test")
                || host.ends_with(".example")
        }
        None => false,
    }
}

#[cfg(not(test))]
fn is_test_host(_base_url: &str) -> bool {
    false
}

/// Fails unless `base_url` is the BMS-IR origin. Called before any IR HTTP client
/// is built so that no request for another IR can be constructed.
pub fn ensure_ir_origin_allowed(base_url: &str, service: &str) -> Result<()> {
    if is_bms_ir_origin(base_url) || is_test_host(base_url) {
        return Ok(());
    }
    bail!("{service} is not available in this build; only BMS-IR can be used")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_bms_ir_origin_is_allowed() {
        assert!(is_bms_ir_origin(crate::ir::bms_ir::BMS_IR_DEFAULT_BASE_URL));
        assert!(!is_bms_ir_origin("https://rianir.link/"));
        assert!(!is_bms_ir_origin("https://ir.bmz.example/"));
        assert!(!is_bms_ir_origin("not a url"));
        assert!(!is_bms_ir_origin(""));
    }

    #[test]
    fn other_services_are_refused_with_a_clear_error() {
        let error = ensure_ir_origin_allowed("https://rianir.link/", "rianIR").unwrap_err();
        assert!(error.to_string().contains("only BMS-IR"));
        assert!(
            ensure_ir_origin_allowed(crate::ir::bms_ir::BMS_IR_DEFAULT_BASE_URL, "BMS-IR").is_ok()
        );
    }

    #[test]
    fn online_features_are_compiled_out() {
        assert!(!ONLINE_UPDATES_ENABLED);
        assert!(!DISCORD_PRESENCE_ENABLED);
    }
}

//! Answers a `chart_check` from the local song library (read-only).

use std::path::Path;

use crate::storage::common::hex_to_hash;
use crate::storage::library_db::LibraryDatabase;

/// What the library knows about a candidate chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChartAnswer {
    pub available: bool,
    /// 0 when unknown (spec: "分からなければ 0").
    pub totalnotes: u32,
    /// 5, 7, 9, 10 or 14; 0 when the chart is not available.
    pub play_mode: u32,
}

/// Arena `play_mode` for a library `key_mode` label (`KeyMode::as_str`).
/// 4K, 6K and 8K have no Arena `play_mode`.
pub fn play_mode_for_key_mode(key_mode: &str) -> Option<u32> {
    match key_mode {
        "5K" => Some(5),
        "7K" => Some(7),
        "9K" => Some(9),
        "10K" => Some(10),
        "14K" => Some(14),
        _ => None,
    }
}

/// The foundation plays 7KEY only (decision 9), so any other mode is "not available".
pub fn answer_for_mode(key_mode: &str, total_notes: u32) -> ChartAnswer {
    match play_mode_for_key_mode(key_mode) {
        Some(7) => ChartAnswer { available: true, totalnotes: total_notes, play_mode: 7 },
        _ => ChartAnswer::default(),
    }
}

/// Looks the chart up by MD5. Any failure is "not available".
pub fn check_chart(library_db: &Path, md5_hex: &str) -> ChartAnswer {
    let Ok(md5) = hex_to_hash::<16>(md5_hex) else {
        return ChartAnswer::default();
    };
    let database = match LibraryDatabase::open_read_only(library_db) {
        Ok(database) => database,
        Err(error) => {
            tracing::warn!(%error, "arena: song library not readable for chart_check");
            return ChartAnswer::default();
        }
    };
    let Ok(Some(sha256)) = database.chart_sha256_by_md5(md5) else {
        return ChartAnswer::default();
    };
    match database.list_charts_by_sha256(sha256) {
        Ok(charts) => charts
            .first()
            .map(|chart| answer_for_mode(&chart.mode, chart.total_notes))
            .unwrap_or_default(),
        Err(_) => ChartAnswer::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_mode_follows_the_key_mode_label() {
        assert_eq!(play_mode_for_key_mode("7K"), Some(7));
        assert_eq!(play_mode_for_key_mode("14K"), Some(14));
        assert_eq!(play_mode_for_key_mode("5K"), Some(5));
        assert_eq!(play_mode_for_key_mode("9K"), Some(9));
        assert_eq!(play_mode_for_key_mode("10K"), Some(10));
        assert_eq!(play_mode_for_key_mode("4K"), None);
        assert_eq!(play_mode_for_key_mode("8K"), None);
    }

    #[test]
    fn only_7k_charts_are_available_in_the_foundation() {
        assert_eq!(
            answer_for_mode("7K", 1234),
            ChartAnswer { available: true, totalnotes: 1234, play_mode: 7 }
        );
        assert_eq!(answer_for_mode("14K", 2000), ChartAnswer::default());
        assert_eq!(answer_for_mode("5K", 500), ChartAnswer::default());
    }

    #[test]
    fn an_unreadable_library_or_a_bad_hash_is_not_available() {
        let missing = Path::new("/nonexistent/library.db");
        assert_eq!(check_chart(missing, &"0".repeat(32)), ChartAnswer::default());
        assert_eq!(check_chart(missing, "not-hex"), ChartAnswer::default());
    }
}

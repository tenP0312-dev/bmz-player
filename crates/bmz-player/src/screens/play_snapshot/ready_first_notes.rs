//! READY-only preview of the first notes to hit.
//!
//! While the Play scene is in READY (chart time below zero) and the profile option is on,
//! the render snapshot shows only the lanes' first hit notes, parked at the top of the
//! lanes (or just under SUDDEN+). Falling notes are not drawn for that time. The preview
//! is display-only: it never touches the judge, the score, replays or the session.
use bmz_render::snapshot::{NoteVisualKind, VisibleNote};

use super::*;

/// One full dim -> bright -> dim cycle.
pub const READY_PULSE_PERIOD_MS: i64 = 1000;
const PULSE_MIN_ALPHA: f32 = 0.65;
/// Approximate note thickness as a fraction of the lane height. The snapshot only carries
/// the note's bottom edge, and the skin owns the real thickness, so the preview is parked
/// this far below the top edge to keep a typical note inside the lane.
pub const READY_NOTE_THICKNESS: f32 = 0.04;

/// The lanes whose first hit note is at the earliest hit time of the chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyFirstNotes {
    lanes: Vec<(Lane, TimeUs)>,
}

impl ReadyFirstNotes {
    /// Mines, INVISIBLE notes and long-note ends are not hit notes; a long-note start is.
    /// Returns `None` when the chart has no hit note, so the normal drawing stays.
    pub fn from_chart(chart: &PlayableChart) -> Option<Self> {
        let mut per_lane: Vec<(Lane, TimeUs)> = Vec::new();
        for lane in Lane::ALL {
            let first = chart
                .notes_for_lane(lane)
                .iter()
                .filter(|note| matches!(note.kind, NoteKind::Tap | NoteKind::LongStart))
                .map(|note| note.time)
                .min();
            if let Some(time) = first {
                per_lane.push((lane, time));
            }
        }
        let earliest = per_lane.iter().map(|&(_, time)| time).min()?;
        per_lane.retain(|&(_, time)| time == earliest);
        Some(Self { lanes: per_lane })
    }

    #[cfg(test)]
    pub fn lanes(&self) -> impl Iterator<Item = Lane> + '_ {
        self.lanes.iter().map(|&(lane, _)| lane)
    }

    /// Replaces the lanes' visible objects with the preview. `lane_cover` is the amount the
    /// snapshot already reports (0 when SUDDEN+ is not shown) and `lift` the LIFT amount.
    pub(super) fn apply(
        &self,
        snapshot: &mut RenderSnapshot,
        chart_now: TimeUs,
        lane_cover: f32,
        lift: f32,
    ) {
        for notes in &mut snapshot.visible_notes {
            notes.clear();
        }
        for mines in &mut snapshot.visible_mines {
            mines.clear();
        }
        snapshot.visible_long_notes.clear();
        let y = first_note_y(lane_cover, lift);
        let alpha = pulse_alpha(chart_now.0.div_euclid(1_000));
        for &(lane, time) in &self.lanes {
            snapshot.visible_notes[lane.index()].push(VisibleNote {
                lane,
                time,
                y,
                alpha,
                kind: NoteVisualKind::Tap,
                processed_judge: None,
                pms_miss_fall: None,
            });
        }
    }
}

/// Progress of the top of the uncovered lane: 1.0 without SUDDEN+, the cover's lower edge
/// with it. Mirrors the renderer's own cover geometry. Both amounts are clamped to 0..=1.
fn uncovered_top_progress(lane_cover: f32, lift: f32) -> f32 {
    let lift = lift.clamp(0.0, 1.0);
    let visible = (1.0 - lift).max(f32::EPSILON);
    ((visible - lane_cover.clamp(0.0, visible)) / visible).clamp(0.0, 1.0)
}

/// Where the preview notes sit: just under the cover (or the lane top), by one note thickness.
pub fn first_note_y(lane_cover: f32, lift: f32) -> f32 {
    (uncovered_top_progress(lane_cover, lift) - READY_NOTE_THICKNESS).clamp(0.0, 1.0)
}

/// 65% at 0 ms, 100% at 500 ms, 65% again at 1000 ms.
pub fn pulse_alpha(elapsed_ms: i64) -> f32 {
    let phase = elapsed_ms.rem_euclid(READY_PULSE_PERIOD_MS) as f32 / READY_PULSE_PERIOD_MS as f32;
    let wave = 0.5 * (1.0 - (std::f32::consts::TAU * phase).cos());
    PULSE_MIN_ALPHA + (1.0 - PULSE_MIN_ALPHA) * wave
}

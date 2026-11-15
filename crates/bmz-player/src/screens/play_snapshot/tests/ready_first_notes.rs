use super::*;
use crate::screens::play_snapshot::ready_first_notes::{
    READY_NOTE_THICKNESS, ReadyFirstNotes, first_note_y, pulse_alpha,
};
use bmz_audio::clock::AudioClock;
use std::sync::atomic::AtomicU64;

fn note(id: u32, lane: Lane, kind: NoteKind, time_us: i64) -> NoteEvent {
    NoteEvent { kind, ..tap_note(id, lane, 0, time_us) }
}

fn chart_with(notes: Vec<NoteEvent>) -> PlayableChart {
    let mut chart = chart();
    chart.lane_notes = std::array::from_fn(|_| Vec::new());
    for note in notes {
        chart.lane_notes[note.lane.index()].push(note);
    }
    chart
}

fn lanes(chart: &PlayableChart) -> Option<Vec<Lane>> {
    ReadyFirstNotes::from_chart(chart).map(|first| first.lanes().collect())
}

#[test]
fn only_the_earliest_hit_time_is_used() {
    let chart = chart_with(vec![
        note(1, Lane::Key1, NoteKind::Tap, 1_000_000),
        note(2, Lane::Key2, NoteKind::Tap, 1_500_000),
        note(3, Lane::Key1, NoteKind::Tap, 2_000_000),
    ]);
    assert_eq!(lanes(&chart), Some(vec![Lane::Key1]));
}

#[test]
fn simultaneous_notes_and_the_scratch_are_all_included() {
    let chart = chart_with(vec![
        note(1, Lane::Scratch, NoteKind::Tap, 1_000_000),
        note(2, Lane::Key3, NoteKind::Tap, 1_000_000),
        note(3, Lane::Key5, NoteKind::Tap, 1_000_000),
        note(4, Lane::Key2, NoteKind::Tap, 1_000_001),
    ]);
    assert_eq!(lanes(&chart), Some(vec![Lane::Scratch, Lane::Key3, Lane::Key5]));
}

#[test]
fn mines_long_note_ends_and_invisible_notes_do_not_count() {
    let chart = chart_with(vec![
        note(1, Lane::Key1, NoteKind::Mine, 100_000),
        note(2, Lane::Key2, NoteKind::Invisible, 200_000),
        note(3, Lane::Key3, NoteKind::LongEnd, 300_000),
        note(4, Lane::Key4, NoteKind::Tap, 1_000_000),
    ]);
    assert_eq!(lanes(&chart), Some(vec![Lane::Key4]));
}

#[test]
fn a_long_note_start_counts() {
    let chart = chart_with(vec![
        note(1, Lane::Key6, NoteKind::LongStart, 500_000),
        note(2, Lane::Key6, NoteKind::LongEnd, 900_000),
        note(3, Lane::Key1, NoteKind::Tap, 600_000),
    ]);
    assert_eq!(lanes(&chart), Some(vec![Lane::Key6]));
}

#[test]
fn double_play_uses_one_time_across_both_sides() {
    let chart = chart_with(vec![
        note(1, Lane::Key1, NoteKind::Tap, 2_000_000),
        note(2, Lane::Key9, NoteKind::Tap, 1_000_000),
        note(3, Lane::Scratch2, NoteKind::Tap, 1_000_000),
    ]);
    assert_eq!(lanes(&chart), Some(vec![Lane::Key9, Lane::Scratch2]));
}

#[test]
fn a_chart_without_hit_notes_has_no_preview() {
    let chart = chart_with(vec![
        note(1, Lane::Key1, NoteKind::Mine, 100_000),
        note(2, Lane::Key2, NoteKind::LongEnd, 200_000),
    ]);
    assert_eq!(ReadyFirstNotes::from_chart(&chart), None);
    assert_eq!(ReadyFirstNotes::from_chart(&chart_with(Vec::new())), None);
}

#[test]
fn the_preview_follows_the_arranged_lane() {
    // The preview is computed from the chart handed to the session, i.e. after RANDOM.
    let mut chart = chart_with(vec![note(1, Lane::Key1, NoteKind::Tap, 1_000_000)]);
    let moved = chart.lane_notes[Lane::Key1.index()].remove(0);
    chart.lane_notes[Lane::Key7.index()].push(NoteEvent { lane: Lane::Key7, ..moved });
    assert_eq!(lanes(&chart), Some(vec![Lane::Key7]));
}

#[test]
fn position_is_the_lane_top_or_just_under_the_cover_and_clamped() {
    let top = 1.0 - READY_NOTE_THICKNESS;
    assert!(approx_eq(first_note_y(0.0, 0.0), top));
    // SUDDEN+ 30%: the lower edge of the cover is at 70% of the lane.
    assert!(approx_eq(first_note_y(0.3, 0.0), 0.7 - READY_NOTE_THICKNESS));
    // LIFT 20% + SUDDEN+ 30%: the visible depth is 0.8; the cover leaves 0.5 of it.
    assert!(approx_eq(first_note_y(0.3, 0.2), 0.5 / 0.8 - READY_NOTE_THICKNESS));
    // Out-of-range amounts are clamped into 0..=1.
    assert!(approx_eq(first_note_y(5.0, 0.0), 0.0));
    assert!(approx_eq(first_note_y(-1.0, -1.0), top));
    assert!((0.0..=1.0).contains(&first_note_y(0.0, 9.0)));
}

#[test]
fn pulse_runs_between_65_and_100_percent_once_per_second() {
    assert!(approx_eq(pulse_alpha(0), 0.65));
    assert!(approx_eq(pulse_alpha(500), 1.0));
    assert!(approx_eq(pulse_alpha(1000), 0.65));
    assert!(approx_eq(pulse_alpha(-500), 1.0), "negative READY time wraps the same way");
    assert!((0.65..=1.0).contains(&pulse_alpha(123)));
}

fn ready_session(chart: PlayableChart) -> GameSession {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    session.lift = 0.0;
    session.offsets.visual_offset_us = 0;
    session.audio_clock =
        AudioClock::with_position(1_000_000, 0, -1_000_000, Arc::new(AtomicU64::new(0)), true);
    session
}

fn project(
    session: &GameSession,
    cache: &PlayRenderSnapshotCache,
    now: i64,
    lane_cover: f32,
) -> RenderSnapshot {
    let pool = PlayfieldProjection::pool(session, cache);
    let mut snapshot = build_render_snapshot(session, TimeUs(now), &[], None);
    snapshot.lane_cover = lane_cover;
    let mut clock = ProjectionClock::default();
    pool[0].project(&mut snapshot, TimeUs(now), &mut clock);
    snapshot
}

fn two_note_chart() -> PlayableChart {
    chart_with(vec![
        note(1, Lane::Key1, NoteKind::Tap, 1_000_000),
        note(2, Lane::Key1, NoteKind::Tap, 2_000_000),
        note(3, Lane::Key3, NoteKind::Tap, 1_000_000),
        note(4, Lane::Key2, NoteKind::Mine, 1_500_000),
    ])
}

#[test]
fn ready_shows_only_the_first_notes_and_playing_is_unchanged() {
    let session = ready_session(two_note_chart());
    let off = PlayRenderSnapshotCache::from_chart(&session.chart);
    let mut on = PlayRenderSnapshotCache::from_chart(&session.chart);
    on.enable_ready_first_notes(&session.chart);

    let ready_off = project(&session, &off, -500_000, 0.0);
    let ready_on = project(&session, &on, -500_000, 0.0);
    assert!(ready_off.visible_notes.iter().all(Vec::is_empty), "READY is empty without the option");
    let shown: Vec<Lane> =
        Lane::ALL.into_iter().filter(|l| !ready_on.visible_notes[l.index()].is_empty()).collect();
    assert_eq!(shown, vec![Lane::Key1, Lane::Key3]);
    for lane in shown {
        let notes = &ready_on.visible_notes[lane.index()];
        assert_eq!(notes.len(), 1);
        assert!(approx_eq(notes[0].y, first_note_y(0.0, 0.0)));
        assert_eq!(notes[0].time, TimeUs(1_000_000));
    }
    assert!(ready_on.visible_mines.iter().all(Vec::is_empty));
    assert!(ready_on.visible_long_notes.is_empty());

    // Once Playing (chart time >= 0) the option changes nothing.
    for now in [0, 400_000, 900_000] {
        let a = project(&session, &off, now, 0.0);
        let b = project(&session, &on, now, 0.0);
        assert_eq!(a.visible_notes, b.visible_notes, "now={now}");
        assert_eq!(a.visible_mines, b.visible_mines, "now={now}");
        assert_eq!(a.visible_long_notes, b.visible_long_notes, "now={now}");
    }
}

#[test]
fn ready_preview_follows_sudden_plus() {
    let session = ready_session(two_note_chart());
    let mut cache = PlayRenderSnapshotCache::from_chart(&session.chart);
    cache.enable_ready_first_notes(&session.chart);

    let covered = project(&session, &cache, -500_000, 0.3);
    let y = covered.visible_notes[Lane::Key1.index()][0].y;
    assert!(approx_eq(y, first_note_y(0.3, 0.0)));
}

#[test]
fn a_chart_without_hit_notes_keeps_the_normal_ready_snapshot() {
    let session = ready_session(chart_with(vec![note(1, Lane::Key1, NoteKind::Mine, 1_000_000)]));
    let mut cache = PlayRenderSnapshotCache::from_chart(&session.chart);
    cache.enable_ready_first_notes(&session.chart);
    let off = PlayRenderSnapshotCache::from_chart(&session.chart);

    let a = project(&session, &cache, -500_000, 0.0);
    let b = project(&session, &off, -500_000, 0.0);
    assert_eq!(a.visible_notes, b.visible_notes);
    assert_eq!(a.visible_mines, b.visible_mines);
}

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// A READY-phase session at chart time -1s. `frame` moves the audio clock.
fn ready_session(mode: HispeedMode) -> (GameSession, Arc<AtomicU64>) {
    let mut session = session_with_autoplay(chart_with_keysound());
    let frame = Arc::new(AtomicU64::new(0));
    session.audio_clock = AudioClock::with_position(48_000, 0, -1_000_000, frame.clone(), true);
    session.hispeed_mode = mode;
    session.base_hispeed_mode = HispeedMode::Classic;
    (session, frame)
}

fn start_playing(frame: &AtomicU64) {
    frame.store(48_000, Ordering::Relaxed);
}

#[test]
fn a_new_session_has_not_used_floating() {
    let session = session_with_autoplay(chart_with_keysound());
    assert!(!session.floating_hispeed_used);
}

#[test]
fn floating_only_during_ready_is_not_counted() {
    let (mut session, frame) = ready_session(HispeedMode::Floating);
    let mut audio = TestAudio::default();

    let ready = advance_session_frame(&mut session, &mut audio);
    assert_eq!(ready.state, PlayState::Ready);
    assert!(!session.floating_hispeed_used);

    // Back to the base mode before the first Playing frame.
    session.hispeed_mode = HispeedMode::Classic;
    start_playing(&frame);
    let playing = advance_session_frame(&mut session, &mut audio);
    assert_eq!(playing.state, PlayState::Playing);
    assert!(!session.floating_hispeed_used);
}

#[test]
fn floating_while_playing_is_counted() {
    let (mut session, frame) = ready_session(HispeedMode::Floating);
    let mut audio = TestAudio::default();
    advance_session_frame(&mut session, &mut audio);
    start_playing(&frame);

    let playing = advance_session_frame(&mut session, &mut audio);

    assert_eq!(playing.state, PlayState::Playing);
    assert!(session.floating_hispeed_used);
}

#[test]
fn switching_to_floating_mid_play_stays_counted_after_switching_back() {
    let (mut session, frame) = ready_session(HispeedMode::Classic);
    let mut audio = TestAudio::default();
    start_playing(&frame);
    advance_session_frame(&mut session, &mut audio);
    assert!(!session.floating_hispeed_used);

    session.hispeed_mode = HispeedMode::Floating;
    advance_session_frame(&mut session, &mut audio);
    assert!(session.floating_hispeed_used);

    session.hispeed_mode = session.base_hispeed_mode;
    advance_session_frame(&mut session, &mut audio);
    assert!(session.floating_hispeed_used);
}

#[test]
fn base_mode_for_the_whole_play_is_not_counted() {
    let (mut session, frame) = ready_session(HispeedMode::Classic);
    let mut audio = TestAudio::default();
    advance_session_frame(&mut session, &mut audio);
    start_playing(&frame);
    advance_session_frame(&mut session, &mut audio);
    advance_session_frame(&mut session, &mut audio);

    assert!(!session.floating_hispeed_used);
}

#[test]
fn a_session_whose_floating_policy_is_disabled_keeps_the_base_mode() {
    let (mut session, frame) = ready_session(HispeedMode::Normal);
    session.floating_policy = FloatingPolicy::Disabled;
    let mut audio = TestAudio::default();
    start_playing(&frame);
    advance_session_frame(&mut session, &mut audio);

    assert!(!session.floating_hispeed_used);
}

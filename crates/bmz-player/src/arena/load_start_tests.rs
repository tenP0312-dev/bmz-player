//! B2-2: option phase, `prepare`, `play_ready` and `start` (spec 3.4 steps 8-11).

use super::match_setup_tests::{MD5, at, chart, connected, in_nomination, json, recv, sent};
use super::*;
use crate::arena::protocol::{OptionSelect, Prepare, Start};

/// Match `m1` with the chart decided and checked, as `option_select` finds it.
fn at_chart(session: &mut ArenaSession) {
    in_nomination(session, "m1");
    recv(session, chart(Some("m1"), MD5));
}

/// Messages a tick sends, without the keep-alive pings.
fn ticked(session: &mut ArenaSession, secs: f64) -> Vec<ClientMessage> {
    sent(&session.handle(at(secs), Event::Tick))
        .into_iter()
        .filter(|message| json(message)["type"] != "ping")
        .collect()
}

fn option_select(ready: bool, deadline: f64) -> ServerMessage {
    ServerMessage::OptionSelect(OptionSelect {
        match_id: "m1".into(),
        deadline: Some(deadline),
        ready,
        ready_count: Some(0),
        player_count: Some(2),
        play_mode: Some(7),
        ln_mode: "LN".into(),
    })
}

pub(super) fn prepare(play_option: u32) -> ServerMessage {
    ServerMessage::Prepare(Prepare {
        match_id: "m1".into(),
        md5: MD5.into(),
        totalnotes: Some(900),
        load_deadline: Some(1_130.0),
        ln_mode: "LN".into(),
        random_seed: Some(777),
        play_option,
        play_mode: Some(7),
        ruleset_profile: "oraja".into(),
    })
}

pub(super) fn in_options(session: &mut ArenaSession, deadline: f64) {
    at_chart(session);
    recv(session, option_select(false, deadline));
}

pub(super) fn in_loading(session: &mut ArenaSession) {
    in_options(session, 1_010.0);
    recv(session, prepare(2));
}

#[test]
fn option_select_opens_the_option_phase() {
    let mut session = connected();
    in_options(&mut session, 1_010.0);
    let view = session.panel().current_match.unwrap();
    assert_eq!(view.stage, MatchStage::Options);
    assert_eq!(view.deadline, Some(1_010.0));
    let option = view.option.unwrap();
    assert!(!option.ready);
    assert_eq!(option.play_mode, Some(7));
    assert_eq!(option.player_count, Some(2));
}

#[test]
fn pressing_ready_sends_the_current_choice_once() {
    let mut session = connected();
    in_options(&mut session, 1_010.0);
    session.handle(at(2.0), Event::SetOptionChoice(102));
    let messages = sent(&session.handle(at(3.0), Event::OptionReadyNow));
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"option_ready","match_id":"m1","play_option":102,
                           "play_mode":7,"ln_mode":"LN"})
    );
    assert!(sent(&session.handle(at(3.1), Event::OptionReadyNow)).is_empty());
    // The deadline does not send it a second time either.
    assert!(ticked(&mut session, 9.9).is_empty());
}

#[test]
fn the_choice_is_sent_automatically_a_quarter_second_before_the_deadline() {
    let mut session = connected();
    in_options(&mut session, 1_010.0);
    session.handle(at(2.0), Event::SetOptionChoice(3));
    // Server time = local time here (hello_ok carried server_time 1000 at local 1000).
    assert!(ticked(&mut session, 9.0).is_empty());
    assert!(ticked(&mut session, 9.7).is_empty());
    let messages = ticked(&mut session, 9.76);
    let option_ready: Vec<_> =
        messages.iter().filter(|m| json(m)["type"] == "option_ready").collect();
    assert_eq!(option_ready.len(), 1);
    assert_eq!(json(option_ready[0])["play_option"], 3);
    assert!(ticked(&mut session, 9.9).iter().all(|m| json(m)["type"] != "option_ready"));
}

#[test]
fn without_a_choice_the_automatic_send_is_normal() {
    let mut session = connected();
    in_options(&mut session, 1_010.0);
    let messages = ticked(&mut session, 9.8);
    let option_ready = messages.iter().find(|m| json(m)["type"] == "option_ready").unwrap();
    assert_eq!(json(option_ready)["play_option"], 0);
}

#[test]
fn nothing_is_auto_sent_when_the_server_already_has_the_option() {
    let mut session = connected();
    at_chart(&mut session);
    recv(&mut session, option_select(true, 1_010.0));
    assert!(ticked(&mut session, 9.9).is_empty());
    assert!(sent(&session.handle(at(9.9), Event::OptionReadyNow)).is_empty());
}

#[test]
fn option_ready_is_only_for_the_option_phase() {
    let mut session = connected();
    in_nomination(&mut session, "m1");
    assert!(sent(&session.handle(at(2.0), Event::OptionReadyNow)).is_empty());
    recv(&mut session, chart(Some("m1"), MD5));
    assert!(sent(&session.handle(at(2.0), Event::OptionReadyNow)).is_empty());
}

#[test]
fn prepare_hands_the_app_the_confirmed_option_once() {
    let mut session = connected();
    in_options(&mut session, 1_010.0);
    recv(&mut session, prepare(102));
    let panel = session.panel();
    let view = panel.current_match.unwrap();
    assert_eq!(view.stage, MatchStage::Loading);
    assert_eq!(view.deadline, Some(1_130.0));
    let pending = panel.pending_prepare.unwrap();
    assert_eq!(pending.play_option, 102);
    assert_eq!(pending.random_seed, Some(777));
    session.handle(at(2.0), Event::PrepareTaken { match_id: "other".into() });
    assert!(session.panel().pending_prepare.is_some());
    session.handle(at(2.0), Event::PrepareTaken { match_id: "m1".into() });
    assert!(session.panel().pending_prepare.is_none());
    // The match keeps the confirmed option for `play_ready`.
    assert!(session.panel().current_match.unwrap().prepare.is_some());
}

#[test]
fn a_late_option_select_does_not_reopen_the_option_phase() {
    let mut session = connected();
    in_loading(&mut session);
    recv(&mut session, option_select(true, 1_010.0));
    assert_eq!(session.panel().current_match.unwrap().stage, MatchStage::Loading);
}

#[test]
fn play_ready_is_sent_once_with_the_loaded_note_count() {
    let mut session = connected();
    in_loading(&mut session);
    let messages = sent(&session.handle(
        at(3.0),
        Event::PlayLoaded { match_id: "m1".into(), md5: MD5.to_uppercase(), totalnotes: 1234 },
    ));
    assert_eq!(
        json(&messages[0]),
        serde_json::json!({"type":"play_ready","match_id":"m1","chart_hash":MD5,
                           "ln_mode":"LN","totalnotes":1234})
    );
    assert!(
        sent(&session.handle(
            at(3.1),
            Event::PlayLoaded { match_id: "m1".into(), md5: MD5.into(), totalnotes: 1234 }
        ))
        .is_empty()
    );
}

#[test]
fn play_ready_is_not_sent_for_the_wrong_match_chart_stage_or_zero_notes() {
    let loaded = |match_id: &str, md5: &str, totalnotes: u32| Event::PlayLoaded {
        match_id: match_id.into(),
        md5: md5.into(),
        totalnotes,
    };
    let mut session = connected();
    // Before `prepare`.
    in_options(&mut session, 1_010.0);
    assert!(sent(&session.handle(at(2.0), loaded("m1", MD5, 10))).is_empty());
    recv(&mut session, prepare(0));
    assert!(sent(&session.handle(at(3.0), loaded("other", MD5, 10))).is_empty());
    assert!(
        sent(&session.handle(at(3.0), loaded("m1", "ffffffffffffffffffffffffffffffff", 10)))
            .is_empty()
    );
    // v8 needs a positive count.
    assert!(sent(&session.handle(at(3.0), loaded("m1", MD5, 0))).is_empty());
    assert_eq!(sent(&session.handle(at(3.0), loaded("m1", MD5, 10))).len(), 1);
}

#[test]
fn start_gives_the_app_the_server_time_to_begin() {
    let mut session = connected();
    in_loading(&mut session);
    assert!(session.panel().start_gate.is_none());
    recv(
        &mut session,
        ServerMessage::Start(Start {
            match_id: "m1".into(),
            start_at: Some(1_020.5),
            ln_mode: "LN".into(),
        }),
    );
    let panel = session.panel();
    assert_eq!(panel.current_match.unwrap().stage, MatchStage::Countdown);
    let gate = panel.start_gate.unwrap();
    assert_eq!((gate.match_id.as_str(), gate.start_at), ("m1", 1_020.5));
    // A `start` for another match changes nothing.
    recv(
        &mut session,
        ServerMessage::Start(Start {
            match_id: "x".into(),
            start_at: Some(5.0),
            ..Start::default()
        }),
    );
    assert_eq!(session.panel().start_gate.unwrap().start_at, 1_020.5);
}

#[test]
fn cancel_and_release_drop_the_load_state() {
    for end in [
        ServerMessage::MatchCancelled { reason: "load_timeout".into() },
        ServerMessage::MatchReleased,
    ] {
        let mut session = connected();
        in_loading(&mut session);
        recv(&mut session, end);
        let panel = session.panel();
        assert!(panel.current_match.is_none());
        assert!(panel.pending_prepare.is_none());
        assert!(panel.start_gate.is_none());
    }
}

#[test]
fn a_new_chart_after_a_reroll_restarts_the_option_phase() {
    let mut session = connected();
    in_loading(&mut session);
    recv(&mut session, chart(Some("m1"), "ffffffffffffffffffffffffffffffff"));
    let panel = session.panel();
    let view = panel.current_match.unwrap();
    assert_eq!(view.stage, MatchStage::ChartChosen);
    assert!(view.option.is_none() && view.prepare.is_none());
    assert!(panel.pending_prepare.is_none());
    // The option for the new round can be sent again.
    recv(&mut session, option_select(false, 1_040.0));
    assert_eq!(sent(&session.handle(at(5.0), Event::OptionReadyNow)).len(), 1);
}

#[test]
fn a_failed_load_forfeits_only_the_current_match() {
    let mut session = connected();
    in_loading(&mut session);
    assert!(
        sent(&session.handle(at(3.0), Event::LoadFailed { match_id: "other".into() })).is_empty()
    );
    let messages = sent(&session.handle(at(3.0), Event::LoadFailed { match_id: "m1".into() }));
    assert_eq!(json(&messages[0]), serde_json::json!({"type":"forfeit","match_id":"m1"}));
}

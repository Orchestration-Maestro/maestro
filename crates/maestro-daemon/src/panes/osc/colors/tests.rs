//! Ordered default-colour observation cases.

use super::*;

fn tracked_default_color_events(events: Vec<DefaultColorTrackedEvent>) -> Vec<DefaultColorEvent> {
    events.into_iter().map(|event| event.event).collect()
}

#[test]
fn pane_default_color_event_tracker_detects_queries_sets_and_resets() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(
        b"\x1b]10;?\x07\x1b]11;?\x1b\\\x1b]12;?\x07\x1b]4;0;?\x07\x1b]10;rgb:11/22/33\x07\x1b]111\x07",
    );

    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![
            DefaultColorEvent::Query(DefaultColorQuery::Foreground),
            DefaultColorEvent::Query(DefaultColorQuery::Background),
            DefaultColorEvent::Query(DefaultColorQuery::Cursor),
            DefaultColorEvent::PaletteQuery(0),
            DefaultColorEvent::Set(DefaultColorQuery::Foreground),
            DefaultColorEvent::Reset(DefaultColorQuery::Background),
        ]
    );
}

#[test]
fn pane_default_color_event_tracker_tracks_each_multi_value_set() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(
        b"\x1b]10;rgb:11/22/33;rgb:44/55/66\x1b\\\x1b]10;?;rgb:77/88/99\x1b\\\x1b]10;;rgb:aa/bb/cc\x1b\\",
    );

    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![
            DefaultColorEvent::Set(DefaultColorQuery::Foreground),
            DefaultColorEvent::Set(DefaultColorQuery::Background),
            DefaultColorEvent::Set(DefaultColorQuery::Background),
            DefaultColorEvent::Set(DefaultColorQuery::Foreground),
        ]
    );
}

#[test]
fn pane_default_color_event_tracker_handles_split_default_color_queries() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(b"\x1b]11");
    assert!(tracker.drain_pending().is_empty());
    tracker.observe(b";?\x1b");
    assert!(tracker.drain_pending().is_empty());
    tracker.observe(b"\\");

    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![DefaultColorEvent::Query(DefaultColorQuery::Background)]
    );
}

#[test]
fn pane_default_color_event_tracker_handles_split_palette_color_queries() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(b"\x1b]4;25");
    assert!(tracker.drain_pending().is_empty());
    tracker.observe(b"5;?\x1b");
    assert!(tracker.drain_pending().is_empty());
    tracker.observe(b"\\");

    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![DefaultColorEvent::PaletteQuery(255)]
    );
}

#[test]
fn pane_default_color_event_tracker_rejects_malformed_palette_color_queries() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(b"\x1b]4;;?\x07");
    tracker.observe(b"\x1b]4;-1;?\x07");
    tracker.observe(b"\x1b]4;256;?\x07");
    tracker.observe(b"\x1b]4;0;?;1;?\x07");
    tracker.observe(b"\x1b]4;0;rgb:1111/2222/3333\x07");
    tracker.observe(b"\x1b]4;0;?\x07");

    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![DefaultColorEvent::PaletteQuery(0)]
    );
}

#[test]
fn pane_default_color_event_tracker_ignores_other_osc_and_dcs_payloads() {
    let mut tracker = DefaultColorEventTracker::default();

    tracker.observe(b"\x1b]0;title\x07");
    tracker.observe(b"\x1b]52;c;?\x07");
    tracker.observe(b"\x1bPtmux;\x1b\x1b]11;?\x07\x1b\\");
    tracker.observe(b"\x1bPtmux;payload\x07\x1b]11;?\x07\x1b\\");

    assert!(tracker.drain_pending().is_empty());
}

#[test]
fn pane_default_color_event_tracker_ignores_oversized_osc_until_terminator() {
    let mut tracker = DefaultColorEventTracker::default();
    let mut oversized = Vec::from(b"\x1b]11;".as_slice());
    oversized.extend(std::iter::repeat_n(b'a', 1025));
    oversized.extend_from_slice(b"\x1b]11;?\x07");

    tracker.observe(&oversized);
    assert!(tracker.drain_pending().is_empty());

    tracker.observe(b"\x1b]11;?\x07");
    assert_eq!(
        tracked_default_color_events(tracker.drain_pending()),
        vec![DefaultColorEvent::Query(DefaultColorQuery::Background)]
    );
}

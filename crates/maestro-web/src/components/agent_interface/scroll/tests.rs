//! Boundary witnesses for the transcript's native scroll observations.
use super::{FollowState, ReadingScroll};

/// Construct an observation with an exact fractional distance from the bottom.
fn at(top: f64, distance: f64) -> ReadingScroll {
    ReadingScroll {
        top,
        height: top + 100. + distance,
        client_height: 100.,
    }
}
/// Disable only for nonzero upward movement beyond the strict fifty-pixel boundary.
#[test]
fn follow_disables_only_after_nonzero_upward_scroll_beyond_fifty() {
    for (top, distance, expected) in [
        (80., 50., true),
        (80., 50.125, false),
        (0., 100., true),
        (100., 100., true),
        (101., 100., true),
        (99.875, 50.125, false),
    ] {
        let mut state = FollowState {
            last_top: 100.,
            last_height: 100.,
            ..Default::default()
        };
        state.observe(Some(at(top, distance)));
        assert_eq!(state.enabled, expected, "top={top} distance={distance}");
    }
    let mut state = FollowState {
        enabled: false,
        last_top: 100.,
        last_height: 100.,
    };
    state.observe(Some(at(101., 100.)));
    assert!(!state.enabled);
}
/// Resume only below ten, including negative/no-overflow distances and near-bottom upward motion.
#[test]
fn follow_resumes_only_below_ten() {
    for (distance, expected) in [(9.875, true), (10., false), (10.125, false), (-20., true)] {
        let mut state = FollowState {
            enabled: false,
            last_top: 100.,
            last_height: 100.,
        };
        state.observe(Some(at(99.875, distance)));
        assert_eq!(state.enabled, expected, "distance={distance}");
    }
}
/// A shrinking editor changes only remembered height, so the next scroll keeps its prior origin.
#[test]
fn follow_ignores_editor_height_shrink_without_advancing_last_top() {
    for enabled in [false, true] {
        let mut state = FollowState {
            enabled,
            last_top: 100.5,
            last_height: 120.,
        };
        state.observe(Some(at(80., 100.)));
        assert_eq!(state.last_top.to_bits(), 100.5_f64.to_bits());
        assert_eq!(state.last_height.to_bits(), 100_f64.to_bits());
        assert_eq!(state.enabled, enabled);
        state.observe(Some(at(90., 100.)));
        assert!(
            !state.enabled,
            "next movement is upward from the retained top"
        );
        assert_eq!(state.last_top.to_bits(), 90_f64.to_bits());
        let mut growth = at(91., 9.);
        growth.client_height = 101.;
        state.observe(Some(growth));
        assert!(state.enabled, "growth is not the editor shrink branch");
        assert_eq!(state.last_top.to_bits(), 91_f64.to_bits());
    }
}
/// Missing viewports are inert and fractional positions remain exact for later transitions.
#[test]
fn follow_tracks_positions_and_handles_missing_viewport() {
    let mut state = FollowState::default();
    state.observe(Some(at(100.125, 50.)));
    state.observe(None);
    assert!(state.enabled);
    assert_eq!(state.last_top.to_bits(), 100.125_f64.to_bits());
    assert_eq!(state.last_height.to_bits(), 100_f64.to_bits());
    state.observe(Some(at(100., 50.125)));
    assert!(!state.enabled);
    assert_eq!(state.last_top.to_bits(), 100_f64.to_bits());
}

/// Recorded native viewport operands retain exact follow state and fractional positions.
#[test]
fn recorded_scroll_observations_match_follow_state() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/transcript.json"
    )))
    .unwrap();
    for row in corpus["scroll"].as_array().unwrap() {
        let input = &row["input"];
        let mut state = FollowState::default();
        let current = input["top"].as_f64().map(|top| {
            state.enabled = input["enabled"].as_bool().unwrap();
            state.last_top = input["lastTop"].as_f64().unwrap();
            state.last_height = input["lastHeight"].as_f64().unwrap();
            ReadingScroll {
                top,
                height: input["height"].as_f64().unwrap(),
                client_height: input["client"].as_f64().unwrap(),
            }
        });
        state.observe(current);
        let expected = &row["expected"];
        assert_eq!(state.enabled, expected["enabled"], "{}", row["id"]);
        assert_eq!(
            state.last_top.to_bits(),
            expected["lastTop"].as_f64().unwrap().to_bits(),
            "{}",
            row["id"]
        );
        assert_eq!(
            state.last_height.to_bits(),
            expected["lastHeight"].as_f64().unwrap().to_bits(),
            "{}",
            row["id"]
        );
    }
}

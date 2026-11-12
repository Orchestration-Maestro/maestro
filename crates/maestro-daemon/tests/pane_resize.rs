//! Pane resize through the daemon's handler: which divider moves, by how much, and what is published.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{
    answer_command, event_names, events_after, failure, request,
};
use maestro_daemon::state::test_support::pane_resize::{pair, ratio, resize_of};
use maestro_daemon::state::test_support::run;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{PaneDirection, PaneResizeParams, PaneResizeResult};
use serde_json::json;
/// Resizes `pane` rightward by a decoded amount, which the wire form cannot carry.
fn resize_by(app: &mut App, amount: Option<f32>) -> PaneResizeResult {
    let command = RpcCommand::PaneResize(PaneResizeParams {
        pane_id: Some("w1:p1".into()),
        direction: PaneDirection::Right,
        amount,
    });
    resize_of(run(answer_command(app, command)).unwrap()).unwrap()
}
#[test]
fn a_resize_amount_defaults_ignores_sign_and_is_capped() {
    let (mut app, _, _) = pair().unwrap();
    for (amount, expected) in [
        (None, 0.55),
        (Some(f32::NAN), 0.6),
        (Some(f32::INFINITY), 0.65),
        (Some(-0.1), 0.75),
        (Some(9.0), 0.9),
    ] {
        assert!(resize_by(&mut app, amount).changed, "{amount:?}");
        assert!(
            (ratio(&mut app).unwrap() - expected).abs() < 1e-5,
            "{amount:?}"
        );
    }
    assert!(!resize_by(&mut app, Some(0.2)).changed);
    assert_eq!(event_names(&app, 0).unwrap(), ["layout_updated"; 5]);
}
#[test]
fn resizing_a_missing_pane_is_refused_without_a_change() {
    let (mut app, _, _) = pair().unwrap();
    let wire = json!({"type": "pane.resize", "pane_id": "w1:p9", "direction": "right"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "pane not found")
    );
    assert!((ratio(&mut app).unwrap() - 0.5).abs() < f32::EPSILON);
    assert!(events_after(&app, 0).is_empty());
}

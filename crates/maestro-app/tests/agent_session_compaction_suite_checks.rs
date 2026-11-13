//! Public prompts respect the automatic-compaction threshold and enable switch.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::compaction::seeded;
use crate::support::session_harness::{Harness, of_kinds, prompt_to_end, summary_provider};
use maestro_app::test_support::{HarnessOptions, Scripted, on_task_set};
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::json;

/// Checks completed compaction events against the saved branch.
fn assert_threshold(h: &Harness, expected: usize) {
    let events = h.events.borrow();
    let starts = of_kinds(&events, &["compaction_start"]);
    let ends = of_kinds(&events, &["compaction_end"]);
    assert_eq!(starts.len(), expected);
    assert_eq!(ends.len(), expected);
    for start in starts {
        assert_eq!(
            start,
            json!({"type": "compaction_start", "reason": "threshold"})
        );
    }
    for end in ends {
        assert_eq!(end["reason"], "threshold");
        assert_eq!(end["aborted"], false);
        assert_eq!(end["willRetry"], false);
        assert_eq!(end["result"]["summary"], "public auto summary");
    }
    assert_eq!(
        h.session
            .session_manager()
            .borrow()
            .get_entries()
            .iter()
            .filter(|entry| entry["type"] == "compaction")
            .count(),
        expected
    );
    assert_eq!(h.requests.0.borrow().len(), 3);
    assert!(!h.session.is_compacting());
}

/// Sends a response reporting `tokens` through a complete public prompt.
async fn threshold_case(tokens: u32, enabled: bool, expected: usize) {
    let h = seeded(HarnessOptions {
        factories: vec![summary_provider("public auto summary")],
        responses: Some(vec![
            Scripted::text("one"),
            Scripted::text("two"),
            Scripted {
                usage: Some((f64::from(tokens), 0.0)),
                ..Scripted::text("boundary response")
            },
        ]),
        ..HarnessOptions::default()
    })
    .await;
    h.session.settings_manager().borrow_mut().apply_overrides(
        serde_json::from_value(json!({
            "compaction": {"enabled": enabled, "reserveTokens": 1000}
        }))
        .unwrap(),
    );

    prompt_to_end(
        &h,
        "trigger threshold",
        maestro_app::PromptOptions::default(),
    )
    .await;

    assert_threshold(&h, expected);
}

#[test]
fn public_compaction_obeys_the_threshold_boundary_and_switch() {
    on_task_set(|_| async {
        for enabled in [true, false] {
            for (tokens, count) in [
                (0, 0),
                (198_999, 0),
                (199_000, 0),
                (199_001, 1),
                (200_000, 1),
            ] {
                threshold_case(tokens, enabled, usize::from(enabled) * count).await;
            }
        }
    })
    .unwrap();
}

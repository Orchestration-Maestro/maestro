//! The shipped turn status hooks exercised through the built component's public UI.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::documents::{Answer, Ended};
use super::host::Harness;
use super::theme_host::{Palette, Reply};
use super::ui_host::receipts;

/// Supplies literal style answers, independent of the example's concatenation.
fn palette() -> Palette {
    let replies = [
        ("dim", "Ready", "ready"),
        ("accent", "●", "dot"),
        ("success", "✓", "check"),
        ("dim", " Turn 0 complete", "zero done"),
        ("dim", " Turn 1...", "one busy"),
        ("dim", " Turn 1 complete", "one done"),
        ("dim", " Turn 2...", "two busy"),
        ("dim", " Turn 2 complete", "two done"),
        ("dim", " Turn 3...", "three busy"),
        ("dim", " Turn 3 complete", "three done"),
    ]
    .into_iter()
    .map(|(color, text, styled)| Reply {
        method: "fg".into(),
        operands: json!([color, text]),
        answer: json!({"ok": styled}),
    })
    .collect();
    Palette {
        name: None,
        source_path: None,
        source_info: None,
        mode: "truecolor".into(),
        replies,
    }
}

/// Loads the single shipped factory and supplies the UI's current theme.
fn start() -> Result<Harness, String> {
    let mut harness = Harness::example("status_line")?;
    assert!(receipts(&harness.observed().transcript).is_empty());
    harness.observed().themes.publish("dark", palette());
    Ok(harness)
}

/// Invokes the named registration, preserving the event and inspecting its ending.
fn invoke(harness: &mut Harness, event: &str) -> Result<Ended, String> {
    let key = harness
        .callback(&format!("event {event}"))
        .map_err(|e| e.to_string())?;
    let document = super::documents::document(event);
    let capabilities = super::host_family::guest::Capabilities {
        ctx: harness.ordinary("/controlled").map_err(|e| e.to_string())?,
        signal: None,
    };
    let exports = harness.exports.clone();
    let run = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                document.to_string(),
                capabilities,
            )
            .await
    }))
    .map_err(|e| e.to_string())?;
    let outcome = run.map_err(|e| e.to_string())??;
    let answer = Answer::of(super::host_family::delivery(outcome))?;
    assert_eq!(answer.event, Some(super::documents::document(event)));
    Ok(answer.ended)
}

/// Asserts the hook requests exactly one status under the authored key.
fn status(harness: &mut Harness, event: &str, expected: &str) -> Result<(), String> {
    let from = harness.observed().transcript.len();
    assert_eq!(invoke(harness, event)?, Ended::Returned(None));
    let calls = receipts(&harness.observed().transcript[from..]);
    let statuses: Vec<_> = calls
        .iter()
        .filter(|call| call.method == "setStatus")
        .collect();
    assert_eq!(statuses.len(), 1, "{event}");
    assert_eq!(statuses[0].operands, json!(["status-demo", expected]));
    Ok(())
}

/// Releases all three callbacks and proves no lent resources remain.
fn finish(mut harness: Harness) -> Result<(), String> {
    for event in ["session_start", "turn_start", "turn_end"] {
        let key = harness
            .callback(&format!("event {event}"))
            .map_err(|e| e.to_string())?;
        super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

/// Each session start reads the current theme, rather than retaining styled text.
#[test]
fn status_line_component_sets_ready_on_every_session_start() -> Result<(), String> {
    let mut harness = start()?;
    status(&mut harness, "session_start", "ready")?;
    let mut changed = palette();
    changed.replies[0].answer = json!({"ok": "new ready"});
    harness.observed().themes.publish("dark", changed);
    status(&mut harness, "session_start", "new ready")?;
    finish(harness)
}

/// The factory counts starts, not event indices, and a new session does not reset it.
#[test]
fn status_line_component_counts_turns_across_sessions() -> Result<(), String> {
    let mut harness = start()?;
    for (event, expected) in [
        ("turn_end", "checkzero done"),
        ("turn_start", "dotone busy"),
        ("turn_end", "checkone done"),
        ("turn_end", "checkone done"),
        ("session_start", "ready"),
        ("turn_start", "dottwo busy"),
        ("turn_end", "checktwo done"),
    ] {
        status(&mut harness, event, expected)?;
    }
    finish(harness)?;
    let mut fresh = start()?;
    status(&mut fresh, "turn_start", "dotone busy")?;
    finish(fresh)
}

/// Rejected UI and theme requests return the host message without undoing a started turn.
#[test]
fn status_line_component_preserves_count_after_host_rejection() -> Result<(), String> {
    let mut harness = start()?;
    for method in ["theme", "setStatus"] {
        harness.observed().ui.reject(method, "host refused");
        assert_eq!(
            invoke(&mut harness, "turn_start")?,
            Ended::Failed("host refused".into())
        );
    }
    status(&mut harness, "turn_end", "checktwo done")?;
    status(&mut harness, "turn_start", "dotthree busy")?;
    status(&mut harness, "turn_end", "checkthree done")?;
    finish(harness)
}

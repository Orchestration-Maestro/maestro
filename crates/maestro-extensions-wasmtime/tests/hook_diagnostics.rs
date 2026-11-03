//! A hook's supplied diagnostic reaches the native error listener unchanged.
//!
//! An authored component supplies the message and stack of its own failure; the runner projects
//! those fields into the attributed failure it reports, and a failure known only by its message
//! keeps no stack.
#![cfg(test)]
use crate::component_events::{Harness, cases};
use crate::components::local;
use serde_json::{Value, json};

/// The failure one instance reports, as the listener saw it.
fn failure(path: &str, event: &str, error: &str, stack: Option<&str>) -> Value {
    json!({ "extensionPath": path, "event": event, "error": error, "stack": stack })
}

#[test]
fn authored_hook_diagnostic_reaches_native_error_listener() {
    local(async {
        let harness = Harness::load().await;
        supplied_stack_and_healthy_handler(&harness).await;
        absent_and_empty_fields(&harness).await;
        multiline_unicode(&harness).await;
        edit_before_failed_decision(&harness).await;
    });
}

/// Both supplied fields cross, and the handler after the failing one still runs.
async fn supplied_stack_and_healthy_handler(harness: &Harness) {
    let run = harness
        .prepare(
            "agent_start",
            &[
                vec![json!([{
                    "authored": { "message": "owned diagnostic", "stack": "owned stack" }
                }])],
                vec![json!([{ "trace": "healthy" }])],
            ],
        )
        .await;
    cases::notify(&run.runner, "agent_start").await;
    assert_eq!(
        run.diagnostics(),
        json!([failure(
            "<extension:0>",
            "agent_start",
            "owned diagnostic",
            Some("owned stack")
        )]),
        "the listener receives both exact fields and no failure from the healthy handler"
    );
    assert_eq!(harness.bus.take(), [json!("healthy")]);
}

/// An absent stack stays absent and supplied empty fields stay empty.
async fn absent_and_empty_fields(harness: &Harness) {
    let run = harness
        .prepare(
            "agent_start",
            &[vec![json!([{ "authored": { "message": "message only" } }])]],
        )
        .await;
    cases::notify(&run.runner, "agent_start").await;
    assert_eq!(
        run.diagnostics(),
        json!([failure(
            "<extension:0>",
            "agent_start",
            "message only",
            None
        )]),
        "an absent stack stays absent"
    );
    let run = harness
        .prepare(
            "agent_start",
            &[vec![
                json!([{ "authored": { "message": "", "stack": "" } }]),
            ]],
        )
        .await;
    cases::notify(&run.runner, "agent_start").await;
    assert_eq!(
        run.diagnostics(),
        json!([failure("<extension:0>", "agent_start", "", Some(""))]),
        "empty fields stay empty"
    );
}

/// Multiline Unicode fields cross without normalisation.
async fn multiline_unicode(harness: &Harness) {
    let message = "first line\nsécond ✓";
    let stack = "at one\nat two ✓";
    let run = harness
        .prepare(
            "agent_start",
            &[vec![json!([{
                "authored": { "message": message, "stack": stack }
            }])]],
        )
        .await;
    cases::notify(&run.runner, "agent_start").await;
    assert_eq!(
        run.diagnostics(),
        json!([failure(
            "<extension:0>",
            "agent_start",
            message,
            Some(stack)
        )]),
        "multiline Unicode fields cross unchanged"
    );
}

/// An edit before a separately failed decision is retained.
async fn edit_before_failed_decision(harness: &Harness) {
    let run = harness
        .prepare(
            "before_provider_request",
            &[
                vec![json!([
                    { "edit": { "payload": { "nested": { "flag": "edited" } } } },
                    { "authored": { "message": "after edit", "stack": "edit stack" } },
                ])],
                vec![json!([{ "observe": "payload" }])],
            ],
        )
        .await;
    let payload = run
        .runner
        .emit_before_provider_request(json!({ "nested": { "flag": "original" } }))
        .await
        .unwrap();
    assert_eq!(
        payload,
        json!({ "nested": { "flag": "edited" } }),
        "the edit survives the later failed decision"
    );
    assert_eq!(
        harness.bus.take(),
        [json!({ "nested": { "flag": "edited" } })]
    );
    assert_eq!(
        run.diagnostics(),
        json!([failure(
            "<extension:0>",
            "before_provider_request",
            "after edit",
            Some("edit stack")
        )]),
        "the failed decision still reaches the listener"
    );
}

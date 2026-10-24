//! A transient lost connection is retried by the application session, keeping its own
//! diagnostic apart from any transport setup retry.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, Scripted, delivered_events, get_assistant_texts, harness, on_task_set,
    start_prompt,
};
use serde_json::{Value, json};

#[test]
fn lost_connection_retries_and_keeps_diagnostic() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            responses: Some(vec![
                Scripted::failure("Network connection lost."),
                Scripted::text("recovered after reconnect"),
            ]),
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            ..HarnessOptions::default()
        })
        .await;
        let events = delivered_events(&h.session);
        start_prompt(&h, "test").await.unwrap().unwrap();
        let events = events.borrow();
        let starts: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_start")
            .collect();
        let ends: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_end")
            .collect();
        assert_eq!(h.requests.0.borrow().len(), 2);
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0]["errorMessage"], json!("Network connection lost."));
        assert_eq!(ends.len(), 1);
        assert_eq!(ends[0]["success"], json!(true));
        assert_eq!(get_assistant_texts(&h), ["recovered after reconnect"]);
    });
}

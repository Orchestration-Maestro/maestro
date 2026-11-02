//! Context usage estimates over image attachments and all-zero assistant usage.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    HarnessOptions, assistant, harness, on_task_set, usage, user,
};
use maestro_extensions::NativeAgentMessage;
use serde_json::{Value, json};

/// The estimated context tokens of a session whose agent holds `messages`.
fn context_tokens(messages: &[Value]) -> Option<f64> {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            context_window: Some(100_000.0),
            ..HarnessOptions::default()
        })
        .await;
        let history: Vec<NativeAgentMessage> = serde_json::from_value(json!(messages)).unwrap();
        h.session
            .agent()
            .state()
            .write()
            .unwrap()
            .set_messages(history);
        h.session.get_context_usage().unwrap().unwrap().tokens
    })
}

#[test]
fn user_image_attachment_counts_toward_context_usage() {
    let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
    let tokens = context_tokens(&[user(&json!([image]), 1)]);
    assert_eq!(tokens, Some(1200.0));
}

#[test]
fn all_zero_assistant_usage_estimates_from_the_previous_usage() {
    let zero = assistant(
        "partial",
        4,
        &json!({"usage": usage([0, 0, 0, 0], 0.0, None)}),
    );
    let answered = json!({"usage": usage([300, 200, 0, 0], 0.0, Some(500))});
    let tokens = context_tokens(&[
        user(&json!("hello"), 1),
        assistant("answer", 2, &answered),
        user(&json!("again"), 3),
        zero,
    ]);
    assert_eq!(tokens, Some(500.0 + 2.0 + 2.0));
}

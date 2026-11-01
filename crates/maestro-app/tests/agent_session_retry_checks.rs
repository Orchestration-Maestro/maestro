//! Retry settlement in a complete session: the retry budget, a provider network failure and
//! a recovered tool turn that settles before a follow-up prompt.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, delivered_events, echo_tools, get_assistant_texts,
    get_message_text, harness, on_task_set, retry_order, start_prompt,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// A session over the scripted stream answering `responses` under the `retry` settings.
async fn retrying(responses: Vec<Scripted>, retry: Value) -> Harness {
    harness(HarnessOptions {
        responses: Some(responses),
        settings: json!({"retry": retry}),
        ..HarnessOptions::default()
    })
    .await
}

#[test]
fn retry_budget_ends_with_failure() {
    on_task_set(|_| async {
        let h = retrying(
            vec![Scripted::failure("overloaded_error"); 3],
            json!({"enabled": true, "maxRetries": 2, "baseDelayMs": 1}),
        )
        .await;
        let events = delivered_events(&h.session);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        let events = events.borrow();
        assert_eq!(retry_order(&events), ["start:1", "start:2", "end:false"]);
        let starts: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_start")
            .collect();
        let ends: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_end")
            .collect();
        assert_eq!(h.requests.0.borrow().len(), 3);
        assert_eq!(starts[0]["delayMs"], json!(1));
        assert_eq!(starts[1]["delayMs"], json!(2));
        assert_eq!(ends[0]["attempt"], json!(2));
        assert_eq!(ends[0]["finalError"], json!("overloaded_error"));
        assert!(!h.session.is_retrying());
        assert!(!h.session.is_streaming());
    });
}

#[test]
fn provider_network_error_recovers() {
    on_task_set(|_| async {
        let h = retrying(
            vec![
                Scripted::failure("Provider finish_reason: network_error"),
                Scripted::text("Recovered after retry"),
            ],
            json!({"enabled": true, "maxRetries": 3, "baseDelayMs": 1}),
        )
        .await;
        let events = delivered_events(&h.session);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        let events = events.borrow();
        assert_eq!(retry_order(&events), ["start:1", "end:true"]);
        let starts: Vec<&Value> = events
            .iter()
            .filter(|event| event["type"] == "auto_retry_start")
            .collect();
        assert_eq!(h.requests.0.borrow().len(), 2);
        assert_eq!(
            starts[0]["errorMessage"],
            json!("Provider finish_reason: network_error")
        );
        assert_eq!(get_assistant_texts(&h), ["Recovered after retry"]);
    });
}

/// A prompt whose first answer fails with `failure` is retried once and then recovers.
fn recovers_after(failure: &'static str) {
    on_task_set(|_| async {
        let h = retrying(
            vec![Scripted::failure(failure), Scripted::text("Recovered")],
            json!({"enabled": true, "maxRetries": 3, "baseDelayMs": 1}),
        )
        .await;
        let events = delivered_events(&h.session);
        start_prompt(&h, "hi").await.unwrap().unwrap();
        assert_eq!(retry_order(&events.borrow()), ["start:1", "end:true"]);
        assert_eq!(h.requests.0.borrow().len(), 2);
        assert_eq!(get_assistant_texts(&h), ["Recovered"]);
    });
}

#[test]
fn anthropic_stream_ended_before_message_stop_recovers() {
    recovers_after("Anthropic stream ended before message_stop");
}

#[test]
fn upstream_request_buffer_limit_recovers() {
    recovers_after("Error: exceeded request buffer limit while retrying upstream");
}

#[test]
fn recovered_text_and_tool_turn_settle_before_follow_up() {
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let h = harness(HarnessOptions {
            responses: Some(vec![
                Scripted::failure("overloaded_error"),
                Scripted {
                    text: Some("Looking that up now.".into()),
                    tool_calls: vec![("echo".into(), json!({"text": "hello"}))],
                    ..Scripted::default()
                },
                Scripted::text("Final answer."),
                Scripted::text("Follow-up answer."),
            ]),
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 1}}),
            base_tools_override: Some(echo_tools(Rc::clone(&executed))),
            ..HarnessOptions::default()
        })
        .await;
        start_prompt(&h, "hi").await.unwrap().unwrap();
        assert_eq!(h.requests.0.borrow().len(), 3);
        assert!(*executed.borrow(), "the retried turn ran the tool");
        let results: Vec<String> = {
            let messages = h.session.messages();
            let messages = messages.read().unwrap();
            messages
                .iter()
                .map(|message| serde_json::to_value(message).unwrap())
                .filter(|record| record["role"] == "toolResult")
                .map(|record| get_message_text(&record))
                .collect()
        };
        assert_eq!(results, ["echoed"]);
        assert_eq!(
            get_assistant_texts(&h).last().map(String::as_str),
            Some("Final answer.")
        );
        assert!(!h.session.is_streaming());
        start_prompt(&h, "follow-up").await.unwrap().unwrap();
        assert_eq!(h.requests.0.borrow().len(), 4);
    });
}

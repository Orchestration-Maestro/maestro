//! Continuation selection, projection and payload witnesses for the cached-context transport.

use super::super::continuation::{Continuation, Request, project, select};
use super::super::debug::{
    get_openai_codex_web_socket_debug_stats, reset_openai_codex_web_socket_debug_stats,
};
use super::super::sessions::close_openai_codex_web_socket_sessions;
use super::super::websocket::wire_body;
use super::loopback::{Conversation, answer, read_request, respond};
use super::socket_transport::{accept, listen, run_native};
use crate::providers::json_text::compact_json;
use crate::providers::responses::openai_responses_shared::{
    ConvertResponsesMessagesOptions, messages::convert_responses_messages,
};
use crate::{AssistantMessage, Model, Transport};
use futures_util::future::join;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// A request body and the retained context it is compared with.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuationCase {
    /// Inputs of the selection.
    input: ContinuationInput,
    /// What it sends and keeps.
    expected: ContinuationExpected,
}

/// Recorded JSON texts of one selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuationInput {
    /// Request body.
    body: String,
    /// Retained context, absent when the connection has none.
    continuation: Option<String>,
}

/// Request body text and whether the context survives.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContinuationExpected {
    /// The body that is sent.
    body: String,
    /// The retained context is kept.
    continuation_retained: bool,
}

/// A retained context as recorded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Retained {
    /// Body of the completed request.
    #[serde(rename = "lastRequestBody")]
    request_body: Value,
    /// Identifier of its response.
    #[serde(rename = "lastResponseId")]
    response_id: String,
    /// Items that response contributes.
    #[serde(rename = "lastResponseItems")]
    response_items: Vec<Value>,
}

/// Arrays wrapped around a number.
fn nested(depth: usize) -> Value {
    (0..depth).fold(json!(0), |inner, _| Value::Array(vec![inner]))
}

/// Replace the `extra` member wherever it occurs.
fn restore(value: &mut Value, depth: usize) {
    match value {
        Value::Object(members) => {
            for (name, member) in members {
                if name == "extra" {
                    *member = nested(depth);
                } else {
                    restore(member, depth);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| restore(item, depth)),
        _ => {}
    }
}

/// Decode recorded JSON. A text nested beyond the decoder's depth limit is decoded with a shallow
/// stand-in for its one deep member, which is then rebuilt by wrapping and must compact to the
/// recorded text.
fn recorded(text: &str) -> Value {
    if let Ok(value) = serde_json::from_str(text) {
        return value;
    }
    let start = text.find("[[[[").unwrap();
    let depth = text[start..]
        .bytes()
        .take_while(|byte| *byte == b'[')
        .count();
    let end = start + 2 * depth + 1;
    assert_eq!(&text[start + depth..=start + depth], "0");
    assert!(
        text[start + depth + 1..end]
            .bytes()
            .all(|byte| byte == b']')
    );
    let shallow = format!("{}[0]{}", &text[..start], &text[end..]);
    let mut value: Value = serde_json::from_str(&shallow).unwrap();
    restore(&mut value, depth);
    assert_eq!(compact_json(&value).unwrap(), text);
    value
}

#[test]
fn maestro_response_sessions_send_cached_input_delta() {
    let rows: Vec<ContinuationCase> = super::fixture_rows(
        include_str!("fixtures/socket_continuation.json"),
        &["input"],
    )
    .unwrap();
    assert_eq!(rows.len(), 30);
    for (index, row) in rows.iter().enumerate() {
        let body = recorded(&row.input.body);
        let slot = Mutex::new(row.input.continuation.as_deref().map(|text| {
            let retained: Retained = serde_json::from_value(recorded(text)).unwrap();
            Continuation::new(
                &retained.request_body,
                &retained.response_id,
                retained.response_items,
            )
            .unwrap()
        }));
        let sent = select(&slot, &body)
            .unwrap_or_else(|| Request::full(&body).unwrap())
            .wire;
        let expected = wire_body(&recorded(&row.expected.body)).unwrap();
        assert_eq!(sent, expected, "case {index}");
        assert_eq!(
            slot.lock().unwrap().is_some(),
            row.expected.continuation_retained,
            "case {index}"
        );
    }
    let _isolated = super::exclusive();
    send_delta_over_the_wire();
}

/// The request message with its members in the order they were sent.
fn member_names(request: &Value) -> Vec<&str> {
    request
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

/// The second request of a cached conversation carries only the input suffix on the wire.
fn send_delta_over_the_wire() {
    run_native(async {
        let (listener, url) = listen().await;
        let mut chat =
            Conversation::new(&url, Some("delta-wire"), Some(Transport::WebsocketCached)).await;
        let server = async {
            let mut socket = accept(&listener).await;
            let first = answer(&mut socket, "r1", "answer").await;
            let second = answer(&mut socket, "r2", "again").await;
            (first, second)
        };
        let client = async {
            let first = chat.send().await;
            first.result.unwrap();
            chat.follow(&first.output, "next");
            chat.send().await.result.unwrap();
        };
        let ((first, second), ()) = join(server, client).await;
        assert!(first.get("previous_response_id").is_none());
        assert_eq!(first["input"].as_array().unwrap().len(), 1);
        assert_eq!(second["previous_response_id"], "r1");
        let next = json!([{"role":"user","content":[{"type":"input_text","text":"next"}]}]);
        assert_eq!(second["input"], next);
        close_openai_codex_web_socket_sessions(Some("delta-wire"));
    });
}

#[test]
fn maestro_response_sessions_send_full_context_without_cache_mode() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let mut chat = Conversation::new(&url, Some("modes"), None).await;
        let modes = [
            (Some(Transport::Websocket), None),
            (None, None),
            (Some(Transport::WebsocketCached), None),
            (Some(Transport::Auto), Some("r3")),
            (Some(Transport::Websocket), None),
            (Some(Transport::WebsocketCached), Some("r4")),
        ];
        let server = async {
            let mut socket = accept(&listener).await;
            let mut sent = Vec::new();
            for turn in 1..=modes.len() {
                sent.push(answer(&mut socket, &format!("r{turn}"), &format!("t{turn}")).await);
            }
            sent
        };
        let client = async {
            for (turn, (transport, _)) in modes.iter().enumerate() {
                chat.setup.options.common.transport.clone_from(transport);
                let outcome = chat.send().await;
                outcome.result.unwrap();
                chat.follow(&outcome.output, &format!("u{}", turn + 2));
            }
        };
        reset_openai_codex_web_socket_debug_stats(Some("modes"));
        let (sent, ()) = join(server, client).await;
        for (turn, (request, (_, previous))) in sent.iter().zip(&modes).enumerate() {
            assert_eq!(
                request.get("previous_response_id").and_then(Value::as_str),
                *previous,
                "request {}",
                turn + 1
            );
        }
        let inputs: Vec<usize> = sent
            .iter()
            .map(|r| r["input"].as_array().unwrap().len())
            .collect();
        assert_eq!(inputs, [1, 3, 5, 1, 9, 3]);
        let stats = get_openai_codex_web_socket_debug_stats("modes").unwrap();
        assert_eq!(
            (stats.connections_created, stats.connections_reused),
            (1, 5)
        );
        close_openai_codex_web_socket_sessions(Some("modes"));
        reset_openai_codex_web_socket_debug_stats(Some("modes"));
    });
}

/// A model and a reduced assistant message as recorded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionCase {
    /// Inputs of the projection.
    input: ProjectionInput,
    /// The projected items.
    expected: ProjectionExpected,
}

/// The model and the message it produced.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionInput {
    /// Model the message belongs to.
    #[serde(deserialize_with = "super::consumed")]
    model: Model,
    /// Reduced message.
    #[serde(deserialize_with = "super::consumed")]
    message: AssistantMessage,
}

/// Compact text of the projected items.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionExpected {
    /// Compact JSON array.
    items: String,
}

/// The full converter's items for a history holding only this message, before and after the
/// function results are filtered out.
fn full_conversion(model: &Model, message: &AssistantMessage) -> (Vec<Value>, Vec<Value>) {
    let context = serde_json::from_value(json!({"messages": [message]})).unwrap();
    let providers = HashSet::from(["openai-codex".to_owned()]);
    let options = ConvertResponsesMessagesOptions {
        include_system_prompt: false,
    };
    let all = convert_responses_messages(model, &context, &providers, Some(&options)).unwrap();
    let kept = all
        .iter()
        .filter(|item| item["type"] != "function_call_output")
        .cloned()
        .collect();
    (all, kept)
}

#[test]
fn maestro_response_sessions_project_socket_response_items() {
    let rows: Vec<ProjectionCase> =
        super::fixture_rows(include_str!("fixtures/socket_projection.json"), &["input"]).unwrap();
    assert_eq!(rows.len(), 8);
    for (index, row) in rows.into_iter().enumerate() {
        let ProjectionInput { model, message } = row.input;
        let calls = serde_json::to_value(&message).unwrap()["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| block["type"] == "toolCall")
            .count();
        let output = Arc::new(std::sync::RwLock::new(message.clone()));
        let borrowed = project(&output, &model).unwrap();
        let text = compact_json(&Value::Array(borrowed.clone())).unwrap();
        assert_eq!(text, row.expected.items, "case {index}");
        let (all, filtered) = full_conversion(&model, &message);
        assert_eq!(borrowed, filtered, "case {index}");
        let skipped = matches!(
            message.stop_reason,
            crate::StopReason::Error | crate::StopReason::Aborted
        );
        if calls > 0 && !skipped {
            assert_eq!(all.len() - filtered.len(), calls, "case {index}");
        }
    }
    let _isolated = super::exclusive();
    retain_context_after_reduction();
}

/// Retention follows reduction: a response without an identifier leaves the context unchanged,
/// and a request that mismatches it clears the context for good.
fn retain_context_after_reduction() {
    run_native(async {
        let (listener, url) = listen().await;
        let mut chat =
            Conversation::new(&url, Some("retain"), Some(Transport::WebsocketCached)).await;
        let server = async {
            let mut socket = accept(&listener).await;
            let mut sent = Vec::new();
            for id in [Some("r1"), None, None, None] {
                sent.push(read_request(&mut socket).await);
                respond(&mut socket, id, "text").await;
            }
            sent.push(answer(&mut socket, "r5", "text").await);
            sent
        };
        let client = async {
            let first = chat.send().await;
            first.result.unwrap();
            chat.follow(&first.output, "u2");
            let second = chat.send().await;
            second.result.unwrap();
            chat.follow(&second.output, "u3");
            let third = chat.send().await;
            third.result.unwrap();
            chat.restart("different");
            let fourth = chat.send().await;
            fourth.result.unwrap();
            chat.follow(&fourth.output, "u2");
            chat.send().await.result.unwrap();
        };
        let (sent, ()) = join(server, client).await;
        let previous: Vec<Option<&str>> = sent
            .iter()
            .map(|r| r.get("previous_response_id").and_then(Value::as_str))
            .collect();
        assert_eq!(previous, [None, Some("r1"), Some("r1"), None, None]);
        let inputs: Vec<usize> = sent
            .iter()
            .map(|r| r["input"].as_array().unwrap().len())
            .collect();
        assert_eq!(inputs, [1, 1, 3, 1, 3]);
        close_openai_codex_web_socket_sessions(Some("retain"));
    });
}

/// Run two cached-transport requests whose prepared body is edited by `edit`, returning both
/// message texts as the peer received them.
async fn two_requests(session: &str, edit: fn(&mut Value)) -> (Value, Value) {
    let (listener, url) = listen().await;
    let mut chat = Conversation::new(&url, Some(session), Some(Transport::WebsocketCached)).await;
    let hooks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = Arc::clone(&hooks);
    chat.setup.options.common.on_payload = Some(Arc::new(move |mut payload, _| {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        edit(&mut payload);
        Box::pin(async move { Ok(payload) })
    }));
    let server = async {
        let mut socket = accept(&listener).await;
        let first = answer(&mut socket, "r1", "answer").await;
        let second = answer(&mut socket, "r2", "again").await;
        (first, second)
    };
    let client = async {
        let first = chat.send().await;
        first.result.unwrap();
        chat.follow(&first.output, "next");
        chat.send().await.result.unwrap();
    };
    let (sent, ()) = join(server, client).await;
    assert_eq!(
        hooks.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "one hook call per request"
    );
    close_openai_codex_web_socket_sessions(Some(session));
    sent
}

#[test]
fn maestro_response_sessions_preserve_open_socket_payloads() {
    let _isolated = super::exclusive();
    run_native(async {
        let (first, second) = two_requests("open-fields", |payload| {
            payload["unknown"] = json!({"z": 1, "a": [1, 2, 2]});
            payload["store"] = json!(true);
        })
        .await;
        let mut expected = member_names(&first);
        expected.push("previous_response_id");
        assert_eq!(member_names(&second), expected);
        assert_eq!(second["unknown"], first["unknown"]);
        assert_eq!(second["store"], true);

        let (first, second) = two_requests("open-slot", |payload| {
            payload["store"] = Value::Null;
            payload["previous_response_id"] = Value::Null;
        })
        .await;
        assert_eq!(first["previous_response_id"], Value::Null);
        assert_eq!(member_names(&second), member_names(&first));
        assert_eq!(second["previous_response_id"], "r1");
        assert_eq!(second["store"], Value::Null);

        let (first, second) = two_requests("open-empty-id", |payload| {
            payload["previous_response_id"] = json!("");
        })
        .await;
        assert_eq!(first["previous_response_id"], "");
        assert_eq!(second["previous_response_id"], "r1");

        let (first, second) = two_requests("open-missing-input", |payload| {
            drop(payload.as_object_mut().unwrap().remove("input"));
        })
        .await;
        assert!(first.get("input").is_none() && second.get("input").is_none());
        assert!(second.get("previous_response_id").is_none());

        let (first, second) =
            two_requests("open-null-input", |payload| payload["input"] = Value::Null).await;
        assert_eq!(first["input"], Value::Null);
        assert_eq!(second["input"], Value::Null);

        let (first, second) =
            two_requests("open-text-input", |payload| payload["input"] = json!("Ω😀")).await;
        assert_eq!(
            (first["input"].as_str(), second["input"].as_str()),
            (Some("Ω😀"), Some("Ω😀"))
        );
        assert!(second.get("previous_response_id").is_none());
        let stats = get_openai_codex_web_socket_debug_stats("open-text-input").unwrap();
        assert_eq!(stats.last_input_items, 3);
        reset_openai_codex_web_socket_debug_stats(None);
    });
}

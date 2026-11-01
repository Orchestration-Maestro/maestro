//! Streaming behavior of the standard response endpoint.
use crate::chat;
use crate::response_cases;
use crate::response_endpoint as endpoint;
use crate::response_runtime as runtime;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_price_echoed_tier_before_requested_tier() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_price_echoed_tier_before_requested_tier",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_run_hooks_before_body_and_propagate_failures() -> chat::TestResult {
    crate::runtime::run(async {
        endpoint::assert_rows("responses_run_hooks_before_body_and_propagate_failures").await?;
        let failed =
            endpoint::run_case(&serde_json::json!({"hook":"response_failure","cancel":"response"}))
                .await?;
        assert_eq!(failed["result"]["errorMessage"], "response failed");
        assert_eq!(failed["result"]["stopReason"], "aborted");
        assert_eq!(
            failed["events"],
            serde_json::json!([{"type":"error","reason":"aborted"}])
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_finish_success_or_error_once_with_partial_output() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_finish_success_or_error_once_with_partial_output",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_preserve_cancellation_stage_and_message() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_preserve_cancellation_stage_and_message",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_return_http_failure_without_start() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_return_http_failure_without_start",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_interpret_named_events_and_done_prefix() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_interpret_named_events_and_done_prefix",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_map_terminal_outcomes_without_replacing_usage() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_map_terminal_outcomes_without_replacing_usage",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_keep_earlier_setup_failures_ahead_of_hooks() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_keep_earlier_setup_failures_ahead_of_hooks",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_preserve_raw_effort_endpoint_outcomes() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_preserve_raw_effort_endpoint_outcomes",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_compute_catalog_costs_for_priority_and_flex() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "responses_compute_catalog_costs_for_priority_and_flex",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_native_endpoint_delivers_both_entry_results() -> chat::TestResult {
    crate::runtime::run(runtime::native(runtime::Endpoint::Standard))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_gate_hooks_and_cancel_a_pending_body() -> chat::TestResult {
    crate::runtime::run(runtime::gated(runtime::Endpoint::Standard))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_forward_retry_timeout_and_cancellation_options() -> chat::TestResult {
    crate::runtime::run_paused(runtime::transport_options(runtime::Endpoint::Standard))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_native_runtime_failure_finishes_stream() -> chat::TestResult {
    runtime::without_runtime(runtime::Endpoint::Standard)
}

#[test]
fn responses_fixture_cases_have_complete_consumers() -> chat::TestResult {
    runtime::fixture_consumers(
        endpoint::FIXTURE,
        response_cases::rows,
        concat!(
            include_str!("response_endpoint_requests.rs"),
            include_str!("response_endpoint_streams.rs")
        ),
        (28, 435),
        |q| Ok(serde_json::to_string(q)?),
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_errors_do_not_append_chat_metadata() -> chat::TestResult {
    crate::runtime::run(async {
        for (status, body, expected) in [
            (
                400,
                r#"{"error":{"message":"bad","metadata":{"raw":"detail"}}}"#,
                "400 bad",
            ),
            (
                200,
                "data: {\"error\":{\"message\":\"bad\",\"metadata\":{\"raw\":\"detail\"}}}\n\n",
                "bad",
            ),
        ] {
            let actual =
                endpoint::run_case(&serde_json::json!({"status":status,"body":body})).await?;
            assert_eq!(actual["result"]["errorMessage"], expected);
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_edited_stream_controls_processing_and_failure_order() -> chat::TestResult {
    crate::runtime::run(async {
        let null = endpoint::run_case(&serde_json::json!({"hook":"null"})).await?;
        assert_eq!(null["requests"], serde_json::json!([]));
        assert_eq!(null["hooks"], serde_json::json!(["payload"]));
        assert_eq!(
            null["events"],
            serde_json::json!([{"type":"error","reason":"error"}])
        );
        assert!(
            !null["result"]["errorMessage"]
                .as_str()
                .ok_or("error")?
                .is_empty()
        );
        let malformed = endpoint::run_case(&serde_json::json!({
            "hook":"nonstream","contentType":"application/json","body":"{"
        }))
        .await?;
        assert_eq!(malformed["requests"][0]["body"]["stream"], false);
        assert_eq!(malformed["hooks"], serde_json::json!(["payload"]));
        assert_eq!(
            malformed["events"],
            serde_json::json!([{"type":"error","reason":"error"}])
        );
        let parsed = endpoint::run_case(&serde_json::json!({
            "hook":"nonstream","contentType":"application/json","body":"{}"
        }))
        .await?;
        assert_eq!(parsed["requests"][0]["body"]["stream"], false);
        assert_eq!(parsed["hooks"][1][0], "response");
        assert_eq!(
            parsed["events"],
            serde_json::json!([{"type":"start"},{"type":"error","reason":"error"}])
        );
        assert_ne!(
            parsed["result"]["errorMessage"],
            "Response stream ended before a terminal event"
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_render_selected_error_json_without_erasing_failures() -> chat::TestResult {
    crate::runtime::run(async {
        let deep = format!("{}0{}", "[".repeat(128), "]".repeat(128));
        for (error, expected) in [
            (
                r#"{"message":{"x":"retained"}}"#.to_owned(),
                r#"{"x":"retained"}"#,
            ),
            (
                r#"{"message":{"z":"value","a":"decoy"}}"#.to_owned(),
                r#"{"z":"value","a":"decoy"}"#,
            ),
            (format!(r#"{{"message":"kept","unread":{deep}}}"#), "kept"),
        ] {
            for status in [200, 400] {
                let actual = error_message(status, &error).await?;
                let expected = [expected.to_owned(), format!("400 {expected}")]
                    [usize::from(status == 400)]
                .clone();
                assert_eq!(actual, expected);
            }
        }
        for (error, diagnostic) in [
            (
                format!(r#"{{"message":{deep}}}"#),
                "recursion limit exceeded",
            ),
            (r#"{"message":{"x":"\ud800"}}"#.to_owned(), "hex escape"),
            (r#"{"message":{"\ud800":"value"}}"#.to_owned(), "utf-8"),
        ] {
            for status in [200, 400] {
                let actual = error_message(status, &error).await?;
                assert!(
                    actual.contains(diagnostic),
                    "status {status}, expected {diagnostic}: {actual}"
                );
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Consume a selected error through the HTTP or SSE public invocation.
async fn error_message(status: u16, error: &str) -> chat::TestResult<String> {
    let envelope = format!(r#"{{"error":{error}}}"#);
    let body = if status == 200 {
        format!("data: {envelope}\n\n")
    } else {
        envelope
    };
    let actual = endpoint::run_case(&serde_json::json!({"status":status,"body":body})).await?;
    Ok(actual["result"]["errorMessage"]
        .as_str()
        .ok_or("error")?
        .to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_forward_array_payload_unchanged() -> chat::TestResult {
    crate::runtime::run(async {
        let actual = endpoint::run_case(&serde_json::json!({"hook":"array","status":400,"body":"{\"error\":{\"message\":\"array rejected\"}}"})).await?;
        assert_eq!(
            actual["requests"][0]["body"],
            serde_json::json!(["replacement", {"retained":true}])
        );
        assert_eq!(actual["result"]["errorMessage"], "400 array rejected");
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_keep_body_failure_text_when_aborted() -> chat::TestResult {
    crate::runtime::run(async {
        for entry in ["raw", "simple"] {
            let actual =
                endpoint::run_case(&serde_json::json!({"entry":entry,"cancel":"body_failure"}))
                    .await?;
            assert_eq!(actual["result"]["stopReason"], "aborted");
            assert_eq!(actual["result"]["errorMessage"], "body failed");
            assert_eq!(
                actual["events"],
                serde_json::json!([{"type":"start"},{"type":"error","reason":"aborted"}])
            );
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_trim_media_type_with_script_whitespace() -> chat::TestResult {
    crate::runtime::run(async {
        for (content_type, response_hook) in [
            ("application/example+json\u{0085}", false),
            ("\u{feff}application/example+json\u{feff}", true),
        ] {
            let actual = endpoint::run_case(&serde_json::json!({
                "hook":"nonstream","contentType":content_type,"body":"{}"
            }))
            .await?;
            if response_hook {
                assert_eq!(actual["hooks"][1][0], "response");
            } else {
                assert_eq!(actual["hooks"], serde_json::json!(["payload"]));
                assert_eq!(
                    actual["result"]["errorMessage"],
                    "Response body is not an object"
                );
                assert_eq!(
                    actual["events"],
                    serde_json::json!([{"type":"error","reason":"error"}])
                );
            }
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn responses_send_false_payload_as_json() -> chat::TestResult {
    crate::runtime::run(async {
        for entry in ["raw", "simple"] {
            let case = serde_json::json!({
                "entry":entry,"hook":"scalar","payload":false
            });
            let actual = endpoint::run_case(&case).await?;
            assert_eq!(actual["requests"][0]["wire"], serde_json::json!(b"false"));
            assert_eq!(
                actual["requests"][0]["headers"]["content-type"],
                "application/json"
            );
        }
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_responses_emit_carriage_delimited_events() -> chat::TestResult {
    use futures_util::FutureExt as _;
    use maestro_models::{AssistantMessageEvent, OpenAIResponsesOptions, StreamOptions};
    use std::sync::Arc;

    crate::runtime::run(async {
        let (fetch, reached, release) = carriage_transport();
        let model =
            chat::model(&serde_json::json!({"api":"openai-responses","provider":"openai"}))?;
        let context = chat::context(&serde_json::json!({"messages":[]}))?;
        let stream =
            maestro_models::providers::responses::openai_responses::stream_openai_responses(
                model,
                context,
                Some(OpenAIResponsesOptions {
                    common: StreamOptions {
                        api_key: Some("fixture-key".into()),
                        fetch: Some(fetch),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        reached.await?;
        let mut partial = None;
        while let Some(Some(event)) = stream.next().now_or_never() {
            if let AssistantMessageEvent::TextDelta {
                partial: message,
                delta,
                ..
            } = event
            {
                assert_eq!(delta, "carriage");
                partial = Some(message);
            }
        }
        let partial = partial.ok_or("CRCR must emit text before the second body poll")?;
        let snapshot = partial.read().map_err(|e| e.to_string())?.clone();
        assert_eq!(
            serde_json::to_value(snapshot)?["content"][0]["text"],
            "carriage"
        );
        release.send(()).map_err(|()| "body stopped waiting")?;
        let (events, result) = endpoint::collect(&stream).await?;
        assert_eq!(
            events.last(),
            Some(&serde_json::json!({"type":"done","reason":"stop"}))
        );
        assert_eq!(result["content"][0]["text"], "carriage");
        assert!(Arc::ptr_eq(&partial, &stream.result().await));
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A finite body whose second poll is acknowledged before terminal bytes are released.
fn carriage_transport() -> (
    maestro_models::Fetch,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    use futures_util::StreamExt as _;
    use maestro_models::HttpBody;
    use std::sync::{Arc, Mutex, PoisonError};

    let (reached_tx, reached) = tokio::sync::oneshot::channel();
    let (release, release_rx) = tokio::sync::oneshot::channel::<()>();
    let head = b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"m\",\"content\":[]}}\r\rdata: {\"type\":\"response.content_part.added\",\"part\":{\"type\":\"output_text\",\"text\":\"\"}}\r\rdata: {\"type\":\"response.output_text.delta\",\"delta\":\"carriage\"}\r\r".to_vec();
    let body: HttpBody = Box::pin(futures_util::stream::once(async { Ok(head) }).chain(
        futures_util::stream::once(async move {
            reached_tx.send(()).ok();
            release_rx.await.ok();
            Ok(endpoint::COMPLETE.as_bytes().to_vec())
        }),
    ));
    let body = Arc::new(Mutex::new(Some(body)));
    let fetch: maestro_models::Fetch = Arc::new(move |_| {
        let body = body.lock().unwrap_or_else(PoisonError::into_inner).take();
        Box::pin(async move {
            let body = body.ok_or(maestro_models::FetchError::Aborted)?;
            Ok(maestro_models::HttpResponse {
                status: 200,
                status_text: String::new(),
                headers: std::collections::BTreeMap::default(),
                body,
            })
        })
    });
    (fetch, reached, release)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn response_argument_roots_survive_each_completion_route() -> chat::TestResult {
    crate::runtime::run(async {
        for row in crate::root_routes::recorded()?.responses {
            let (feed, body) = crate::messages::feed();
            let stream =
                maestro_models::providers::responses::openai_responses::stream_openai_responses(
                    chat::model(
                        &serde_json::json!({"api":"openai-responses","provider":"openai"}),
                    )?,
                    chat::context(&serde_json::json!({"messages":[]}))?,
                    Some(maestro_models::OpenAIResponsesOptions {
                        common: maestro_models::StreamOptions {
                            api_key: Some("fixture-key".into()),
                            fetch: Some(crate::root_routes::body_fetch(body)),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                );
            crate::root_routes::gated(
                &stream,
                feed,
                row.chunks.into_iter().map(String::into_bytes).collect(),
                row.expected,
            )
            .await?;
        }
        Ok(())
    })
}

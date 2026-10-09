//! Streaming behavior of the standard response endpoint.
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/response_endpoint.rs"]
mod endpoint;
#[path = "support/json.rs"]
mod json;
#[allow(
    dead_code,
    reason = "Endpoint tests use the finite loopback exchange only."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[path = "support/response_cases.rs"]
mod response_cases;
#[path = "support/response_runtime.rs"]
mod runtime;
#[allow(
    dead_code,
    reason = "Endpoint tests use the scripted attempt queue only."
)]
#[path = "support/transport.rs"]
mod transport;

#[test]
fn responses_price_echoed_tier_before_requested_tier() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_price_echoed_tier_before_requested_tier"),
    )
}

#[test]
fn responses_run_hooks_before_body_and_propagate_failures() -> chat::TestResult {
    chat::block_on(false, async {
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

#[test]
fn responses_finish_success_or_error_once_with_partial_output() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_finish_success_or_error_once_with_partial_output"),
    )
}

#[test]
fn responses_preserve_cancellation_stage_and_message() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_preserve_cancellation_stage_and_message"),
    )
}

#[test]
fn responses_return_http_failure_without_start() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_return_http_failure_without_start"),
    )
}

#[test]
fn responses_interpret_named_events_and_done_prefix() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_interpret_named_events_and_done_prefix"),
    )
}

#[test]
fn responses_map_terminal_outcomes_without_replacing_usage() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_map_terminal_outcomes_without_replacing_usage"),
    )
}

#[test]
fn responses_keep_earlier_setup_failures_ahead_of_hooks() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_keep_earlier_setup_failures_ahead_of_hooks"),
    )
}

#[test]
fn responses_preserve_raw_effort_endpoint_outcomes() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_preserve_raw_effort_endpoint_outcomes"),
    )
}

#[test]
fn responses_compute_catalog_costs_for_priority_and_flex() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_compute_catalog_costs_for_priority_and_flex"),
    )
}

#[test]
fn responses_native_endpoint_delivers_both_entry_results() -> chat::TestResult {
    chat::block_on(false, runtime::native())
}

#[test]
fn responses_gate_hooks_and_cancel_a_pending_body() -> chat::TestResult {
    chat::block_on(false, runtime::gated())
}

#[test]
fn responses_forward_retry_timeout_and_cancellation_options() -> chat::TestResult {
    chat::block_on(true, runtime::transport_options())
}

#[test]
fn responses_native_runtime_failure_finishes_stream() -> chat::TestResult {
    runtime::without_runtime()
}

#[test]
fn responses_fixture_cases_have_complete_consumers() -> chat::TestResult {
    runtime::fixture_consumers()
}

#[test]
fn responses_errors_do_not_append_chat_metadata() -> chat::TestResult {
    chat::block_on(false, async {
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

#[test]
fn responses_edited_stream_controls_processing_and_failure_order() -> chat::TestResult {
    chat::block_on(false, async {
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

#[test]
fn responses_render_selected_error_json_without_erasing_failures() -> chat::TestResult {
    chat::block_on(false, async {
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

//! Public content-generation invocation behavior.
#![allow(
    dead_code,
    reason = "shared chat support serves multiple test binaries"
)]

use crate::{bundled, chat, response_output};
use chat::TestResult;

#[test]
fn raw_request_projects_the_selected_payload() -> TestResult {
    content_invocation::assert_rows("raw_request_projects_the_selected_payload")
}

#[cfg(test)]
#[path = "support/content_invocation.rs"]
mod content_invocation;

#[test]
fn segments_accept_all_declared_delimiters() -> TestResult {
    content_invocation::assert_rows("segments_accept_all_declared_delimiters")
}

#[test]
fn null_response_roots_do_not_end_streaming() -> TestResult {
    content_invocation::assert_rows("null_response_roots_do_not_end_streaming")
}

#[test]
fn eof_requires_a_whitespace_only_tail() -> TestResult {
    content_invocation::assert_rows("eof_requires_a_whitespace_only_tail")
}

#[test]
fn bare_error_objects_are_independent_of_chunk_boundaries() -> TestResult {
    content_invocation::assert_rows("bare_error_objects_are_independent_of_chunk_boundaries")
}

#[test]
fn delimited_bare_errors_preserve_prior_deltas() -> TestResult {
    crate::runtime::run(async {
        let error = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"busy"}}"#;
        let delta =
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"retained\"}]}}]}\n\n";
        for delimiter in ["\n\n", "\r\r", "\r\n\r\n"] {
            for prefix in ["", delta] {
                let body = format!("{prefix}{error}{delimiter}");
                native::assert_error_splits(&body, !prefix.is_empty()).await?;
            }
        }
        let stream = native_stream(StreamOptions {
            fetch: Some(body_fetch(
                format!(": heartbeat\n\nnot JSON\r\r{delta}").into_bytes(),
            )),
            api_key: Some("fixture-key".into()),
            ..Default::default()
        })?;
        let output = stream.result().await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
        assert_eq!(
            serde_json::to_value(&output.read().unwrap().content)?[0]["text"],
            "retained"
        );
        Ok(())
    })
}

#[test]
fn payload_edits_and_replacements_reach_the_transport() -> TestResult {
    content_invocation::assert_rows("payload_edits_and_replacements_reach_the_transport")
}

#[test]
fn payload_failure_prevents_dispatch_and_start() -> TestResult {
    content_invocation::assert_rows("payload_failure_prevents_dispatch_and_start")
}

#[test]
fn projection_failure_precedes_transport_failure() -> TestResult {
    content_invocation::assert_rows("projection_failure_precedes_transport_failure")
}

#[test]
fn raw_and_simple_key_failures_keep_their_boundary() -> TestResult {
    content_invocation::assert_rows("raw_and_simple_key_failures_keep_their_boundary")
}

#[test]
fn explicit_and_environment_keys_keep_precedence() -> TestResult {
    content_invocation::assert_rows("explicit_and_environment_keys_keep_precedence")
}

#[test]
fn common_retry_timeout_and_response_hook_are_unused() -> TestResult {
    content_invocation::assert_rows("common_retry_timeout_and_response_hook_are_unused")?;
    crate::runtime::run_paused(native::common_timeout_ignored())
}

#[test]
fn hook_retry_options_do_not_enable_retries() -> TestResult {
    content_invocation::assert_rows("hook_retry_options_do_not_enable_retries")
}

#[test]
fn dispatch_failure_does_not_publish_start() -> TestResult {
    content_invocation::assert_rows("dispatch_failure_does_not_publish_start")
}

#[test]
fn successful_statuses_start_before_reduction() -> TestResult {
    content_invocation::assert_rows("successful_statuses_start_before_reduction")
}

#[test]
fn bodyless_success_fails_after_start() -> TestResult {
    content_invocation::assert_rows("bodyless_success_fails_after_start")
}

#[test]
fn empty_successful_body_is_a_clean_stop() -> TestResult {
    content_invocation::assert_rows("empty_successful_body_is_a_clean_stop")
}

#[test]
fn http_failures_keep_text_status_and_no_start() -> TestResult {
    content_invocation::assert_rows("http_failures_keep_text_status_and_no_start")
}

#[test]
fn http_json_errors_keep_complete_server_values() -> TestResult {
    content_invocation::assert_rows("http_json_errors_keep_complete_server_values")
}

#[test]
fn content_type_json_detection_is_case_sensitive() -> TestResult {
    content_invocation::assert_rows("content_type_json_detection_is_case_sensitive")
}

#[test]
fn malformed_http_json_fails_before_start() -> TestResult {
    content_invocation::assert_rows("malformed_http_json_fails_before_start")
}

#[test]
fn http_body_read_failure_wins_before_start() -> TestResult {
    content_invocation::assert_rows("http_body_read_failure_wins_before_start")
}

#[test]
fn delimiter_splits_preserve_framing() -> TestResult {
    content_invocation::assert_rows("delimiter_splits_preserve_framing")
}

#[test]
fn mixed_or_incomplete_delimiters_remain_a_tail() -> TestResult {
    content_invocation::assert_rows("mixed_or_incomplete_delimiters_remain_a_tail")
}

#[test]
fn whole_segment_and_payload_trimming_use_the_owned_whitespace_set() -> TestResult {
    content_invocation::assert_rows(
        "whole_segment_and_payload_trimming_use_the_owned_whitespace_set",
    )
}

#[test]
fn only_an_initial_data_field_is_recognized() -> TestResult {
    content_invocation::assert_rows("only_an_initial_data_field_is_recognized")
}

#[test]
fn data_payloads_are_one_json_value_not_multiline_sse() -> TestResult {
    content_invocation::assert_rows("data_payloads_are_one_json_value_not_multiline_sse")
}

#[test]
fn json_whitespace_inside_one_data_payload_is_valid() -> TestResult {
    content_invocation::assert_rows("json_whitespace_inside_one_data_payload_is_valid")
}

#[test]
fn complete_events_survive_a_later_bad_frame() -> TestResult {
    content_invocation::assert_rows("complete_events_survive_a_later_bad_frame")
}

#[test]
fn read_failure_keeps_partial_without_closing_it() -> TestResult {
    content_invocation::assert_rows("read_failure_keeps_partial_without_closing_it")
}

#[test]
fn ignored_frames_do_not_end_streaming() -> TestResult {
    content_invocation::assert_rows("ignored_frames_do_not_end_streaming")
}

#[test]
fn utf8_splits_bom_and_invalid_bytes_follow_text_decoding() -> TestResult {
    content_invocation::assert_rows("utf8_splits_bom_and_invalid_bytes_follow_text_decoding")
}

#[test]
fn trailing_partial_utf8_is_not_a_new_event() -> TestResult {
    content_invocation::assert_rows("trailing_partial_utf8_is_not_a_new_event")
}

#[test]
fn framed_error_members_remain_response_data() -> TestResult {
    content_invocation::assert_rows("framed_error_members_remain_response_data")
}

#[test]
fn terminal_reason_and_partial_content_are_preserved() -> TestResult {
    content_invocation::assert_rows("terminal_reason_and_partial_content_are_preserved")
}

#[test]
fn public_stream_reuses_reduction_and_shared_message_identity() -> TestResult {
    content_invocation::assert_rows("public_stream_reuses_reduction_and_shared_message_identity")?;
    crate::runtime::run(native::live_alias("google-generative-ai"))
}

#[test]
fn preaborted_requests_skip_hooks_and_fetch() -> TestResult {
    content_invocation::assert_rows("preaborted_requests_skip_hooks_and_fetch")
}

#[test]
fn hook_time_abort_is_observed_before_fetch() -> TestResult {
    content_invocation::assert_rows("hook_time_abort_is_observed_before_fetch")
}

#[test]
fn removed_hook_signal_only_changes_transport_cancellation() -> TestResult {
    content_invocation::assert_rows("removed_hook_signal_only_changes_transport_cancellation")
}

#[test]
fn live_abort_releases_pending_body_and_retains_partial() -> TestResult {
    content_invocation::assert_rows("live_abort_releases_pending_body_and_retains_partial")?;
    crate::runtime::run(native::held_body_abort("google-generative-ai"))
}

#[test]
fn only_hook_positive_timeout_controls_dispatch() -> TestResult {
    content_invocation::assert_rows("only_hook_positive_timeout_controls_dispatch")
}

#[test]
fn simple_entry_disables_thinking_through_the_actual_request() -> TestResult {
    content_invocation::assert_rows("simple_entry_disables_thinking_through_the_actual_request")
}

#[test]
fn simple_entry_forwards_reasoning_and_common_options() -> TestResult {
    content_invocation::assert_rows("simple_entry_forwards_reasoning_and_common_options")
}

#[test]
fn registered_raw_options_keep_declared_domains() -> TestResult {
    content_invocation::assert_rows("registered_raw_options_keep_declared_domains")
}

#[test]
fn empty_model_endpoint_uses_the_public_environment_default() -> TestResult {
    content_invocation::assert_rows("empty_model_endpoint_uses_the_public_environment_default")
}

#[test]
fn explicit_model_endpoint_ignores_environment_defaults() -> TestResult {
    content_invocation::assert_rows("explicit_model_endpoint_ignores_environment_defaults")
}

#[test]
fn environment_endpoint_is_captured_before_payload_hooks() -> TestResult {
    content_invocation::assert_rows("environment_endpoint_is_captured_before_payload_hooks")
}

use maestro_models::{
    Cancellation, Context, Fetch, GoogleOptions, HttpResponse, Model, StopReason, StreamOptions,
};
use native::{body_fetch, empty_response, held_body_deadline, native_input, native_stream};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[test]
fn hook_selected_signal_controls_transport_not_terminal_classification() -> TestResult {
    crate::runtime::run(async {
        for mode in [
            "replace-abort-B",
            "replace-abort-A",
            "remove-abort-A",
            "retain-abort-A",
        ] {
            native::assert_selected_signal(mode).await?;
        }
        Ok(())
    })
}

#[test]
fn cancellation_during_setup_releases_the_request() -> TestResult {
    crate::runtime::run(async {
        let (fetch, admission, release) = native::held_fetch(empty_response());
        let caller = Cancellation::new();
        let stream = native_stream(StreamOptions {
            fetch: Some(fetch),
            signal: Some(caller.clone()),
            api_key: Some("fixture-key".into()),
            ..Default::default()
        })?;
        let request_signal = admission.await?.expect("owned transport signal");
        caller.abort();
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Aborted
        );
        assert!(release.is_closed());
        assert!(request_signal.is_aborted());
        assert!(matches!(
            stream.next().await,
            Some(maestro_models::AssistantMessageEvent::Error { .. })
        ));
        assert!(stream.next().await.is_none());
        Ok(())
    })
}

#[test]
fn positive_timeout_covers_body_and_error_admission() -> TestResult {
    crate::runtime::run_paused(async {
        held_body_deadline(200, "google-generative-ai").await?;
        held_body_deadline(500, "google-generative-ai").await
    })
}

#[test]
fn timeout_does_not_cancel_the_callers_signal() -> TestResult {
    crate::runtime::run_paused(async {
        let (fetch, admission, release) = native::held_fetch(empty_response());
        let caller = Cancellation::new();
        let stream = native_stream(StreamOptions {
            fetch: Some(fetch),
            signal: Some(caller.clone()),
            api_key: Some("fixture-key".into()),
            on_payload: Some(Arc::new(
                |maestro_models::PayloadHookInput {
                     mut payload,
                     signal: hook_signal,
                 },
                 _| {
                    payload["config"]["httpOptions"] = json!({"timeout":1});
                    Box::pin(async {
                        Ok(maestro_models::PayloadHookInput {
                            payload,
                            signal: hook_signal,
                        })
                    })
                },
            )),
            ..Default::default()
        })?;
        let request_signal = admission.await?.expect("owned signal");
        tokio::time::advance(std::time::Duration::from_millis(2)).await;
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Error
        );
        assert!(release.is_closed());
        assert!(request_signal.is_aborted());
        assert!(!caller.is_aborted());
        Ok(())
    })
}

#[test]
fn generated_call_numbers_survive_invocation_boundaries() -> TestResult {
    crate::runtime::run(async {
        let mut generated = Vec::new();
        for parts in [
            json!([{"functionCall":{"name":"lookup", "args":{}}}]),
            json!([{"functionCall":{"id":"provided", "name":"lookup", "args":{}}}]),
            json!([{"functionCall":{"id":"duplicate", "name":"lookup", "args":{}}},{"functionCall":{"id":"duplicate", "name":"lookup", "args":{}}}]),
        ] {
            let frame = format!(
                "data: {}\n\n",
                json!({"candidates":[{"content":{"parts":parts}, "finishReason":"STOP"}]})
            );
            let stream = native_stream(StreamOptions {
                fetch: Some(body_fetch(frame.into_bytes())),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            })?;
            let output = stream.result().await;
            let message = output.read().unwrap();
            assert_eq!(message.stop_reason, StopReason::ToolUse);
            generated.extend(native::generated_ids(&message)?);
        }
        assert_eq!(generated.len(), 2);
        assert_eq!(generated[1], generated[0] + 1);
        Ok(())
    })
}

#[test]
fn dropping_the_reader_keeps_the_producer_running() -> TestResult {
    crate::runtime::run(native::dropped_reader("google-generative-ai"))
}

#[test]
fn root_and_registered_entries_invoke_the_content_provider() -> TestResult {
    crate::runtime::run(async {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::clone(&captured);
        let fetch: Fetch = Arc::new(move |request| {
            requests.lock().unwrap().push(request);
            Box::pin(async { Ok(empty_response()) })
        });
        for route in 0..6 {
            let (model, _) = native_input()?;
            let context = chat::context(
                &json!({"messages":[{"role":"user", "content":format!("route {route}")}]}),
            )?;
            let common = StreamOptions {
                fetch: Some(Arc::clone(&fetch)),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            };
            let output = native::route_output(route, model, context, common).await?;
            assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
            let requests = captured.lock().unwrap();
            assert_eq!(requests.len(), route + 1);
            let body: serde_json::Value = serde_json::from_slice(&requests[route].body)?;
            assert_eq!(
                body["contents"][0]["parts"][0]["text"],
                format!("route {route}")
            );
        }
        assert_eq!(bundled::registered(), bundled::registered_apis());
        Ok(())
    })
}

#[test]
fn registered_simple_missing_key_becomes_a_terminal_error() -> TestResult {
    if std::env::var("MAESTRO_CHILD_CASE").ok().is_none() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "registered_simple_missing_key_becomes_a_terminal_error",
            )?
            .nocapture()
            .env_clear()
            .env("MAESTRO_CHILD_CASE", "missing-key")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::runtime::run(async {
        for (aborted, generic) in [(false, false), (false, true), (true, false), (true, true)] {
            let (model, context) = native_input()?;
            let signal = Cancellation::new();
            if aborted {
                signal.abort();
            }
            let options = maestro_models::SimpleStreamOptions {
                common: StreamOptions {
                    api_key: Some(String::new()),
                    signal: Some(signal),
                    ..Default::default()
                },
                ..Default::default()
            };
            let stream = if generic {
                maestro_models::stream_simple(model, context, Some(options))?
            } else {
                maestro_models::stream_simple_google(model, context, Some(options))
            };
            let output = stream.result().await;
            let message = output.read().unwrap();
            assert_eq!(message.stop_reason, StopReason::Error);
            assert_eq!(
                message.error_message.as_deref(),
                Some("No API key for provider: google")
            );
            assert!(message.content.is_empty());
        }
        Ok(())
    })
}

#[test]
fn registered_options_reject_wrong_native_shapes() -> TestResult {
    crate::runtime::run(async {
        let sends = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&sends);
        let fetch: Fetch = Arc::new(move |_| {
            count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Box::pin(async { Ok(empty_response()) })
        });
        for extra in native::malformed_raw_extras() {
            let (model, context) = native_input()?;
            let stream = maestro_models::stream(
                model,
                context,
                Some(maestro_models::ProviderStreamOptions {
                    common: StreamOptions {
                        fetch: Some(Arc::clone(&fetch)),
                        api_key: Some("fixture-key".into()),
                        ..Default::default()
                    },
                    extra: extra.as_object().unwrap().clone(),
                    ..Default::default()
                }),
            )?;
            assert_eq!(
                stream.result().await.read().unwrap().stop_reason,
                StopReason::Error
            );
            assert_eq!(sends.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
        let (model, context) = native_input()?;
        let extra = json!({"unrelated":{"deep":[1,2,3]}})
            .as_object()
            .unwrap()
            .clone();
        let stream = maestro_models::stream(
            model,
            context,
            Some(maestro_models::ProviderStreamOptions {
                common: StreamOptions {
                    fetch: Some(fetch),
                    api_key: Some("fixture-key".into()),
                    ..Default::default()
                },
                extra,
                ..Default::default()
            }),
        )?;
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Stop
        );
        assert_eq!(sends.load(std::sync::atomic::Ordering::Relaxed), 1);
        Ok(())
    })
}

#[cfg(test)]
pub(crate) mod native {
    /// Select request cancellation independently of the caller's terminal signal.
    fn selected_signal_hook(
        mode: &'static str,
        selected: Cancellation,
    ) -> maestro_models::OnPayload {
        Arc::new(move |mut input, _| {
            assert!(input.signal.is_some());
            if mode.starts_with("replace") {
                input.signal = Some(selected.clone());
            } else if mode.starts_with("remove") {
                input.signal = None;
            }
            Box::pin(async { Ok(input) })
        })
    }

    /// Observe the admitted transport's disposal or completion before terminal assertions.
    pub(super) async fn assert_selected_signal(mode: &'static str) -> TestResult {
        let caller = Cancellation::new();
        let replacement = Cancellation::new();
        let selected = replacement.clone();
        let response = HttpResponse {
            body: Box::pin(futures_util::stream::iter([Ok(b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"released\"}]},\"finishReason\":\"STOP\"}]}\n\n".to_vec())])),
            ..empty_response()
        };
        let (fetch, admission, mut release) = native::held_fetch(response);
        let stream = native_stream(StreamOptions {
            fetch: Some(fetch),
            signal: Some(caller.clone()),
            api_key: Some("fixture-key".into()),
            on_payload: Some(selected_signal_hook(mode, selected)),
            ..Default::default()
        })?;
        let request_signal = admission.await?;
        if mode == "replace-abort-B" {
            replacement.abort();
        } else {
            caller.abort();
        }
        let cancels_transport = matches!(mode, "replace-abort-B" | "retain-abort-A");
        if cancels_transport {
            release.closed().await;
        } else {
            release.send(()).unwrap();
        }
        let output = stream.result().await;
        let message = output.read().unwrap();
        let reason = if mode == "replace-abort-B" {
            StopReason::Error
        } else {
            StopReason::Aborted
        };
        assert_eq!(message.stop_reason, reason, "{mode}");
        assert_eq!(
            request_signal
                .as_ref()
                .is_some_and(Cancellation::is_aborted),
            cancels_transport,
            "{mode}"
        );
        if cancels_transport {
            assert!(message.content.is_empty(), "{mode}");
        } else {
            assert_eq!(
                serde_json::to_value(&message.content)?[0]["text"],
                "released",
                "{mode}"
            );
        }
        assert_eq!(
            message.error_message.as_deref(),
            Some("Request was aborted"),
            "{mode}"
        );
        Ok(())
    }

    /// Compare every byte split and the unsplit body through public invocation.
    pub(super) async fn assert_error_splits(body: &str, has_prefix: bool) -> TestResult {
        for split in 0..body.len() {
            let chunks = if split == 0 {
                vec![body.as_bytes().to_vec()]
            } else {
                vec![
                    body.as_bytes()[..split].to_vec(),
                    body.as_bytes()[split..].to_vec(),
                ]
            };
            let stream = native_stream(StreamOptions {
                fetch: Some(body_chunks_fetch(chunks)),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            })?;
            let (events, _) = response_output::collect(&stream).await?;
            let output = stream.result().await;
            let message = output.read().unwrap();
            assert_eq!(message.stop_reason, StopReason::Error, "split {split}");
            assert_eq!(
                message.error_message.as_deref(),
                Some(
                    "got status: RESOURCE_EXHAUSTED. {\"error\":{\"code\":429,\"status\":\"RESOURCE_EXHAUSTED\",\"message\":\"busy\"}}"
                )
            );
            assert!(!events.iter().any(|event| event["type"] == "text_end"));
            if has_prefix {
                assert_eq!(
                    serde_json::to_value(&message.content)?[0]["text"],
                    "retained"
                );
            } else {
                assert!(message.content.is_empty());
            }
        }
        Ok(())
    }

    use super::*;
    /// Baseline public model and changed conversation.
    pub(crate) fn native_input() -> TestResult<(Model, Context)> {
        Ok((
            chat::model(
                &json!({"id":"gemini-2.5-flash", "api":"google-generative-ai", "provider":"google", "baseUrl":"https://fixture.invalid/v1beta"}),
            )?,
            chat::context(
                &json!({"messages":[{"role":"user", "content":"changed native input"}]}),
            )?,
        ))
    }
    /// Start with the supplied controlled transport and native settings.
    pub(crate) fn native_stream(
        common: StreamOptions,
    ) -> TestResult<maestro_models::AssistantMessageEventStream> {
        native_stream_for("google-generative-ai", common)
    }
    /// Drive either content entry with the same synchronized transport fixtures.
    pub(crate) fn native_stream_for(
        api: &str,
        common: StreamOptions,
    ) -> TestResult<maestro_models::AssistantMessageEventStream> {
        let (mut model, context) = native_input()?;
        model.api = api.into();
        if api == "google-vertex" {
            model.provider = api.into();
            Ok(maestro_models::stream_google_vertex(
                model,
                context,
                Some(maestro_models::GoogleVertexOptions {
                    common,
                    ..Default::default()
                }),
            ))
        } else {
            Ok(maestro_models::stream_google(
                model,
                context,
                Some(GoogleOptions {
                    common,
                    ..Default::default()
                }),
            ))
        }
    }
    /// Dropping observation does not cancel an admitted producer.
    pub(crate) async fn dropped_reader(api: &str) -> TestResult {
        let response = HttpResponse {body: Box::pin(futures_util::stream::iter([Ok(b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"released producer\"}]}}]}\n\n".to_vec())])), ..empty_response()};
        let (fetch, admission, release) = held_fetch(response);
        let stream = native_stream_for(
            api,
            StreamOptions {
                fetch: Some(fetch),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            },
        )?;
        let result = stream.result();
        admission.await?;
        drop(stream);
        release.send(()).unwrap();
        let output = result.await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
        assert_eq!(
            serde_json::to_value(&output.read().unwrap().content)?[0]["text"],
            "released producer"
        );
        Ok(())
    }
    /// An empty successful streamed body.
    pub(crate) fn empty_response() -> HttpResponse {
        HttpResponse {
            status: 200,
            status_text: String::new(),
            headers: std::collections::BTreeMap::default(),
            body: Box::pin(futures_util::stream::empty()),
        }
    }
    /// Assert a selected deadline spans the held body and leaves the caller active.
    pub(crate) async fn held_body_deadline(status: u16, api: &str) -> TestResult {
        let BodyGate {
            fetch,
            admission,
            read_started,
            release,
        } = body_gate(status, Vec::new(), Vec::new());
        let caller = Cancellation::new();
        let stream = native_stream_for(
            api,
            StreamOptions {
                fetch: Some(fetch),
                signal: Some(caller.clone()),
                api_key: Some("fixture-key".into()),
                on_payload: Some(Arc::new(|mut input, _| {
                    input.payload["config"]["httpOptions"] = json!({"timeout":10});
                    Box::pin(async { Ok(input) })
                })),
                ..Default::default()
            },
        )?;
        let request_signal = admission.await?.expect("owned signal");
        read_started.await?;
        let result = stream.result();
        let mut result = std::pin::pin!(result);
        tokio::time::advance(std::time::Duration::from_millis(9)).await;
        assert!(
            std::future::Future::poll(
                result.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop())
            )
            .is_pending()
        );
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        let std::task::Poll::Ready(output) = std::future::Future::poll(
            result.as_mut(),
            &mut std::task::Context::from_waker(std::task::Waker::noop()),
        ) else {
            panic!("request must settle at the selected deadline");
        };
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Error);
        assert!(!caller.is_aborted());
        assert!(request_signal.is_aborted());
        assert!(release.is_closed());
        let (records, _) = response_output::collect(&stream).await?;
        let kinds: Vec<_> = records
            .into_iter()
            .map(|event| event["type"].clone())
            .collect();
        assert_eq!(
            kinds,
            if status == 200 {
                vec![json!("start"), json!("error")]
            } else {
                vec![json!("error")]
            }
        );
        Ok(())
    }
    /// A controlled successful stream carrying the supplied response bytes.
    pub(crate) fn body_fetch(body: Vec<u8>) -> Fetch {
        body_chunks_fetch(vec![body])
    }

    /// A controlled successful stream preserving supplied chunk boundaries.
    fn body_chunks_fetch(chunks: Vec<Vec<u8>>) -> Fetch {
        Arc::new(move |_| {
            let chunks = chunks.clone();
            Box::pin(async move {
                Ok(HttpResponse {
                    body: Box::pin(futures_util::stream::iter(chunks.into_iter().map(Ok))),
                    ..empty_response()
                })
            })
        })
    }
    /// Check supplied identifiers and parse the generated timestamp/counter form.
    pub(crate) fn generated_ids(
        message: &maestro_models::AssistantMessage,
    ) -> TestResult<Vec<u64>> {
        let mut generated = Vec::new();
        for block in &message.content {
            let maestro_models::AssistantContent::ToolCall(call) = block else {
                return Err("tool call required".into());
            };
            if call.id.starts_with("lookup_") {
                let pieces: Vec<_> = call.id.split('_').collect();
                assert_eq!(pieces.len(), 3);
                assert!(pieces[1].parse::<u64>().is_ok());
                generated.push(pieces[2].parse::<u64>()?);
            } else {
                assert!(["provided", "duplicate"].contains(&call.id.as_str()));
            }
        }
        Ok(generated)
    }

    /// Witness body polling before holding the read until release or future drop.
    async fn read_held(
        reading: tokio::sync::oneshot::Sender<()>,
        held: tokio::sync::oneshot::Receiver<()>,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>, maestro_models::FetchError> {
        reading.send(()).unwrap();
        let _ = held.await;
        Ok(bytes)
    }
    /// Hold exactly one fetch after admission, retaining its request signal.
    pub(crate) fn held_fetch(
        response: HttpResponse,
    ) -> (
        Fetch,
        tokio::sync::oneshot::Receiver<Option<Cancellation>>,
        tokio::sync::oneshot::Sender<()>,
    ) {
        let (entered, admission) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let state = Arc::new(Mutex::new(Some((entered, held, response))));
        let fetch: Fetch = Arc::new(move |request| {
            let (entered, held, response) = state.lock().unwrap().take().unwrap();
            Box::pin(async move {
                assert!(entered.send(request.signal).is_ok());
                let _ = held.await;
                Ok(response)
            })
        });
        (fetch, admission, release)
    }

    /// Transport and the completion gates for a single held body read.
    struct BodyGate {
        /// Injected transport.
        fetch: Fetch,
        /// Request admission and owned signal.
        admission: tokio::sync::oneshot::Receiver<Option<Cancellation>>,
        /// The held body has been polled.
        read_started: tokio::sync::oneshot::Receiver<()>,
        /// Completes the same held read.
        release: tokio::sync::oneshot::Sender<()>,
    }

    /// Admit headers, deliver a prefix, then hold the remaining body until release.
    fn body_gate(status: u16, prefix: Vec<u8>, suffix: Vec<u8>) -> BodyGate {
        let (entered, admission) = tokio::sync::oneshot::channel();
        let (reading, read_started) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel();
        let state = Arc::new(Mutex::new(Some((entered, reading, held, prefix, suffix))));
        let fetch: Fetch = Arc::new(move |request| {
            let (entered, reading, held, prefix, suffix) = state.lock().unwrap().take().unwrap();
            assert!(entered.send(request.signal).is_ok());
            let body = futures_util::StreamExt::chain(
                futures_util::stream::iter([Ok(prefix)]),
                futures_util::stream::once(read_held(reading, held, suffix)),
            );
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    body: Box::pin(body),
                    ..empty_response()
                })
            })
        });
        BodyGate {
            fetch,
            admission,
            read_started,
            release,
        }
    }

    /// A common timeout never controls this provider's held attempt.
    pub(super) async fn common_timeout_ignored() -> TestResult {
        let (entered, admission) = tokio::sync::oneshot::channel();
        let (drive, tick) = tokio::sync::oneshot::channel();
        let (checked, checkpoint) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let state = Arc::new(Mutex::new(Some((entered, tick, checked, held))));
        let fetch: Fetch = Arc::new(move |request| {
            let (entered, tick, checked, held) = state.lock().unwrap().take().unwrap();
            Box::pin(async move {
                assert!(entered.send(request.signal).is_ok());
                tick.await.unwrap();
                checked.send(()).unwrap();
                held.await.unwrap();
                Ok(empty_response())
            })
        });
        let stream = native_stream(StreamOptions {
            fetch: Some(fetch),
            timeout_ms: Some(1.0),
            api_key: Some("fixture-key".into()),
            ..Default::default()
        })?;
        assert!(admission.await?.is_none());
        tokio::time::advance(std::time::Duration::from_millis(100)).await;
        drive.send(()).unwrap();
        checkpoint.await?;
        release.send(()).unwrap();
        assert_eq!(
            stream.result().await.read().unwrap().stop_reason,
            StopReason::Stop
        );
        Ok(())
    }

    /// Observe empty content at start and the same alias after gated reduction.
    pub(crate) async fn live_alias(api: &str) -> TestResult {
        let frame =
            b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"late content\"}]}}]}\n\n"
                .to_vec();
        let BodyGate {
            fetch,
            admission,
            read_started,
            release,
        } = body_gate(200, Vec::new(), frame);
        let stream = native_stream_for(
            api,
            StreamOptions {
                fetch: Some(fetch),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            },
        )?;
        admission.await?;
        read_started.await?;
        let Some(maestro_models::AssistantMessageEvent::Start { partial }) = stream.next().await
        else {
            return Err("start required".into());
        };
        assert!(partial.read().unwrap().content.is_empty());
        release.send(()).unwrap();
        response_output::collect(&stream).await?;
        let output = stream.result().await;
        assert!(Arc::ptr_eq(&partial, &output));
        assert_eq!(
            serde_json::to_value(&partial.read().unwrap().content)?[0]["text"],
            "late content"
        );
        Ok(())
    }

    /// Abort the held body after a real delta, retaining content without a closing update.
    pub(crate) async fn held_body_abort(api: &str) -> TestResult {
        let prefix =
            b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"held partial\"}]}}]}\n\n"
                .to_vec();
        let BodyGate {
            fetch,
            admission,
            read_started,
            release,
        } = body_gate(200, prefix, Vec::new());
        let caller = Cancellation::new();
        let stream = native_stream_for(
            api,
            StreamOptions {
                fetch: Some(fetch),
                signal: Some(caller.clone()),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            },
        )?;
        let request_signal = admission.await?.expect("owned signal");
        read_started.await?;
        caller.abort();
        let output = stream.result().await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Aborted);
        assert!(request_signal.is_aborted());
        assert!(release.is_closed());
        let (events, message) = response_output::collect(&stream).await?;
        assert_eq!(message["content"][0]["text"], "held partial");
        assert_eq!(
            events
                .iter()
                .map(|event| &event["type"])
                .collect::<Vec<_>>(),
            vec![
                &json!("start"),
                &json!("text_start"),
                &json!("text_delta"),
                &json!("error")
            ]
        );
        Ok(())
    }
    /// Invoke one of the six public routes with the same observable request identity.
    pub(super) async fn route_output(
        route: usize,
        model: Model,
        context: Context,
        common: StreamOptions,
    ) -> TestResult<maestro_models::SharedAssistantMessage> {
        let raw = maestro_models::ProviderStreamOptions {
            common: common.clone(),
            ..Default::default()
        };
        let simple = maestro_models::SimpleStreamOptions {
            common: common.clone(),
            ..Default::default()
        };
        Ok(match route {
            0 => {
                maestro_models::stream_google(
                    model,
                    context,
                    Some(GoogleOptions {
                        common,
                        ..Default::default()
                    }),
                )
                .result()
                .await
            }
            1 => {
                maestro_models::stream_simple_google(model, context, Some(simple))
                    .result()
                    .await
            }
            2 => {
                maestro_models::stream(model, context, Some(raw))?
                    .result()
                    .await
            }
            3 => {
                maestro_models::stream_simple(model, context, Some(simple))?
                    .result()
                    .await
            }
            4 => maestro_models::complete(model, context, Some(raw)).await?,
            _ => maestro_models::complete_simple(model, context, Some(simple)).await?,
        })
    }

    /// Wrong shapes covering each declared raw field boundary.
    pub(super) fn malformed_raw_extras() -> [serde_json::Value; 13] {
        [
            json!({"toolChoice":false}),
            json!({"toolChoice":[]}),
            json!({"thinking":[]}),
            json!({"thinking":"yes"}),
            json!({"thinking":{}}),
            json!({"thinking":{"enabled":null}}),
            json!({"thinking":{"enabled":"true"}}),
            json!({"thinking":{"enabled":true,"budgetTokens":null}}),
            json!({"thinking":{"enabled":true,"budgetTokens":"4"}}),
            json!({"thinking":{"enabled":true,"level":null}}),
            json!({"thinking":{"enabled":true,"level":4}}),
            json!({"thinking":{"enabled":true,"level":{"HIGH":null}}}),
            json!({"thinking":{"enabled":true,"level":"UNKNOWN"}}),
        ]
    }
}

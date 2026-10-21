//! Authenticated downloaded-model discovery through replaceable socket effects.
#![cfg(not(target_arch = "wasm32"))]
use crate::lm_studio_support::{
    case, connection_failures, empty, failure_phase, group, native, options, replay, schema_records,
};
use crate::support::run_io as run;

#[test]
fn downloaded_models_keep_paths_order_and_fields() {
    run(async {
        replay("ordered_fields_and_duplicates").await;
    });
}

use maestro_app::browser::utils::model_discovery::{
    DiscoveryErrorKind, DiscoveryProvider, SocketPhase, discover_lm_studio_models, discover_models,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[test]
fn display_names_keep_whitespace_and_empty_fallback() {
    run(async {
        replay("wide_integer_and_empty_name").await;
        replay("whitespace_name_and_zero_context").await;
    });
}
#[test]
fn language_flags_are_selected_independently() {
    run(async {
        replay("independent_vision_and_tool_flags").await;
        schema_records(|input| {
            input.get("vision") == Some(&Value::Bool(false))
                || input.get("trainedForToolUse") == Some(&Value::Bool(false))
        })
        .await;
    });
}
#[test]
fn context_limits_keep_zero_negative_and_wide_integers() {
    run(async {
        replay("signed_zero_from_raw_wire").await;
        schema_records(|input| {
            input
                .get("maxContextLength")
                .and_then(Value::as_f64)
                .is_some_and(|value| value.to_bits() != 8192.0_f64.to_bits())
        })
        .await;
    });
}

/// Recorded destination selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UrlCase {
    /// Authored base.
    input: String,
    /// Expected normalized system endpoint, absent for invalid input.
    url: Option<String>,
}
#[test]
fn lm_studio_url_selection_matches_host_and_port_rules() {
    run(async {
        for case in group::<Vec<UrlCase>>("urls") {
            let (options, trace) = options(empty());
            let result = discover_lm_studio_models(&case.input, None, &options).await;
            if let Some(url) = case.url {
                assert!(result.unwrap().is_empty());
                assert_eq!(trace.lock().unwrap().request.as_ref().unwrap().url, url);
            } else {
                assert!(matches!(
                    result.unwrap_err().kind,
                    DiscoveryErrorKind::InvalidUrl
                ));
                assert!(trace.lock().unwrap().request.is_none());
            }
        }
        replay("endpoint_ignores_credentials_path_query_fragment").await;
    });
}
#[test]
fn discovery_dispatches_lm_studio_without_http() {
    run(async {
        let (options, _) = options(empty());
        assert_eq!(DiscoveryProvider::Lmstudio.to_string(), "LM Studio");
        assert!(
            discover_models(DiscoveryProvider::Lmstudio, "http://host", None, &options)
                .await
                .unwrap()
                .is_empty()
        );
    });
}
#[test]
fn system_rpc_authenticates_before_listing() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    run(async {
        let (options, trace) = options(empty());
        let (release, authentication) = tokio::sync::oneshot::channel();
        trace.lock().unwrap().authentication = Some(authentication);
        let mut discovery = Box::pin(discover_lm_studio_models(
            "https://user:password@HOST:1235/path?q#f",
            None,
            &options,
        ));
        assert!(
            std::future::Future::poll(
                discovery.as_mut(),
                &mut std::task::Context::from_waker(std::task::Waker::noop()),
            )
            .is_pending()
        );
        {
            let trace = trace.lock().unwrap();
            assert_eq!(trace.sent.len(), 1);
            assert_eq!(trace.phases, ["send"]);
        }
        release.send(()).unwrap();
        discovery.await.unwrap();
        let trace = trace.lock().unwrap();
        assert_eq!(trace.phases, ["send", "receive", "send", "receive"]);
        let request = trace.request.as_ref().unwrap();
        assert_eq!(request.url, "ws://host:1235/system");
        assert!(request.headers.is_empty());
        assert_eq!(trace.sent.len(), 2);
        let auth: Value = serde_json::from_slice(&trace.sent[0]).unwrap();
        assert_eq!(auth.as_object().unwrap().len(), 3);
        assert_eq!(auth["authVersion"], 1);
        for key in ["clientIdentifier", "clientPasskey"] {
            let credential = auth[key].as_str().unwrap();
            assert_eq!(credential.len(), 24);
            assert_eq!(STANDARD.decode(credential).unwrap().len(), 18);
        }
        let independent = auth["clientIdentifier"] != auth["clientPasskey"];
        assert!(independent);
        let rpc: Value = serde_json::from_slice(&trace.sent[1]).unwrap();
        assert_eq!(
            rpc,
            json!({"type":"rpcCall","endpoint":"listDownloadedModels","callId":0})
        );
    });
}
#[test]
fn authentication_refusal_never_sends_rpc() {
    run(async {
        let trace = replay("authentication_denied").await;
        let trace = trace.lock().unwrap();
        assert_eq!(trace.sent.len(), 1);
        assert_eq!(trace.phases, ["send", "receive"]);
    });
}
#[test]
fn native_discovery_uses_real_authenticated_system_socket() {
    run(async {
        native(false).await;
    });
}
#[test]
fn discovery_preserves_url_connection_auth_rpc_and_decode_failures() {
    run(async {
        for base in group::<Vec<String>>("invalidUrls") {
            let (options, trace) = options(empty());
            let error = discover_lm_studio_models(&base, None, &options)
                .await
                .unwrap_err();
            assert!(matches!(error.kind, DiscoveryErrorKind::InvalidUrl));
            assert_eq!(error.provider, DiscoveryProvider::Lmstudio);
            assert!(
                error
                    .to_string()
                    .starts_with("LM Studio discovery failed: ")
            );
            assert!(trace.lock().unwrap().request.is_none());
        }
        for name in [
            "authentication_denied",
            "authentication_malformed_json",
            "authentication_malformed_shape",
            "closed_during_authentication",
            "closed_during_rpc",
        ] {
            replay(name).await;
        }
        for phase in [
            SocketPhase::Connection,
            SocketPhase::Authentication,
            SocketPhase::Rpc,
        ] {
            failure_phase(phase).await;
        }
        connection_failures().await;
    });
}
#[test]
fn malformed_messages_fail_instead_of_waiting() {
    run(async {
        for name in [
            "invalid_json_then_success",
            "invalid_envelope_then_success",
            "invalid_integer_call_id_then_success",
            "invalid_result_then_success",
            "invalid_embedding_then_success",
        ] {
            let trace = replay(name).await;
            assert_eq!(trace.lock().unwrap().reads, 2);
        }
    });
}
#[test]
fn unmatched_and_control_messages_leave_current_rpc_pending() {
    run(async {
        for name in [
            "valid_control_messages_then_success",
            "valid_unowned_channel_and_signal_messages",
            "unmatched_result_then_success",
        ] {
            let trace = replay(name).await;
            assert_eq!(trace.lock().unwrap().reads, case(name).replies.len());
        }
    });
}
#[test]
fn unowned_messages_are_consumed_through_nonempty_correlated_result() {
    run(async {
        for name in [
            "valid_control_messages_then_success",
            "valid_unowned_channel_and_signal_messages",
            "unmatched_result_then_success",
        ] {
            let mut selected = case(name);
            let mut listing = case("ordered_fields_and_duplicates");
            *selected.replies.last_mut().unwrap() = listing.replies.pop().unwrap();
            let reads = selected.replies.len();
            let (options, trace) = options(selected.replies);
            let base = listing.base.replace("$PORT", "12345");
            let mut expected = listing.expected.models.unwrap();
            for model in &mut expected {
                model.base_url = model.base_url.replace("$PORT", "12345");
            }
            let actual = discover_lm_studio_models(&base, None, &options)
                .await
                .unwrap();
            assert_eq!(actual, expected, "operation {name}");
            assert_eq!(trace.lock().unwrap().reads, reads, "operation {name}");
        }
    });
}
#[test]
fn success_and_failure_release_owned_socket() {
    run(async {
        for (index, replies) in [
            empty(),
            vec![json!({"success":false,"error":"denied"})],
            vec![
                json!({"success":true}),
                json!({"type":"rpcResult","callId":0,"result":null}),
            ],
            vec![],
            vec![json!({"success":true})],
            vec![
                json!({"success":true}),
                json!({"type":"rpcError","callId":0,"error":{}}),
            ],
        ]
        .into_iter()
        .enumerate()
        {
            let (options, trace) = options(replies);
            let result = discover_lm_studio_models("http://host", None, &options).await;
            assert_eq!(result.is_ok(), index == 0);
            let trace = trace.lock().unwrap();
            assert_eq!(trace.closes, 1);
            assert_eq!(trace.drops, 1);
        }
    });
}
#[test]
fn dropping_discovery_releases_inflight_socket_and_listeners() {
    run(async {
        native(true).await;
    });
}
#[test]
fn api_key_is_not_used_for_system_authentication() {
    run(async {
        replay("empty_api_key").await;
        for key in [None, Some(""), Some("caller-supplied")] {
            let (options, trace) = options(empty());
            discover_lm_studio_models("http://host", key, &options)
                .await
                .unwrap();
            let trace = trace.lock().unwrap();
            assert!(trace.request.as_ref().unwrap().headers.is_empty());
            let auth: Value = serde_json::from_slice(&trace.sent[0]).unwrap();
            assert_eq!(auth.as_object().unwrap().len(), 3);
            assert!(auth["clientIdentifier"].as_str().unwrap() != "caller-supplied");
            assert!(auth["clientPasskey"].as_str().unwrap() != "caller-supplied");
            assert_eq!(
                serde_json::from_slice::<Value>(&trace.sent[1]).unwrap(),
                json!({"type":"rpcCall","endpoint":"listDownloadedModels","callId":0})
            );
        }
    });
}
#[test]
fn selected_duplicate_json_members_keep_last_values() {
    run(async {
        replay("selected_duplicate_wire_members").await;
    });
}
#[test]
fn unknown_model_fields_do_not_reduce_server_support() {
    run(async {
        schema_records(|input| input.get("format").is_some()).await;
    });
}
#[test]
fn invalid_unused_optional_metadata_invalidates_listing() {
    run(async {
        schema_records(|input| {
            input.get("quantization").is_some()
                || input.get("architecture").is_some()
                || input.get("paramsString").is_some()
        })
        .await;
    });
}
#[test]
fn failure_is_returned_once_without_extra_diagnostics() {
    run(async {
        let (mut options, _) = options(vec![json!({"success":false,"error":"denied"})]);
        let diagnostics = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&diagnostics);
        options.log = Arc::new(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        assert!(
            discover_lm_studio_models("http://host", None, &options)
                .await
                .is_err()
        );
        assert_eq!(diagnostics.load(std::sync::atomic::Ordering::SeqCst), 0);
    });
}

/// Remote error-root admission observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorCase {
    input: Value,
    success: bool,
}
#[test]
fn rpc_failure_keeps_remote_fields_and_native_display() {
    run(async {
        replay("remote_rpc_error").await;
        for case in group::<Vec<ErrorCase>>("remoteErrors") {
            let (options, _) = options(vec![
                json!({"success":true}),
                json!({"type":"rpcError","callId":0,"error":case.input}),
            ]);
            let error = discover_lm_studio_models("http://host", None, &options)
                .await
                .unwrap_err();
            if !case.success {
                assert!(matches!(error.kind, DiscoveryErrorKind::InvalidResponse));
                continue;
            }
            let DiscoveryErrorKind::Rpc(raw) = error.kind else {
                panic!("expected remote error");
            };
            assert_eq!(
                serde_json::from_str::<Value>(raw.get()).unwrap(),
                case.input
            );
            assert_eq!(
                error.message,
                case.input
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown error")
            );
        }
    });
}

//! Cloud content-generation entry behavior.
#[cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/cloud_content_rows.rs"]
mod rows;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_real_keys_bypass_native_credentials() -> TestResult {
    rows::assert_rows("cloud_real_keys_bypass_native_credentials")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_markers_select_native_credentials() -> TestResult {
    rows::assert_rows("cloud_markers_select_native_credentials")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_key_precedence_trims_only_the_shared_whitespace_set() -> TestResult {
    rows::assert_rows("cloud_key_precedence_trims_only_the_shared_whitespace_set")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_environment_markers_select_native_credentials() -> TestResult {
    rows::assert_rows("cloud_environment_markers_select_native_credentials")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_project_and_location_follow_separate_fallback_chains() -> TestResult {
    rows::assert_rows("cloud_project_and_location_follow_separate_fallback_chains")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_default_endpoints_preserve_location_families() -> TestResult {
    rows::assert_rows("cloud_default_endpoints_preserve_location_families")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_missing_scope_fails_before_payload_hooks() -> TestResult {
    rows::assert_rows("cloud_missing_scope_fails_before_payload_hooks")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_generated_endpoints_are_suppressed() -> TestResult {
    rows::assert_rows("cloud_generated_endpoints_are_suppressed")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_custom_endpoints_keep_native_auth() -> TestResult {
    rows::assert_rows("cloud_custom_endpoints_keep_native_auth")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_custom_endpoints_keep_key_auth() -> TestResult {
    rows::assert_rows("cloud_custom_endpoints_keep_key_auth")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_invalid_custom_endpoints_keep_url_error_phase() -> TestResult {
    rows::assert_rows("cloud_invalid_custom_endpoints_keep_url_error_phase")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_custom_versions_are_not_duplicated() -> TestResult {
    rows::assert_rows("cloud_custom_versions_are_not_duplicated")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_resource_paths_preserve_scope_and_qualified_models() -> TestResult {
    rows::assert_rows("cloud_resource_paths_preserve_scope_and_qualified_models")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_request_http_scope_overrides_constructor_scope() -> TestResult {
    rows::assert_rows("cloud_request_http_scope_overrides_constructor_scope")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_entry_honors_ambient_cloud_flags() -> TestResult {
    rows::assert_rows("cloud_public_entry_honors_ambient_cloud_flags")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_enterprise_flag_takes_precedence_over_vertex_flag() -> TestResult {
    rows::assert_rows("cloud_enterprise_flag_takes_precedence_over_vertex_flag")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_environment_endpoint_keeps_default_version() -> TestResult {
    rows::assert_rows("cloud_public_environment_endpoint_keeps_default_version")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_ambient_cloud_keeps_explicit_version_suppression() -> TestResult {
    rows::assert_rows("cloud_public_ambient_cloud_keeps_explicit_version_suppression")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_ambient_empty_key_is_retained() -> TestResult {
    rows::assert_rows("cloud_public_ambient_empty_key_is_retained")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_ambient_missing_auth_fails_during_construction() -> TestResult {
    rows::assert_rows("cloud_public_ambient_missing_auth_fails_during_construction")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_ambient_simple_entry_keeps_missing_key_failure() -> TestResult {
    rows::assert_rows("cloud_public_ambient_simple_entry_keeps_missing_key_failure")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_raw_empty_key_does_not_select_the_sdk_environment_key() -> TestResult {
    rows::assert_rows("cloud_public_raw_empty_key_does_not_select_the_sdk_environment_key")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_public_ambient_native_auth_reuses_the_credential_owner() -> TestResult {
    rows::assert_rows("cloud_public_ambient_native_auth_reuses_the_credential_owner")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_constructor_settings_are_captured_before_payload_hooks() -> TestResult {
    rows::assert_rows("cloud_constructor_settings_are_captured_before_payload_hooks")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_payload_hook_edits_and_live_signals_reach_the_shared_consumer() -> TestResult {
    rows::assert_rows("cloud_payload_hook_edits_and_live_signals_reach_the_shared_consumer")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_failures_preserve_start_order_and_partial_content() -> TestResult {
    rows::assert_rows("cloud_failures_preserve_start_order_and_partial_content")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_simple_disabled_thinking_reaches_the_wire() -> TestResult {
    rows::assert_rows("cloud_simple_disabled_thinking_reaches_the_wire")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_simple_thinking_keeps_its_own_family_policy() -> TestResult {
    rows::assert_rows("cloud_simple_thinking_keeps_its_own_family_policy")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_simple_custom_budgets_preserve_zero_and_negative_one() -> TestResult {
    rows::assert_rows("cloud_simple_custom_budgets_preserve_zero_and_negative_one")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_simple_scope_extras_are_not_forwarded() -> TestResult {
    rows::assert_rows("cloud_simple_scope_extras_are_not_forwarded")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_raw_thinking_preserves_presence_and_level_priority() -> TestResult {
    rows::assert_rows("cloud_raw_thinking_preserves_presence_and_level_priority")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_raw_tool_choice_is_emitted_only_with_tools() -> TestResult {
    rows::assert_rows("cloud_raw_tool_choice_is_emitted_only_with_tools")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_preaborted_requests_stop_before_hooks_and_transport() -> TestResult {
    rows::assert_rows("cloud_preaborted_requests_stop_before_hooks_and_transport")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_native_discovery_still_occurs_after_payload_hooks() -> TestResult {
    rows::assert_rows("cloud_native_discovery_still_occurs_after_payload_hooks")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_headers_keep_layer_precedence_without_response_hooks() -> TestResult {
    rows::assert_rows("cloud_headers_keep_layer_precedence_without_response_hooks")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_owned_producer_keeps_lifetime_and_deadline_policy() -> TestResult {
    use crate::content_generation_invocation::native;
    crate::runtime::run_paused(async {
        native::dropped_reader("google-vertex").await?;
        native::held_body_abort("google-vertex").await?;
        native::held_body_deadline(200, "google-vertex").await?;
        native::held_body_deadline(500, "google-vertex").await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_local_and_threaded_registration_keep_hook_captures() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "cloud_local_and_threaded_registration_keep_hook_captures",
            )?
            .nocapture()
            .env_clear()
            .case("cloud")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::provider_affinity::cloud_affinity()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_raw_entry_uses_shared_response_reduction() -> TestResult {
    crate::content_generation_reduction::cloud_entry_rows()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_call_counters_are_independent_between_entries() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "cloud_call_counters_are_independent_between_entries",
            )?
            .nocapture()
            .env_clear()
            .case("counters")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::runtime::run(async {
        use crate::content_generation_invocation::native;
        use maestro_models::{StopReason, StreamOptions};
        use serde_json::json;
        let mut ordinals = Vec::new();
        for api in [
            "google-generative-ai",
            "google-vertex",
            "google-generative-ai",
        ] {
            let body = b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"functionCall\":{\"name\":\"lookup\",\"args\":{}}}]},\"finishReason\":\"STOP\"}]}\n\n";
            let stream = native::native_stream_for(
                api,
                StreamOptions {
                    api_key: Some("fixture-key".into()),
                    fetch: Some(native::body_fetch(body.to_vec())),
                    ..Default::default()
                },
            )?;
            let output = stream.result().await;
            let message = output.read().unwrap();
            assert_eq!(message.stop_reason, StopReason::ToolUse);
            ordinals.extend(native::generated_ids(&message)?);
        }
        assert_eq!(ordinals, [1, 1, 2]);
        let body = format!(
            "data: {}\n\n",
            json!({"candidates":[{"content":{"parts":[{"functionCall":{"name":"lookup","id":"provided","args":{}}},{"functionCall":{"name":"lookup","id":"duplicate","args":{}}},{"functionCall":{"name":"lookup","id":"duplicate","args":{}}}]},"finishReason":"STOP"}]})
        );
        let stream = native::native_stream_for(
            "google-vertex",
            StreamOptions {
                api_key: Some("fixture-key".into()),
                fetch: Some(native::body_fetch(body.into_bytes())),
                ..Default::default()
            },
        )?;
        let output = stream.result().await;
        assert_eq!(native::generated_ids(&output.read().unwrap())?, [2]);
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/cloud_content_routes.rs"]
mod routes;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_root_and_registry_entries_dispatch() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "cloud_root_and_registry_entries_dispatch",
            )?
            .nocapture()
            .env_clear()
            .case("roots")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::runtime::run(routes::root_routes())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_raw_options_preserve_declared_fields() -> TestResult {
    if !maestro_test_process::selection().is_child() {
        return {
            let output = maestro_test_process::TestProcess::current(
                "cloud_raw_options_preserve_declared_fields",
            )?
            .nocapture()
            .env_clear()
            .case("options")
            .output()?;
            assert!(output.status.success(), "{output:?}");
            Ok(())
        };
    }
    crate::runtime::run(routes::raw_options())
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/cloud_content_deadline.rs"]
mod deadline;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cloud_authentication_consumes_the_selected_deadline() -> TestResult {
    let case = maestro_test_process::selection();
    if case.is_child() {
        return crate::runtime::run(deadline::delayed_refresh(case.runs_case("expired")));
    }
    for case in ["expired", "active"] {
        let files = crate::content_credentials::Files::new();
        let path = files.path("adc.json");
        {
            let output = maestro_test_process::TestProcess::current(
                "cloud_authentication_consumes_the_selected_deadline",
            )?
            .nocapture()
            .env_clear()
            .envs(
                [
                    ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
                    ("NO_PROXY", "127.0.0.1,localhost"),
                ]
                .iter()
                .copied(),
            )
            .case(case)
            .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}

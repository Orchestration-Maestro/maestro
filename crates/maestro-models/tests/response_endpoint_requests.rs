//! Request behavior of the standard response endpoint.
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/response_endpoint.rs"]
mod endpoint;
#[path = "support/json.rs"]
mod json;
#[path = "support/response_cases.rs"]
mod response_cases;

#[test]
fn responses_build_default_payload() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_build_default_payload"),
    )
}

#[test]
fn responses_resolve_retention_before_affinity() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_resolve_retention_before_affinity"),
    )
}

#[test]
fn responses_keep_request_id_when_session_header_disabled() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_keep_request_id_when_session_header_disabled"),
    )
}

#[test]
fn responses_apply_header_layers_case_insensitively() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_apply_header_layers_case_insensitively"),
    )
}

#[test]
fn responses_trim_scope_with_script_whitespace() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_trim_scope_with_script_whitespace"),
    )
}

#[test]
fn responses_distinguish_raw_and_simple_missing_keys() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_distinguish_raw_and_simple_missing_keys"),
    )
}

#[test]
fn responses_resolve_gateway_url_and_authorization() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_resolve_gateway_url_and_authorization"),
    )
}

#[test]
fn responses_derive_account_headers_from_history() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_derive_account_headers_from_history"),
    )
}

#[test]
fn responses_select_effort_summary_and_encrypted_replay() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_select_effort_summary_and_encrypted_replay"),
    )
}

#[test]
fn responses_preserve_numeric_option_omissions() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_preserve_numeric_option_omissions"),
    )
}

#[test]
fn responses_preserve_service_tier_null_and_values() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_preserve_service_tier_null_and_values"),
    )
}

#[test]
fn responses_keep_raw_options_distinct_from_simple() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_keep_raw_options_distinct_from_simple"),
    )
}

#[test]
fn responses_default_simple_budget_from_model() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_default_simple_budget_from_model"),
    )
}

#[test]
fn responses_clamp_simple_reasoning_to_supported_levels() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_clamp_simple_reasoning_to_supported_levels"),
    )
}

#[test]
fn responses_omit_empty_tools_and_use_shared_conversion() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_omit_empty_tools_and_use_shared_conversion"),
    )
}

#[test]
fn responses_replay_history_through_endpoint_conversion() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_replay_history_through_endpoint_conversion"),
    )
}

#[test]
fn responses_keep_body_content_type_and_gateway_token_precedence() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_keep_body_content_type_and_gateway_token_precedence"),
    )
}

#[test]
fn responses_ignore_unsupported_common_options() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("responses_ignore_unsupported_common_options"),
    )
}

#[test]
fn responses_summary_only_uses_literal_medium() -> chat::TestResult {
    chat::block_on(false, async {
        for mapped in ["low", ""] {
            let actual = endpoint::run_case(&serde_json::json!({
                "model":{"reasoning":true,"thinkingLevelMap":{"medium":mapped}},
                "options":{"reasoningSummary":"detailed"}
            }))
            .await?;
            assert_eq!(
                actual["requests"][0]["body"]["reasoning"]["effort"],
                "medium"
            );
        }
        Ok(())
    })
}

#[test]
fn responses_preserve_payload_insertion_order_at_hook_and_wire() -> chat::TestResult {
    chat::block_on(false, async {
        let actual = endpoint::run_case(&serde_json::json!({
            "hook":"order","model":{"reasoning":true},
            "context":{"tools":[{"name":"lookup","description":"Find","parameters":{"type":"object"}}]},
            "options":{"sessionId":"session","cacheRetention":"long","maxTokens":7,"temperature":0.5,"serviceTier":"flex","reasoningEffort":"low"}
        })).await?;
        let expected = serde_json::json!([
            "model",
            "input",
            "stream",
            "prompt_cache_key",
            "prompt_cache_retention",
            "store",
            "max_output_tokens",
            "temperature",
            "service_tier",
            "tools",
            "reasoning",
            "include"
        ]);
        assert_eq!(actual["hooks"][0], expected);
        assert_eq!(actual["requests"][0]["bodyKeys"], expected);
        let wire = actual["requests"][0]["wire"].as_str().ok_or("wire")?;
        let mut previous = 0;
        for key in expected.as_array().ok_or("keys")? {
            let position = wire
                .find(&format!("\"{}\":", key.as_str().ok_or("key")?))
                .ok_or("serialized key")?;
            assert!(position >= previous, "wire key order: {wire}");
            previous = position;
        }
        Ok(())
    })
}

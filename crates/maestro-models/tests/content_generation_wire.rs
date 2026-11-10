//! Wire projection of the content-generation request body through its public operation.
use crate::chat;
use crate::content_wire;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};

use chat::TestResult;
use content_wire::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use content_wire::rows;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::OnPayload;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::content::generative::{GoogleOptions, GoogleThinking, build_params};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_replacement_changes_model_content_and_configuration() -> TestResult {
    let model = Arc::new(chat::model(
        &json!({"api": "google-generative-ai", "provider": "google", "id": "before-hook"}),
    )?);
    let context = chat::context(&json!({
        "systemPrompt": "before system",
        "messages": [{"role": "user", "content": "before turn"}]
    }))?;
    let options = GoogleOptions {
        thinking: Some(GoogleThinking {
            enabled: true,
            budget_tokens: Some(128.0),
            level: None,
        }),
        ..GoogleOptions::default()
    };
    let prepared = serde_json::to_value(build_params(&model, &context, Some(&options))?)?;
    let rows = rows("hook_replacement_changes_model_content_and_configuration")?;
    let row = rows.first().ok_or("recorded hook row")?;
    let replacement: Value = serde_json::from_str(&row.input)?;
    let received = Arc::new(Mutex::new(None));
    let hook: OnPayload = {
        let received = Arc::clone(&received);
        Arc::new(
            move |maestro_models::PayloadHookInput {
                      payload,
                      signal: hook_signal,
                  },
                  _| {
                if let Ok(mut slot) = received.lock() {
                    *slot = Some(payload);
                }
                Box::pin(std::future::ready(Ok(maestro_models::PayloadHookInput {
                    payload: replacement.clone(),
                    signal: hook_signal,
                })))
            },
        )
    };
    let returned = crate::runtime::run::<crate::TestResult<_>>(async {
        Ok(hook(
            maestro_models::PayloadHookInput {
                payload: prepared.clone(),
                signal: None,
            },
            model,
        )
        .await?
        .payload)
    })?;
    let seen = received.lock().map_err(|_| "hook payload lock")?.take();
    assert_eq!(seen, Some(prepared.clone()));
    assert_ne!(prepared["model"], returned["model"]);
    assert_ne!(prepared["contents"], returned["contents"]);
    row.check_value(returned)
}

#[test]
fn configuration_presence_controls_the_generation_object() -> TestResult {
    assert_rows("configuration_presence_controls_the_generation_object")
}

#[test]
fn unknown_and_client_only_fields_do_not_enter_the_body() -> TestResult {
    assert_rows("unknown_and_client_only_fields_do_not_enter_the_body")
}

#[test]
fn model_paths_preserve_qualifiers_and_authored_text() -> TestResult {
    assert_rows("model_paths_preserve_qualifiers_and_authored_text")
}

#[test]
fn model_rejections_precede_contents_and_configuration() -> TestResult {
    assert_rows("model_rejections_precede_contents_and_configuration")
}

#[test]
fn missing_or_null_contents_are_not_invented() -> TestResult {
    assert_rows("missing_or_null_contents_are_not_invented")
}

#[test]
fn content_unions_keep_turns_parts_and_duplicates() -> TestResult {
    assert_rows("content_unions_keep_turns_parts_and_duplicates")
}

#[test]
fn contents_reject_empty_mixed_and_unwrapped_call_forms() -> TestResult {
    assert_rows("contents_reject_empty_mixed_and_unwrapped_call_forms")
}

#[test]
fn system_instructions_use_the_single_content_union() -> TestResult {
    assert_rows("system_instructions_use_the_single_content_union")
}

#[test]
fn empty_system_part_lists_fail_before_sampling() -> TestResult {
    assert_rows("empty_system_part_lists_fail_before_sampling")
}

#[test]
fn part_fields_keep_open_values_and_skip_null() -> TestResult {
    assert_rows("part_fields_keep_open_values_and_skip_null")
}

#[test]
fn combined_part_fields_are_not_exclusive_variants() -> TestResult {
    assert_rows("combined_part_fields_are_not_exclusive_variants")
}

#[test]
fn nested_part_projection_selects_only_owned_members() -> TestResult {
    assert_rows("nested_part_projection_selects_only_owned_members")
}

#[test]
fn unsupported_part_members_fail_even_when_null() -> TestResult {
    assert_rows("unsupported_part_members_fail_even_when_null")
}

#[test]
fn configuration_fields_keep_their_destination_and_zero_values() -> TestResult {
    assert_rows("configuration_fields_keep_their_destination_and_zero_values")
}

#[test]
fn configuration_nulls_are_omitted_without_defaults() -> TestResult {
    assert_rows("configuration_nulls_are_omitted_without_defaults")
}

#[test]
fn configuration_fields_are_projected_in_source_order() -> TestResult {
    assert_rows("configuration_fields_are_projected_in_source_order")
}

#[test]
fn passthrough_fields_do_not_gain_typed_ingress_validation() -> TestResult {
    assert_rows("passthrough_fields_do_not_gain_typed_ingress_validation")
}

#[test]
fn unsupported_configuration_members_fail_on_presence() -> TestResult {
    assert_rows("unsupported_configuration_members_fail_on_presence")
}

#[test]
fn speech_configuration_accepts_names_and_open_records() -> TestResult {
    assert_rows("speech_configuration_accepts_names_and_open_records")
}

#[test]
fn cached_content_names_add_only_a_missing_single_segment_prefix() -> TestResult {
    assert_rows("cached_content_names_add_only_a_missing_single_segment_prefix")
}

#[test]
fn image_configuration_selects_supported_fields() -> TestResult {
    assert_rows("image_configuration_selects_supported_fields")
}

#[test]
fn unsupported_image_configuration_fails_on_presence() -> TestResult {
    assert_rows("unsupported_image_configuration_fails_on_presence")
}

#[test]
fn safety_settings_keep_order_and_select_fields() -> TestResult {
    assert_rows("safety_settings_keep_order_and_select_fields")
}

#[test]
fn safety_method_is_rejected_before_threshold() -> TestResult {
    assert_rows("safety_method_is_rejected_before_threshold")
}

#[test]
fn tools_presence_preserves_an_explicit_empty_list() -> TestResult {
    assert_rows("tools_presence_preserves_an_explicit_empty_list")
}

#[test]
fn tool_fields_preserve_open_values_and_skip_null() -> TestResult {
    assert_rows("tool_fields_preserve_open_values_and_skip_null")
}

#[test]
fn tool_kinds_and_declarations_can_coexist() -> TestResult {
    assert_rows("tool_kinds_and_declarations_can_coexist")
}

#[test]
fn unsupported_tool_members_fail_on_presence() -> TestResult {
    assert_rows("unsupported_tool_members_fail_on_presence")
}

#[test]
fn search_tool_selects_only_supported_configuration() -> TestResult {
    assert_rows("search_tool_selects_only_supported_configuration")
}

#[test]
fn unsupported_search_fields_fail_on_presence() -> TestResult {
    assert_rows("unsupported_search_fields_fail_on_presence")
}

#[test]
fn maps_tool_selects_api_key_and_widget_fields() -> TestResult {
    assert_rows("maps_tool_selects_api_key_and_widget_fields")
}

#[test]
fn unsupported_maps_auth_fields_fail_on_presence() -> TestResult {
    assert_rows("unsupported_maps_auth_fields_fail_on_presence")
}

#[test]
fn tool_configuration_selects_fields_without_declarations() -> TestResult {
    assert_rows("tool_configuration_selects_fields_without_declarations")
}

#[test]
fn streamed_function_arguments_are_rejected_on_presence() -> TestResult {
    assert_rows("streamed_function_arguments_are_rejected_on_presence")
}

#[test]
fn schema_types_use_the_declared_uppercase_vocabulary() -> TestResult {
    assert_rows("schema_types_use_the_declared_uppercase_vocabulary")
}

#[test]
fn schema_type_arrays_keep_order_duplicates_and_nullable() -> TestResult {
    assert_rows("schema_type_arrays_keep_order_duplicates_and_nullable")
}

#[test]
fn schema_rejects_exclusive_type_conflicts() -> TestResult {
    assert_rows("schema_rejects_exclusive_type_conflicts")
}

#[test]
fn schema_nullable_wrappers_and_general_unions_keep_their_rules() -> TestResult {
    assert_rows("schema_nullable_wrappers_and_general_unions_keep_their_rules")
}

#[test]
fn schema_traversal_changes_only_owned_schema_positions() -> TestResult {
    assert_rows("schema_traversal_changes_only_owned_schema_positions")
}

#[test]
fn nullable_schema_siblings_survive_conversion() -> TestResult {
    assert_rows("nullable_schema_siblings_survive_conversion")
}

#[test]
fn nullable_schema_correction_is_shared_by_response_and_function_schemas() -> TestResult {
    assert_rows("nullable_schema_correction_is_shared_by_response_and_function_schemas")
}

#[test]
fn response_json_schema_routing_respects_the_existing_destination() -> TestResult {
    assert_rows("response_json_schema_routing_respects_the_existing_destination")
}

#[test]
fn function_schema_routing_preserves_an_existing_json_destination() -> TestResult {
    assert_rows("function_schema_routing_preserves_an_existing_json_destination")
}

#[test]
fn function_declarations_keep_open_members_and_transform_both_schemas() -> TestResult {
    assert_rows("function_declarations_keep_open_members_and_transform_both_schemas")
}

#[test]
fn raw_function_json_schemas_are_never_transformed() -> TestResult {
    assert_rows("raw_function_json_schemas_are_never_transformed")
}

#[test]
fn projection_failures_follow_the_source_phase_order() -> TestResult {
    assert_rows("projection_failures_follow_the_source_phase_order")
}

#[test]
fn projected_json_reuses_canonical_numbers_and_key_order() -> TestResult {
    assert_rows("projected_json_reuses_canonical_numbers_and_key_order")
}

#[test]
fn schema_property_failures_follow_canonical_key_order() -> TestResult {
    assert_rows("schema_property_failures_follow_canonical_key_order")
}

#[test]
fn selected_transform_inputs_fail_without_silent_defaults() -> TestResult {
    assert_rows("selected_transform_inputs_fail_without_silent_defaults")
}

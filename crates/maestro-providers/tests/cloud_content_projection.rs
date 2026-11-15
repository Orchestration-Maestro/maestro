//! Cloud wire projection through the shared payload boundary.

use crate::{chat::TestResult, content_wire::assert_rows};

#[test]
fn cloud_model_resources_keep_qualified_and_publisher_paths() -> TestResult {
    assert_rows("cloud_model_resources_keep_qualified_and_publisher_paths")
}

#[test]
fn cloud_projection_failures_keep_model_content_config_order() -> TestResult {
    assert_rows("cloud_projection_failures_keep_model_content_config_order")
}

#[test]
fn cloud_presence_and_client_controls_do_not_invent_wire_fields() -> TestResult {
    assert_rows("cloud_presence_and_client_controls_do_not_invent_wire_fields")
}

#[test]
fn cloud_content_unions_share_grouping_and_rejections() -> TestResult {
    assert_rows("cloud_content_unions_share_grouping_and_rejections")
}

#[test]
fn cloud_system_instruction_uses_shared_content_rules() -> TestResult {
    assert_rows("cloud_system_instruction_uses_shared_content_rules")
}

#[test]
fn cloud_parts_keep_open_nested_records_and_omit_null() -> TestResult {
    assert_rows("cloud_parts_keep_open_nested_records_and_omit_null")
}

#[test]
fn cloud_parts_reject_unsupported_members_on_presence() -> TestResult {
    assert_rows("cloud_parts_reject_unsupported_members_on_presence")
}

#[test]
fn cloud_generation_fields_keep_destinations_and_false_values() -> TestResult {
    assert_rows("cloud_generation_fields_keep_destinations_and_false_values")
}

#[test]
fn cloud_generation_order_is_not_input_order() -> TestResult {
    assert_rows("cloud_generation_order_is_not_input_order")
}

#[test]
fn cloud_body_fields_and_safety_are_not_public_endpoint_filtered() -> TestResult {
    assert_rows("cloud_body_fields_and_safety_are_not_public_endpoint_filtered")
}

#[test]
fn cloud_config_rejects_enhanced_civic_presence() -> TestResult {
    assert_rows("cloud_config_rejects_enhanced_civic_presence")
}

#[test]
fn cloud_speech_uses_existing_voice_conversion() -> TestResult {
    assert_rows("cloud_speech_uses_existing_voice_conversion")
}

#[test]
fn cloud_cached_names_use_supplied_project_and_location() -> TestResult {
    assert_rows("cloud_cached_names_use_supplied_project_and_location")
}

#[test]
fn cloud_image_fields_map_output_aliases() -> TestResult {
    assert_rows("cloud_image_fields_map_output_aliases")
}

#[test]
fn cloud_image_output_options_merge_after_aliases() -> TestResult {
    assert_rows("cloud_image_output_options_merge_after_aliases")
}

#[test]
fn cloud_tools_keep_cloud_records_and_open_nested_values() -> TestResult {
    assert_rows("cloud_tools_keep_cloud_records_and_open_nested_values")
}

#[test]
fn cloud_tools_reject_file_search_and_server_records() -> TestResult {
    assert_rows("cloud_tools_reject_file_search_and_server_records")
}

#[test]
fn cloud_tool_lists_keep_empty_and_reject_nonlists() -> TestResult {
    assert_rows("cloud_tool_lists_keep_empty_and_reject_nonlists")
}

#[test]
fn cloud_declarations_select_six_fields_after_schema_preparation() -> TestResult {
    assert_rows("cloud_declarations_select_six_fields_after_schema_preparation")
}

#[test]
fn cloud_declarations_reject_behavior_after_schema_preparation() -> TestResult {
    assert_rows("cloud_declarations_reject_behavior_after_schema_preparation")
}

#[test]
fn cloud_tool_config_keeps_streamed_arguments_and_retrieval() -> TestResult {
    assert_rows("cloud_tool_config_keeps_streamed_arguments_and_retrieval")
}

#[test]
fn cloud_tool_config_rejects_server_invocation_presence() -> TestResult {
    assert_rows("cloud_tool_config_rejects_server_invocation_presence")
}

#[test]
fn cloud_schemas_reuse_conversion_across_all_three_positions() -> TestResult {
    assert_rows("cloud_schemas_reuse_conversion_across_all_three_positions")
}

#[test]
fn cloud_nullable_siblings_reuse_the_established_schema_fix() -> TestResult {
    assert_rows("cloud_nullable_siblings_reuse_the_established_schema_fix")
}

#[test]
fn cloud_json_schema_routing_respects_existing_destinations() -> TestResult {
    assert_rows("cloud_json_schema_routing_respects_existing_destinations")
}

#[test]
fn cloud_json_bytes_use_shared_key_order_and_number_spelling() -> TestResult {
    assert_rows("cloud_json_bytes_use_shared_key_order_and_number_spelling")
}

#[test]
fn cloud_projection_consumes_the_selected_hook_value() -> TestResult {
    assert_rows("cloud_projection_consumes_the_selected_hook_value")
}

#[test]
fn cloud_and_public_endpoint_policies_remain_independent() -> TestResult {
    assert_rows("cloud_and_public_endpoint_policies_remain_independent")
}

//! Recorded conversion cases for the shared content-generation helpers.

use maestro_models::providers::content::google_shared::{
    self, GoogleThinkingLevel as ModuleThinkingLevel, ThoughtMetadata,
};
use maestro_models::{Context, GoogleThinkingLevel, Model, Tool};
use serde::Deserialize;
use serde_json::Value;

/// Result of a fallible test step.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Every test name a fixture row may select.
const TEST_NAMES: [&str; 29] = [
    "thought_flag_alone_classifies_parts",
    "signature_retention_keeps_last_nonempty_value",
    "call_id_requirement_uses_exact_model_prefixes",
    "tool_choice_uses_default_for_unrecognized_strings",
    "typed_finish_mapping_covers_every_declared_reason",
    "raw_finish_mapping_defaults_other_strings_to_error",
    "unknown_typed_finish_is_an_exact_error",
    "thinking_level_serialization_retains_wire_names",
    "empty_context_and_empty_turns_are_distinct",
    "user_parts_keep_order_duplicates_and_images",
    "nonvision_history_uses_shared_image_placeholders",
    "assistant_blank_filter_uses_authored_whitespace",
    "replay_identity_controls_text_thinking_and_call_signatures",
    "replay_signatures_use_shape_validation_not_decoding",
    "unsigned_calls_stay_unsigned_in_both_protocols",
    "projection_omits_failed_assistants_but_keeps_results",
    "projection_handles_redaction_before_content_conversion",
    "projection_inserts_missing_results_at_turn_boundaries",
    "call_ids_normalize_only_on_cross_identity_replay",
    "normalized_call_ids_keep_the_sixty_four_character_boundary",
    "tool_results_select_output_error_and_image_fallback",
    "image_routing_uses_major_version_and_unknown_family_fallback",
    "function_results_merge_only_with_last_function_result_turn",
    "call_arguments_keep_nested_data_without_history_mutation",
    "empty_tools_are_omitted_in_both_schema_modes",
    "tool_schema_modes_preserve_or_remove_only_metadata",
    "plain_schema_preserves_tools_order_and_source_values",
    "schema_array_metadata_is_removed_without_changing_leaf_data",
    "history_arguments_keep_nullish_defaults",
];

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Test that owns the case.
    test: String,
    /// Helper under test.
    operation: String,
    /// Helper input.
    input: Value,
    /// Case identifier.
    id: String,
    /// Recorded result.
    expected: Expected,
}

/// What a helper produced or was recorded to produce.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Expected {
    /// A value.
    Value(Value),
    /// No value.
    Absent(bool),
    /// A failure message.
    Error(String),
}

/// Input of `thinking-part` classification.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ThoughtInput {
    /// Thought flag.
    thought: Option<bool>,
    /// Replay signature.
    thought_signature: Option<String>,
}

/// Input of signature retention.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainInput {
    /// Signature held so far.
    existing: Option<String>,
    /// Signature of the newest delta.
    incoming: Option<String>,
}

/// Input of message conversion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MessagesInput {
    /// Target model.
    model: Model,
    /// Conversation.
    context: Context,
}

/// Input of tool conversion.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ToolsInput {
    /// Offered tools.
    tools: Vec<Tool>,
    /// Schema mode.
    use_parameters: bool,
}

/// Load the recorded rows.
fn rows() -> TestResult<Vec<Row>> {
    Ok(serde_json::from_str(include_str!(
        "fixtures/content_conversion.json"
    ))?)
}

/// Decode a row input into its typed form.
fn input<T: for<'de> Deserialize<'de>>(row: &Row) -> TestResult<T> {
    Ok(serde_json::from_value(row.input.clone())?)
}

/// Turn a helper result into the recorded form.
fn value<T: serde::Serialize>(result: T) -> TestResult<Expected> {
    Ok(Expected::Value(serde_json::to_value(result)?))
}

/// Run the helper a row names.
fn run(row: &Row) -> TestResult<Expected> {
    match row.operation.as_str() {
        "isThinkingPart" => {
            let given: ThoughtInput = input(row)?;
            value(google_shared::is_thinking_part(&ThoughtMetadata {
                thought: given.thought,
                thought_signature: given.thought_signature,
            }))
        }
        "retainThoughtSignature" => {
            let given: RetainInput = input(row)?;
            match google_shared::retain_thought_signature(
                given.existing.as_deref(),
                given.incoming.as_deref(),
            ) {
                Some(signature) => value(signature),
                None => Ok(Expected::Absent(true)),
            }
        }
        "requiresToolCallId" => value(google_shared::requires_tool_call_id(&input::<String>(row)?)),
        "mapToolChoice" => value(google_shared::map_tool_choice(&input::<String>(row)?)),
        "mapStopReason" => match google_shared::map_stop_reason(&input::<String>(row)?) {
            Ok(reason) => value(reason),
            Err(error) => Ok(Expected::Error(error.message)),
        },
        "mapStopReasonString" => value(google_shared::map_stop_reason_string(&input::<String>(
            row,
        )?)),
        "thinkingLevel" => value(thinking_level(&input::<String>(row)?)?),
        "convertMessages" => {
            let given: MessagesInput = input(row)?;
            let before = given.context.clone();
            let converted = google_shared::convert_messages(&given.model, &given.context);
            assert_eq!(given.context, before, "input of case {}", row.id);
            value(converted)
        }
        "convertTools" => {
            let given: ToolsInput = input(row)?;
            let before = given.tools.clone();
            let converted = google_shared::convert_tools(&given.tools, given.use_parameters);
            assert_eq!(given.tools, before, "input of case {}", row.id);
            match converted {
                Some(declarations) => value(declarations),
                None => Ok(Expected::Absent(true)),
            }
        }
        other => Err(format!("unknown operation {other}").into()),
    }
}

/// Build a thinking level through the crate-root export from its wire name.
fn thinking_level(wire: &str) -> TestResult<GoogleThinkingLevel> {
    Ok(match wire {
        "THINKING_LEVEL_UNSPECIFIED" => GoogleThinkingLevel::ThinkingLevelUnspecified,
        "MINIMAL" => GoogleThinkingLevel::Minimal,
        "LOW" => GoogleThinkingLevel::Low,
        "MEDIUM" => GoogleThinkingLevel::Medium,
        "HIGH" => GoogleThinkingLevel::High,
        other => return Err(format!("unknown thinking level {other}").into()),
    })
}

/// Render a recorded result with its object members in order.
fn rendered(expected: &Expected) -> TestResult<String> {
    Ok(match expected {
        Expected::Value(value) => serde_json::to_string(value)?,
        other => format!("{other:?}"),
    })
}

/// Check every row of a test; `ordered` also compares object member order.
fn check(name: &str, ordered: bool) -> TestResult {
    let selected: Vec<Row> = rows()?.into_iter().filter(|row| row.test == name).collect();
    assert!(!selected.is_empty(), "{name} has no recorded case");
    for row in &selected {
        let actual = run(row)?;
        assert_eq!(actual, row.expected, "case {}", row.id);
        if ordered {
            assert_eq!(
                rendered(&actual)?,
                rendered(&row.expected)?,
                "order of case {}",
                row.id
            );
        }
    }
    Ok(())
}

/// Every recorded case belongs to a named test, so none is left unchecked.
#[test]
fn every_recorded_case_belongs_to_a_named_test() -> TestResult {
    let owners: Vec<String> = rows()?.into_iter().map(|row| row.test).collect();
    assert_eq!(owners.len(), 169);
    for owner in &owners {
        assert!(
            TEST_NAMES.contains(&owner.as_str()),
            "unowned case in {owner}"
        );
    }
    for name in TEST_NAMES {
        assert!(
            owners.iter().any(|owner| owner == name),
            "{name} has no case"
        );
    }
    Ok(())
}

#[test]
fn thought_flag_alone_classifies_parts() -> TestResult {
    check("thought_flag_alone_classifies_parts", false)
}

#[test]
fn signature_retention_keeps_last_nonempty_value() -> TestResult {
    check("signature_retention_keeps_last_nonempty_value", false)
}

#[test]
fn call_id_requirement_uses_exact_model_prefixes() -> TestResult {
    check("call_id_requirement_uses_exact_model_prefixes", false)
}

#[test]
fn tool_choice_uses_default_for_unrecognized_strings() -> TestResult {
    check("tool_choice_uses_default_for_unrecognized_strings", false)
}

#[test]
fn typed_finish_mapping_covers_every_declared_reason() -> TestResult {
    check("typed_finish_mapping_covers_every_declared_reason", false)
}

#[test]
fn raw_finish_mapping_defaults_other_strings_to_error() -> TestResult {
    check("raw_finish_mapping_defaults_other_strings_to_error", false)
}

#[test]
fn unknown_typed_finish_is_an_exact_error() -> TestResult {
    check("unknown_typed_finish_is_an_exact_error", false)
}

#[test]
fn thinking_level_serialization_retains_wire_names() -> TestResult {
    check("thinking_level_serialization_retains_wire_names", false)?;
    let _: ModuleThinkingLevel = GoogleThinkingLevel::Low;
    Ok(())
}

#[test]
fn empty_context_and_empty_turns_are_distinct() -> TestResult {
    check("empty_context_and_empty_turns_are_distinct", false)
}

#[test]
fn user_parts_keep_order_duplicates_and_images() -> TestResult {
    check("user_parts_keep_order_duplicates_and_images", false)
}

#[test]
fn nonvision_history_uses_shared_image_placeholders() -> TestResult {
    check("nonvision_history_uses_shared_image_placeholders", false)
}

#[test]
fn assistant_blank_filter_uses_authored_whitespace() -> TestResult {
    check("assistant_blank_filter_uses_authored_whitespace", false)
}

#[test]
fn replay_identity_controls_text_thinking_and_call_signatures() -> TestResult {
    check(
        "replay_identity_controls_text_thinking_and_call_signatures",
        false,
    )
}

#[test]
fn replay_signatures_use_shape_validation_not_decoding() -> TestResult {
    check("replay_signatures_use_shape_validation_not_decoding", false)
}

#[test]
fn unsigned_calls_stay_unsigned_in_both_protocols() -> TestResult {
    check("unsigned_calls_stay_unsigned_in_both_protocols", false)
}

#[test]
fn projection_omits_failed_assistants_but_keeps_results() -> TestResult {
    check(
        "projection_omits_failed_assistants_but_keeps_results",
        false,
    )
}

#[test]
fn projection_handles_redaction_before_content_conversion() -> TestResult {
    check(
        "projection_handles_redaction_before_content_conversion",
        false,
    )
}

#[test]
fn projection_inserts_missing_results_at_turn_boundaries() -> TestResult {
    check(
        "projection_inserts_missing_results_at_turn_boundaries",
        false,
    )
}

#[test]
fn call_ids_normalize_only_on_cross_identity_replay() -> TestResult {
    check("call_ids_normalize_only_on_cross_identity_replay", false)
}

#[test]
fn normalized_call_ids_keep_the_sixty_four_character_boundary() -> TestResult {
    check(
        "normalized_call_ids_keep_the_sixty_four_character_boundary",
        false,
    )
}

#[test]
fn tool_results_select_output_error_and_image_fallback() -> TestResult {
    check("tool_results_select_output_error_and_image_fallback", false)
}

#[test]
fn image_routing_uses_major_version_and_unknown_family_fallback() -> TestResult {
    check(
        "image_routing_uses_major_version_and_unknown_family_fallback",
        false,
    )
}

#[test]
fn function_results_merge_only_with_last_function_result_turn() -> TestResult {
    check(
        "function_results_merge_only_with_last_function_result_turn",
        false,
    )
}

#[test]
fn call_arguments_keep_nested_data_without_history_mutation() -> TestResult {
    check(
        "call_arguments_keep_nested_data_without_history_mutation",
        true,
    )
}

#[test]
fn history_arguments_keep_nullish_defaults() -> TestResult {
    check("history_arguments_keep_nullish_defaults", true)
}

#[test]
fn empty_tools_are_omitted_in_both_schema_modes() -> TestResult {
    check("empty_tools_are_omitted_in_both_schema_modes", false)
}

#[test]
fn tool_schema_modes_preserve_or_remove_only_metadata() -> TestResult {
    check("tool_schema_modes_preserve_or_remove_only_metadata", true)
}

#[test]
fn plain_schema_preserves_tools_order_and_source_values() -> TestResult {
    check("plain_schema_preserves_tools_order_and_source_values", true)
}

#[test]
fn schema_array_metadata_is_removed_without_changing_leaf_data() -> TestResult {
    check(
        "schema_array_metadata_is_removed_without_changing_leaf_data",
        true,
    )
}

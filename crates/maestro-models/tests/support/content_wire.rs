//! Recorded wire-projection rows: strict decoding, assignment checks and result comparison.

use std::collections::BTreeSet;

use maestro_models::compact_json;
use maestro_models::providers::content::google_shared::project_content_request;
use serde::Deserialize;
use serde_json::Value;

use crate::chat::{self, TestResult};

/// Every test name a fixture row may select.
pub const TEST_NAMES: [&str; 50] = [
    "configuration_presence_controls_the_generation_object",
    "unknown_and_client_only_fields_do_not_enter_the_body",
    "model_paths_preserve_qualifiers_and_authored_text",
    "model_rejections_precede_contents_and_configuration",
    "missing_or_null_contents_are_not_invented",
    "content_unions_keep_turns_parts_and_duplicates",
    "contents_reject_empty_mixed_and_unwrapped_call_forms",
    "system_instructions_use_the_single_content_union",
    "empty_system_part_lists_fail_before_sampling",
    "part_fields_keep_open_values_and_skip_null",
    "combined_part_fields_are_not_exclusive_variants",
    "nested_part_projection_selects_only_owned_members",
    "unsupported_part_members_fail_even_when_null",
    "configuration_fields_keep_their_destination_and_zero_values",
    "configuration_nulls_are_omitted_without_defaults",
    "configuration_fields_are_projected_in_source_order",
    "passthrough_fields_do_not_gain_typed_ingress_validation",
    "unsupported_configuration_members_fail_on_presence",
    "speech_configuration_accepts_names_and_open_records",
    "cached_content_names_add_only_a_missing_single_segment_prefix",
    "image_configuration_selects_supported_fields",
    "unsupported_image_configuration_fails_on_presence",
    "safety_settings_keep_order_and_select_fields",
    "safety_method_is_rejected_before_threshold",
    "tools_presence_preserves_an_explicit_empty_list",
    "tool_fields_preserve_open_values_and_skip_null",
    "tool_kinds_and_declarations_can_coexist",
    "unsupported_tool_members_fail_on_presence",
    "search_tool_selects_only_supported_configuration",
    "unsupported_search_fields_fail_on_presence",
    "maps_tool_selects_api_key_and_widget_fields",
    "unsupported_maps_auth_fields_fail_on_presence",
    "tool_configuration_selects_fields_without_declarations",
    "streamed_function_arguments_are_rejected_on_presence",
    "schema_types_use_the_declared_uppercase_vocabulary",
    "schema_type_arrays_keep_order_duplicates_and_nullable",
    "schema_rejects_exclusive_type_conflicts",
    "schema_nullable_wrappers_and_general_unions_keep_their_rules",
    "schema_traversal_changes_only_owned_schema_positions",
    "nullable_schema_siblings_survive_conversion",
    "nullable_schema_correction_is_shared_by_response_and_function_schemas",
    "response_json_schema_routing_respects_the_existing_destination",
    "function_schema_routing_preserves_an_existing_json_destination",
    "function_declarations_keep_open_members_and_transform_both_schemas",
    "raw_function_json_schemas_are_never_transformed",
    "projection_failures_follow_the_source_phase_order",
    "projected_json_reuses_canonical_numbers_and_key_order",
    "hook_replacement_changes_model_content_and_configuration",
    "schema_property_failures_follow_canonical_key_order",
    "selected_transform_inputs_fail_without_silent_defaults",
];

/// The committed rows.
const FIXTURE: &str = include_str!("../fixtures/content_wire.json");

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Test that owns the case.
    test: String,
    /// JSON source text of the value handed to the projector.
    pub input: String,
    /// Recorded result.
    expected: Expected,
}

/// Recorded result of one case.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// The projection succeeds with this model path and compact body text.
    Projected {
        /// Normalized model resource path.
        model: String,
        /// Compact canonical body text.
        body: String,
    },
    /// The projection fails with this complete message.
    Rejected {
        /// Complete authored message.
        error: String,
    },
}

/// Reject rows no test owns and repeated input texts.
fn check_assignment(all: &[Value]) -> TestResult {
    let mut inputs = BTreeSet::new();
    for row in all {
        let name = row["test"].as_str().ok_or("row without a test name")?;
        assert!(TEST_NAMES.contains(&name), "unassigned row for {name}");
        let input = row["input"].as_str().ok_or("row without input text")?;
        assert!(inputs.insert(input.to_owned()), "repeated input in {name}");
    }
    Ok(())
}

/// The strictly decoded rows owned by `test`; the selection is never empty.
pub fn rows(test: &str) -> TestResult<Vec<Row>> {
    let all: Vec<Value> = serde_json::from_str(FIXTURE)?;
    check_assignment(&all)?;
    let owned = chat::rows(FIXTURE, test)?;
    assert!(!owned.is_empty(), "no rows for {test}");
    owned
        .into_iter()
        .map(|row| Ok(serde_json::from_value::<Row>(row)?))
        .collect()
}

impl Row {
    /// Project the row's input and compare the model path, body bytes or message.
    pub fn check(&self) -> TestResult {
        self.check_value(serde_json::from_str(&self.input)?)
    }

    /// Project `params` and compare the model path, body bytes or message with the row.
    pub fn check_value(&self, params: Value) -> TestResult {
        let result = project_content_request(params);
        match (&self.expected, result) {
            (Expected::Projected { model, body }, Ok(projected)) => {
                assert_eq!(&projected.model, model, "{}", self.input);
                assert_eq!(
                    &compact_json(&Value::Object(projected.body))?,
                    body,
                    "{}",
                    self.input
                );
            }
            (Expected::Rejected { error }, Err(rejected)) => {
                assert_eq!(&rejected.message, error, "{}", self.input);
            }
            (_, other) => {
                return Err(format!(
                    "outcome differs for {}: {:?}",
                    self.input,
                    other.map(|p| p.model)
                )
                .into());
            }
        }
        Ok(())
    }
}

/// Check every row owned by `test`.
pub fn assert_rows(test: &str) -> TestResult {
    rows(test)?.iter().try_for_each(Row::check)
}

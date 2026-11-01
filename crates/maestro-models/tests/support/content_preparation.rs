//! Recorded preparation rows, their materialization into typed inputs and their checks.

#![cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;

use maestro_models::providers::content::google::{
    GoogleOptions, GoogleThinking, build_params, build_simple_options,
};
use maestro_models::providers::content::google_shared::{
    FunctionCallingConfigMode, GenerateContentParameters,
};
use maestro_models::{
    Cancellation, Context, GoogleThinkingLevel, Model, SimpleStreamOptions, StreamOptions,
    ThinkingBudgets, ThinkingLevel, ToolChoice, compact_json,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::chat::{self, TestResult};
use crate::environment_rows::check_by_environment;

/// Every test name a fixture row may select.
pub const TEST_NAMES: [&str; 25] = [
    "raw_parameters_keep_empty_config_and_model_identity",
    "generation_fields_preserve_supplied_numbers",
    "system_instruction_omits_only_empty_text",
    "tool_choice_requires_nonempty_declarations",
    "tool_schemas_keep_the_full_schema_field",
    "thinking_requires_an_explicit_request_and_reasoning_support",
    "explicit_thinking_levels_override_budget_values",
    "explicit_thinking_budgets_are_not_clamped",
    "disabled_thinking_uses_the_lowest_family_setting",
    "model_family_recognition_uses_unanchored_ascii_patterns",
    "simple_efforts_select_each_family_level",
    "simple_efforts_select_each_family_budget",
    "custom_budgets_override_only_the_selected_slot",
    "level_families_ignore_custom_budgets",
    "simple_absent_reasoning_disables_family_thinking",
    "simple_token_limits_use_the_base_option_owner",
    "supported_effort_clamping_precedes_family_mapping",
    "supported_extra_high_uses_the_high_policy",
    "nonreasoning_models_never_gain_thinking_config",
    "simple_key_resolution_preserves_nonempty_values",
    "simple_missing_key_fails_before_parameter_preparation",
    "raw_signal_checks_happen_at_preparation",
    "simple_signal_checks_wait_until_parameter_building",
    "simple_tool_selection_does_not_leak_into_raw_options",
    "numeric_payloads_use_the_existing_json_number_semantics",
];

/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Test that owns the case.
    test: String,
    /// Case input.
    input: Input,
    /// Recorded result.
    expected: Expected,
}

/// Which preparation operations a row drives.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Operation {
    /// Raw parameter building only.
    Raw,
    /// Simple option selection followed by parameter building.
    Simple,
}

/// Case input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Operation under test.
    operation: Operation,
    /// Members overlaid on the baseline model.
    model: Value,
    /// Conversation.
    context: Value,
    /// Options in their recorded form.
    options: OptionsInput,
    /// Variables supplied to the child process that evaluates the row; see the child runner.
    env: Option<BTreeMap<String, String>>,
}

/// A number, or a tag naming a value JSON cannot hold.
#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum Number {
    /// A finite number.
    Plain(f64),
    /// A special value.
    Special {
        /// Which special value.
        float: String,
    },
}

impl Number {
    /// The typed value.
    fn value(&self) -> TestResult<f64> {
        Ok(match self {
            Self::Plain(value) => *value,
            Self::Special { float } => match float.as_str() {
                "nan" => f64::NAN,
                "positive-infinity" => f64::INFINITY,
                "negative-infinity" => f64::NEG_INFINITY,
                "negative-zero" => -0.0,
                other => return Err(format!("unknown special number {other}").into()),
            },
        })
    }
}

/// Recorded options of either operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptionsInput {
    /// Sampling temperature.
    temperature: Option<Number>,
    /// Output token limit.
    max_tokens: Option<Number>,
    /// Signal state.
    signal: Option<String>,
    /// Supplied key.
    api_key: Option<String>,
    /// Tool choice in the recorded spelling of its operation.
    tool_choice: Option<Value>,
    /// Raw thinking request.
    thinking: Option<ThinkingInput>,
    /// Simple reasoning effort.
    reasoning: Option<ThinkingLevel>,
    /// Simple per-effort budgets.
    thinking_budgets: Option<ThinkingBudgets>,
}

/// Recorded raw thinking request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ThinkingInput {
    /// Whether thinking is requested.
    enabled: bool,
    /// Supplied budget.
    budget_tokens: Option<Number>,
    /// Supplied level name.
    level: Option<String>,
}

/// Recorded result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Serialized parameters.
    params: Option<Value>,
    /// Selected raw options of a simple row.
    options: Option<Value>,
    /// Failure message.
    error: Option<String>,
    /// Typed state of special numbers before serialization.
    numerics: Option<BTreeMap<String, String>>,
}

/// Load every row, rejecting unassigned and duplicate cases.
fn all_rows() -> TestResult<Vec<Row>> {
    let text = include_str!("../fixtures/content_preparation.json");
    let rows: Vec<Row> = serde_json::from_str(text)?;
    let raw: Vec<Value> = serde_json::from_str(text)?;
    let mut seen = std::collections::BTreeSet::new();
    for row in &raw {
        assert!(seen.insert(row["input"].to_string()), "duplicate query");
    }
    for row in &rows {
        assert!(TEST_NAMES.contains(&row.test.as_str()), "unassigned row");
    }
    Ok(rows)
}

/// The baseline model with the row's members overlaid.
fn model(overlay: &Value) -> TestResult<Model> {
    let members = json!({
        "id": "gemini-2.5-flash", "name": "fixture", "api": "google-generative-ai",
        "provider": "google", "baseUrl": "https://fixture.invalid/v1beta", "reasoning": true,
        "input": ["text", "image"],
        "cost": {"input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4},
        "contextWindow": 128_000, "maxTokens": 8192
    });
    chat::model(&chat::overlay(members, overlay))
}

/// The level named by its wire spelling.
fn level(name: &str) -> TestResult<GoogleThinkingLevel> {
    Ok(match name {
        "THINKING_LEVEL_UNSPECIFIED" => GoogleThinkingLevel::ThinkingLevelUnspecified,
        "MINIMAL" => GoogleThinkingLevel::Minimal,
        "LOW" => GoogleThinkingLevel::Low,
        "MEDIUM" => GoogleThinkingLevel::Medium,
        "HIGH" => GoogleThinkingLevel::High,
        other => return Err(format!("unknown level {other}").into()),
    })
}

/// The wire spelling of a level.
fn level_name(level: GoogleThinkingLevel) -> Value {
    serde_json::to_value(level).unwrap_or(Value::Null)
}

/// A live signal in the recorded state.
fn signal(state: &str) -> TestResult<Cancellation> {
    let signal = Cancellation::new();
    match state {
        "active" => {}
        "aborted" => signal.abort(),
        other => return Err(format!("unknown signal state {other}").into()),
    }
    Ok(signal)
}

/// Common options carrying the recorded numbers, key and signal.
fn common(options: &OptionsInput) -> TestResult<StreamOptions> {
    Ok(StreamOptions {
        temperature: options
            .temperature
            .as_ref()
            .map(Number::value)
            .transpose()?,
        max_tokens: options.max_tokens.as_ref().map(Number::value).transpose()?,
        signal: options.signal.as_deref().map(signal).transpose()?,
        api_key: options.api_key.clone(),
        ..StreamOptions::default()
    })
}

/// Raw options from their recorded form.
fn raw_options(options: &OptionsInput) -> TestResult<GoogleOptions> {
    let tool_choice = match options.tool_choice.as_ref().and_then(Value::as_str) {
        None => None,
        Some("auto") => Some(FunctionCallingConfigMode::Auto),
        Some("none") => Some(FunctionCallingConfigMode::None),
        Some("any") => Some(FunctionCallingConfigMode::Any),
        Some(other) => return Err(format!("unknown raw tool choice {other}").into()),
    };
    let thinking = options
        .thinking
        .as_ref()
        .map(|thinking| -> TestResult<GoogleThinking> {
            Ok(GoogleThinking {
                enabled: thinking.enabled,
                budget_tokens: thinking
                    .budget_tokens
                    .as_ref()
                    .map(Number::value)
                    .transpose()?,
                level: thinking.level.as_deref().map(level).transpose()?,
            })
        })
        .transpose()?;
    Ok(GoogleOptions {
        common: common(options)?,
        tool_choice,
        thinking,
    })
}

/// Simple options from their recorded form.
fn simple_options(options: &OptionsInput) -> TestResult<SimpleStreamOptions> {
    Ok(SimpleStreamOptions {
        common: common(options)?,
        reasoning: options.reasoning,
        thinking_budgets: options.thinking_budgets.clone(),
        tool_choice: options
            .tool_choice
            .clone()
            .map(serde_json::from_value::<ToolChoice>)
            .transpose()?,
    })
}

/// The selected raw options in the recorded form.
fn options_record(options: &GoogleOptions) -> Value {
    let mut record = serde_json::Map::new();
    let common = &options.common;
    if let Some(temperature) = common.temperature {
        record.insert("temperature".into(), json!(temperature));
    }
    if let Some(max_tokens) = common.max_tokens {
        record.insert("maxTokens".into(), json!(max_tokens));
    }
    if let Some(key) = &common.api_key {
        record.insert("apiKey".into(), json!(key));
    }
    if let Some(choice) = options.tool_choice {
        record.insert(
            "toolChoice".into(),
            serde_json::to_value(choice).unwrap_or_default(),
        );
    }
    if let Some(thinking) = &options.thinking {
        let mut members = serde_json::Map::new();
        members.insert("enabled".into(), json!(thinking.enabled));
        if let Some(budget) = thinking.budget_tokens {
            members.insert("budgetTokens".into(), json!(budget));
        }
        if let Some(level) = thinking.level {
            members.insert("level".into(), level_name(level));
        }
        record.insert("thinking".into(), Value::Object(members));
    }
    Value::Object(record)
}

/// Whether a typed number is in the recorded special state.
fn in_state(value: Option<f64>, tag: &str) -> bool {
    match (value, tag) {
        (Some(v), "nan") => v.is_nan(),
        (Some(v), "positive-infinity") => v == f64::INFINITY,
        (Some(v), "negative-infinity") => v == f64::NEG_INFINITY,
        (Some(v), "negative-zero") => v == 0.0 && v.is_sign_negative(),
        _ => false,
    }
}

/// Compare serialized parameters with the recorded tree as compact text, which also fixes
/// key order and number spelling.
fn assert_params(params: &GenerateContentParameters, expected: &Value) -> TestResult {
    assert_eq!(
        compact_json(&serde_json::to_value(params)?)?,
        compact_json(expected)?
    );
    Ok(())
}

/// Check the recorded special-number states on the typed parameters.
fn assert_numerics(
    params: &GenerateContentParameters,
    tags: &BTreeMap<String, String>,
) -> TestResult {
    let config = &params.config;
    for (name, tag) in tags {
        let value = match name.as_str() {
            "temperature" => config.temperature,
            "maxOutputTokens" => config.max_output_tokens,
            "thinkingBudget" => config
                .thinking_config
                .as_ref()
                .and_then(|thinking| thinking.thinking_budget),
            other => return Err(format!("unknown numeric member {other}").into()),
        };
        assert!(in_state(value, tag), "{name} is not {tag}");
    }
    Ok(())
}

/// Check one parameters result against the recorded error or tree.
fn assert_outcome(
    outcome: Result<GenerateContentParameters, maestro_models::DiagnosticErrorInfo>,
    expected: &Expected,
) -> TestResult {
    match (outcome, &expected.error, &expected.params) {
        (Ok(params), None, Some(tree)) => {
            assert_params(&params, tree)?;
            if let Some(tags) = &expected.numerics {
                assert_numerics(&params, tags)?;
            }
        }
        (Err(error), Some(message), None) => {
            assert_eq!(&error.message, message);
            assert_eq!(error.name.as_deref(), Some("Error"));
            assert!(error.stack.is_none() && error.code.is_none());
        }
        _ => return Err("outcome does not match the recorded kind".into()),
    }
    Ok(())
}

/// The only test whose simple rows fail during option selection.
const MISSING_KEY_TEST: &str = "simple_missing_key_fails_before_parameter_preparation";

/// Evaluate one row through the public operations.
fn check(row: &Row) -> TestResult {
    let model = model(&row.input.model)?;
    let context: Context = serde_json::from_value(row.input.context.clone())?;
    let expected = &row.expected;
    match row.input.operation {
        Operation::Raw => {
            let options = raw_options(&row.input.options)?;
            assert_outcome(build_params(&model, &context, Some(&options)), expected)
        }
        Operation::Simple => {
            let simple = simple_options(&row.input.options)?;
            let selected = match build_simple_options(&model, &context, Some(&simple)) {
                Ok(selected) => selected,
                Err(error) if row.test == MISSING_KEY_TEST => {
                    return assert_outcome(Err(error), expected);
                }
                Err(error) => return Err(format!("simple preparation failed: {error}").into()),
            };
            if let Some(options) = &expected.options {
                assert_eq!(
                    compact_json(&options_record(&selected))?,
                    compact_json(options)?
                );
            }
            assert_eq!(
                selected
                    .common
                    .signal
                    .as_ref()
                    .map(Cancellation::is_aborted),
                simple.common.signal.as_ref().map(Cancellation::is_aborted)
            );
            assert_outcome(build_params(&model, &context, Some(&selected)), expected)
        }
    }
}

/// Run every row of `test`; rows with an environment run in child processes sharing it.
///
/// # Errors
/// Returns the first fixture, materialization or child failure.
pub fn assert_rows(test: &str) -> TestResult {
    let rows: Vec<Row> = all_rows()?
        .into_iter()
        .filter(|row| row.test == test)
        .collect();
    assert!(!rows.is_empty(), "{test} has no rows");
    check_by_environment(test, &rows, |row| row.input.env.as_ref(), check)
}

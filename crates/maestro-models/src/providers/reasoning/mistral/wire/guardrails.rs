//! Typed guardrails transport records.

use super::{Nullable, Object, nullable, optional};
use serde::{Deserialize, Serialize};

/// Transport fields for `GuardrailConfig`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct GuardrailConfig {
    /// The `blockOnError` field.
    #[serde(default)]
    block_on_error: bool,
    /// The `moderationLlmV1` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    moderation_llm_v1: Option<Nullable<Object<ModerationLlmv1Config>>>,
    /// The `moderationLlmV2` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    moderation_llm_v2: Option<Nullable<Object<ModerationLlmv2Config>>>,
}

/// Transport fields for `ModerationLlmv1Config`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ModerationLlmv1Config {
    /// The `modelName` field.
    #[serde(default = "moderation_llmv1_config_model_name_default")]
    model_name: String,
    /// The `customCategoryThresholds` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    custom_category_thresholds: Option<Nullable<Object<ModerationLlmv1CategoryThresholds>>>,
    /// The `ignoreOtherCategories` field.
    #[serde(default)]
    ignore_other_categories: bool,
    /// The `action` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    action: Option<String>,
}

/// Transport fields for `ModerationLlmv2Config`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ModerationLlmv2Config {
    /// The `modelName` field.
    #[serde(default = "moderation_llmv2_config_model_name_default")]
    model_name: String,
    /// The `customCategoryThresholds` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    custom_category_thresholds: Option<Nullable<Object<ModerationLlmv2CategoryThresholds>>>,
    /// The `ignoreOtherCategories` field.
    #[serde(default)]
    ignore_other_categories: bool,
    /// The `action` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    action: Option<String>,
}

/// Transport fields for `ModerationLlmv1CategoryThresholds`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ModerationLlmv1CategoryThresholds {
    /// The `sexual` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    sexual: Option<Nullable<serde_json::Number>>,
    /// The `hateAndDiscrimination` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    hate_and_discrimination: Option<Nullable<serde_json::Number>>,
    /// The `violenceAndThreats` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    violence_and_threats: Option<Nullable<serde_json::Number>>,
    /// The `dangerousAndCriminalContent` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    dangerous_and_criminal_content: Option<Nullable<serde_json::Number>>,
    /// The `selfharm` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    selfharm: Option<Nullable<serde_json::Number>>,
    /// The `health` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    health: Option<Nullable<serde_json::Number>>,
    /// The `financial` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    financial: Option<Nullable<serde_json::Number>>,
    /// The `law` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    law: Option<Nullable<serde_json::Number>>,
    /// The `pii` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pii: Option<Nullable<serde_json::Number>>,
}

/// Transport fields for `ModerationLlmv2CategoryThresholds`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
pub(super) struct ModerationLlmv2CategoryThresholds {
    /// The `sexual` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    sexual: Option<Nullable<serde_json::Number>>,
    /// The `hateAndDiscrimination` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    hate_and_discrimination: Option<Nullable<serde_json::Number>>,
    /// The `violenceAndThreats` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    violence_and_threats: Option<Nullable<serde_json::Number>>,
    /// The `dangerous` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    dangerous: Option<Nullable<serde_json::Number>>,
    /// The `criminal` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    criminal: Option<Nullable<serde_json::Number>>,
    /// The `selfharm` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    selfharm: Option<Nullable<serde_json::Number>>,
    /// The `health` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    health: Option<Nullable<serde_json::Number>>,
    /// The `financial` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    financial: Option<Nullable<serde_json::Number>>,
    /// The `law` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    law: Option<Nullable<serde_json::Number>>,
    /// The `pii` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    pii: Option<Nullable<serde_json::Number>>,
    /// The `jailbreaking` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    jailbreaking: Option<Nullable<serde_json::Number>>,
}

/// The declared field default.
fn moderation_llmv1_config_model_name_default() -> String {
    "mistral-moderation-2411".to_owned()
}

/// The declared field default.
fn moderation_llmv2_config_model_name_default() -> String {
    "mistral-moderation-2603".to_owned()
}

//! Reduced fixture schemas: all expected values are compared, without ignored metadata.
use crate::chat::TestResult;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One recorded query and its complete observation.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Exactly one behavior owner.
    test: String,
    /// Query selected by that owner.
    case: Query,
    /// All retained observations are compared.
    expected: Expected,
}
/// Selected model and history overrides are passed to their existing builders.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Query {
    /// Descriptor overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<Value>,
    /// Conversation overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    context: Option<Value>,
    /// Common and endpoint options read by the case runner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    options: Option<IndexMap<OptionField, Value>>,
    /// Simple entry selection; absence selects raw.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    entry: Option<String>,
    /// Controlled environment passed to an isolated child.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    env: Option<IndexMap<String, String>>,
    /// Scripted hook behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hook: Option<String>,
    /// Explicit missing API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    missing_key: Option<bool>,
    /// Cancellation phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cancel: Option<String>,
    /// Raw response bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    /// Events framed into a finite response body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    events: Option<Vec<Value>>,
    /// Byte fragmentation selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chunked: Option<bool>,
    /// Nonstream media type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
    /// Controlled HTTP status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
}
/// Recognized options are consumed according to the selected entry point.
#[derive(Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
enum OptionField {
    /// Explicit credential.
    ApiKey,
    /// Cache policy.
    CacheRetention,
    /// Explicit headers.
    Headers,
    /// Retry count.
    MaxRetries,
    /// Unsupported delay preference.
    MaxRetryDelayMs,
    /// Token limit, including tagged nonfinite inputs.
    MaxTokens,
    /// Unsupported metadata preference.
    Metadata,
    /// Simple reasoning request.
    Reasoning,
    /// Raw effort request.
    ReasoningEffort,
    /// Summary request.
    ReasoningSummary,
    /// Endpoint base URL.
    AzureBaseUrl,
    /// Resource shorthand.
    AzureResourceName,
    /// API version.
    AzureApiVersion,
    /// Deployment.
    AzureDeploymentName,
    /// Send timeout.
    TimeoutMs,
    /// Cache session.
    SessionId,
    /// Temperature, including tagged nonfinite inputs.
    Temperature,
    /// Unsupported transport preference.
    Transport,
}
/// Complete outcomes from either key rejection or a closed stream.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Expected {
    /// Key rejection before a stream exists.
    Early(Early),
    /// Captured transport, hooks, events and resulting message.
    Stream(Observed),
}
/// Early failure has no unobserved transport metadata.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Early {
    /// Exact authored failure.
    thrown: String,
}
/// Every nested JSON member participates in full observation equality.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observed {
    /// Whole captured requests.
    requests: Vec<Value>,
    /// Whole hook observations.
    hooks: Vec<Value>,
    /// Whole event observations without duplicated output handles.
    events: Vec<Value>,
    /// Final message without wall-clock timestamp.
    result: Value,
}
/// Decode without accepting unused wrapper or option fields.
pub fn rows(text: &str) -> TestResult<Vec<Value>> {
    serde_json::from_str::<Vec<Row>>(text)?
        .into_iter()
        .map(|row| Ok(serde_json::to_value(row)?))
        .collect()
}

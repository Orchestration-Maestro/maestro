//! Endpoint option vocabulary for the shared recorded response schema.
use crate::chat::TestResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
/// Decode without accepting unused wrapper or option fields.
pub fn rows(text: &str) -> TestResult<Vec<Value>> {
    crate::response_case_schema::rows::<OptionField>(text)
}

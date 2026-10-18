//! The cloud response fixture's accepted option names over the shared response fixture records.
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
/// Decode the cloud response fixture.
pub fn rows(text: &str) -> TestResult<Vec<Value>> {
    crate::response_cases::rows_with::<OptionField>(text)
}

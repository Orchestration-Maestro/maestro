//! Recorded fallback transactions through credential storage.
use maestro_credentials::{AuthSource, AuthStatus, AuthStorage, ConfigValueOperations};
use maestro_models::{DiagnosticErrorInfo, OAuthError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Recorded invocation and complete observable outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Public behavior group.
    pub test: String,
    /// Operands supplied by the caller.
    pub input: Input,
    /// Full observable outcome.
    pub expected: Value,
}
/// Credential operation inputs; absent members retain their recorded defaults.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub operation: Option<String>,
    pub fallback: String,
    pub earlier: Option<String>,
    pub provider: Option<String>,
    pub runtime: Option<String>,
    pub stored: Option<Value>,
    pub include_fallback: Option<bool>,
    pub phase: Option<String>,
}
/// Select all rows belonging to one independently named test.
pub fn cases(test: &str) -> Vec<Case> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/fallback_failures.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|case| case.test == test).collect();
    assert!(!rows.is_empty());
    rows
}
mod errors;
pub use errors::{error_value, failure};
/// Secret-free status shape as observed by the recorder.
fn status_value(status: AuthStatus) -> Value {
    let mut value = json!({"configured": status.configured});
    if let Some(source) = status.source {
        value["source"] = match source {
            AuthSource::Stored => "stored",
            AuthSource::Runtime => "runtime",
            AuthSource::Environment => "environment",
            AuthSource::Fallback => "fallback",
            AuthSource::ModelsJsonKey | AuthSource::ModelsJsonCommand => unreachable!(),
        }
        .into();
    }
    if let Some(label) = status.label {
        value["label"] = label.into();
    }
    value
}
/// Environment-free stored-value operations.
struct NoOperations;
impl ConfigValueOperations for NoOperations {
    fn environment(&self, _: &str) -> Option<String> {
        None
    }
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}
/// Install a retained callback and retain its exact ordered invocations.
pub fn install(storage: &AuthStorage, fallback: &str) -> Arc<Mutex<Vec<String>>> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    if fallback != "absent" {
        let seen = calls.clone();
        let fallback = fallback.to_owned();
        storage.set_fallback_resolver(move |provider| {
            seen.lock().unwrap().push(provider.to_owned());
            match fallback.as_str() {
                "error" => Err(failure()),
                "none" => Ok(None),
                key => Ok(Some(key.to_owned())),
            }
        });
    }
    calls
}
/// Drive a recorded key, presence, status or fallback-option operation.
pub async fn drive(case: Case) {
    let input = case.input;
    assert!(input.phase.is_none());
    let provider = input
        .provider
        .as_deref()
        .unwrap_or("maestro-custom-fallback");
    let storage = AuthStorage::in_memory(std::iter::empty().collect());
    if let Some(record) = input.stored {
        storage.set(provider, serde_json::from_value(record).unwrap());
    }
    if let Some(key) = input.runtime {
        storage.set_runtime_api_key(provider, &key);
    }
    if let Some(earlier) = &input.earlier {
        assert!(
            earlier.starts_with("environment")
                || earlier.starts_with("stored")
                || earlier.starts_with("runtime")
        );
    }
    let calls = install(&storage, &input.fallback);
    let result = match input.operation.as_deref() {
        Some("hasAuth") => storage.has_auth(provider).map(Value::Bool),
        Some("getAuthStatus") => storage.get_auth_status(provider).map(status_value),
        Some("getApiKey") | None => storage
            .get_api_key(
                provider,
                input.include_fallback.unwrap_or(true),
                &NoOperations,
            )
            .await
            .map(Into::into),
        _ => unreachable!(),
    };
    let calls = calls.lock().unwrap().clone();
    let actual = match result {
        Ok(result) => {
            let value = if input.operation.is_some() {
                let mut value = json!({"result": result, "calls": calls});
                if input.earlier.is_none() {
                    value["drained"] = storage.drain_errors().len().into();
                }
                value
            } else {
                result
            };
            json!({"branch": "ok", "value": value, "calls": calls})
        }
        Err(error) => json!({"branch":"error", "error":error_value(&error), "calls":calls}),
    };
    assert!(storage.drain_errors().is_empty());
    assert!(
        actual == case.expected,
        "recorded credential outcome: {}",
        case.test
    );
}

/// Select each environment row in its matching child, and other rows in one child only.
fn applicable(case: &Case) -> bool {
    let scenario = std::env::var("MAESTRO_FALLBACK_ENV").unwrap_or_default();
    match case
        .input
        .earlier
        .as_deref()
        .filter(|earlier| earlier.starts_with("environment"))
    {
        Some(earlier) => scenario == earlier,
        None => scenario.is_empty() || scenario == "environment",
    }
}
/// Replay the group's disjoint rows using the current isolated environment policy.
pub async fn replay(name: &str) {
    for case in cases(name).into_iter().filter(applicable) {
        drive(case).await;
    }
}

//! Recorded fallback transactions through credential storage.
use crate::{AuthSource, AuthStatus, AuthStorage, ConfigValueOperations};
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
    /// Recorded operation input.
    pub operation: Option<String>,
    /// Recorded fallback input.
    pub fallback: String,
    /// Recorded earlier input.
    pub earlier: Option<String>,
    /// Recorded provider input.
    pub provider: Option<String>,
    /// Recorded runtime input.
    pub runtime: Option<String>,
    /// Recorded stored input.
    pub stored: Option<Value>,
    /// Recorded include fallback input.
    pub include_fallback: Option<bool>,
    /// Recorded phase input.
    pub phase: Option<String>,
}
/// Select all rows belonging to one independently named test.
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
///
/// # Panics
/// Panics if no recorded row belongs to the supplied test.
pub fn cases(test: &str) -> Result<Vec<Case>, serde_json::Error> {
    let rows: Vec<Case> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/fallback_failures.json"
    ))?;
    let rows: Vec<_> = rows.into_iter().filter(|case| case.test == test).collect();
    assert!(!rows.is_empty());
    Ok(rows)
}
pub mod errors;
pub use errors::{error_value, failure};
/// Secret-free status shape as observed by the recorder.
fn status_value(status: AuthStatus) -> Result<Value, Box<dyn std::error::Error>> {
    let mut value = json!({"configured": status.configured});
    if let Some(source) = status.source {
        value["source"] = match source {
            AuthSource::Stored => "stored",
            AuthSource::Runtime => "runtime",
            AuthSource::Environment => "environment",
            AuthSource::Fallback => "fallback",
            AuthSource::ModelsJsonKey | AuthSource::ModelsJsonCommand => {
                return Err("unexpected model credential status".into());
            }
        }
        .into();
    }
    if let Some(label) = status.label {
        value["label"] = label.into();
    }
    Ok(value)
}
/// Stored-value operations that resolve no command.
struct NoOperations;
impl ConfigValueOperations for NoOperations {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}
/// Install a retained callback and retain its exact ordered invocations.
///
/// # Panics
/// Panics synchronously if the callback transcript lock is poisoned.
pub fn install(storage: &AuthStorage, fallback: &str) -> Arc<Mutex<Vec<String>>> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    if fallback != "absent" {
        let seen = calls.clone();
        let fallback = fallback.to_owned();
        storage.set_fallback_resolver(move |provider| {
            let recorded = seen.lock().map(|mut seen| seen.push(provider.to_owned()));
            assert!(recorded.is_ok());
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
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
///
/// # Panics
/// Panics if the recorded transaction or its observed outcome differs.
pub async fn drive(case: Case) -> Result<(), Box<dyn std::error::Error>> {
    let input = case.input;
    assert!(input.phase.is_none());
    let provider = input
        .provider
        .as_deref()
        .unwrap_or("maestro-custom-fallback");
    let storage = AuthStorage::in_memory(std::iter::empty().collect());
    if let Some(record) = input.stored {
        storage.set(provider, serde_json::from_value(record)?);
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
        Some("getAuthStatus") => match storage.get_auth_status(provider) {
            Ok(status) => Ok(status_value(status)?),
            Err(error) => Err(error),
        },
        Some("getApiKey") | None => storage
            .get_api_key(
                provider,
                input.include_fallback.unwrap_or(true),
                &NoOperations,
            )
            .await
            .map(Into::into),
        _ => return Err("unknown recorded credential operation".into()),
    };
    let calls = calls
        .lock()
        .map_err(|_| "callback transcript poisoned")?
        .clone();
    let actual = outcome(
        result,
        &calls,
        &storage,
        input.operation.as_deref(),
        input.earlier.as_deref(),
    )?;
    assert!(storage.drain_errors().is_empty());
    assert!(
        actual == case.expected,
        "recorded credential outcome: {}",
        case.test
    );
    Ok(())
}

/// Select each environment row in its matching `scenario`, and other rows in one child only.
fn applicable(case: &Case, scenario: &str) -> bool {
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
/// Replay the group's disjoint rows for `scenario` using the current isolated environment
/// policy.
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
pub async fn replay(name: &str, scenario: &str) -> Result<(), Box<dyn std::error::Error>> {
    for case in cases(name)?
        .into_iter()
        .filter(|case| applicable(case, scenario))
    {
        drive(case).await?;
    }
    Ok(())
}

/// Observe the complete operation result after its callback transcript settles.
fn outcome(
    result: Result<Value, OAuthError>,
    calls: &[String],
    storage: &AuthStorage,
    operation: Option<&str>,
    earlier: Option<&str>,
) -> Result<Value, serde_json::Error> {
    Ok(match result {
        Ok(result) => {
            let value = if operation.is_some() {
                let mut value = json!({"result": result, "calls": calls});
                if earlier.is_none() {
                    value["drained"] = storage.drain_errors().len().into();
                }
                value
            } else {
                result
            };
            json!({"branch": "ok", "value": value, "calls": calls})
        }
        Err(error) => json!({"branch":"error", "error":error_value(&error)?, "calls":calls}),
    })
}

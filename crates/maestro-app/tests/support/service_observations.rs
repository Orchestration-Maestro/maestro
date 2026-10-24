//! Controlled service transport and consumed fixture pairs.
#![cfg(test)]
use futures_util::stream;
use maestro_models::{Fetch, FetchError, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// One consumed input and its independently recorded output.
pub struct Case {
    /// Operation-specific input, including arbitrary server JSON.
    pub input: Value,
    /// Expected observations from the operation.
    pub output: Value,
}

/// Load consumed input/output pairs from a service fixture group.
///
/// # Panics
/// Panics when fixture data or controlled observations violate the test contract.
#[must_use]
pub fn cases(group: &str) -> Vec<Case> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/services")
        .join(format!("{group}.json"));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Project only the native target spelling in recorded request headers.
///
/// # Panics
/// Panics when fixture data or controlled observations violate the test contract.
#[must_use]
pub fn expected_calls(output: &Value) -> Value {
    let mut calls = output["calls"].clone();
    for call in calls.as_array_mut().unwrap() {
        let platform = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            p => p,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "ia32",
            "aarch64" => "arm64",
            a => a,
        };
        let ua = call["headers"]["user-agent"]
            .as_str()
            .unwrap()
            .replace("(linux; rust; x64)", &format!("({platform}; rust; {arch})"));
        call["headers"]["user-agent"] = json!(ua);
    }
    calls
}

/// HTTP attempt recorder with a controlled response phase.
pub struct Transport {
    /// Replaceable shared HTTP operation.
    pub fetch: Fetch,
    /// Requests admitted by the controlled operation.
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    /// Number of times a body producer was polled.
    pub body_polls: Arc<AtomicUsize>,
    /// Number of released response producers.
    pub dropped: Arc<AtomicUsize>,
    /// Permission to complete delayed headers.
    pub headers: Arc<tokio::sync::Notify>,
}
/// A response producer whose release is observable.
struct Release(Arc<AtomicUsize>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl Transport {
    /// Create a transport with the selected response status, body and failure phase.
    ///
    /// # Panics
    /// Panics when fixture data or controlled observations violate the test contract.
    #[must_use]
    pub fn new(status: u16, body: String, mode: &str) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let body_polls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let (seen, polls, drops) = (
            Arc::clone(&requests),
            Arc::clone(&body_polls),
            Arc::clone(&dropped),
        );
        let headers = Arc::new(tokio::sync::Notify::new());
        let ready = Arc::clone(&headers);
        let mode = mode.to_owned();
        let fetch: Fetch = Arc::new(move |request| {
            seen.lock().unwrap().push(request);
            Box::pin(response(
                status,
                body.clone(),
                mode.clone(),
                ResponseState {
                    polls: Arc::clone(&polls),
                    drops: Arc::clone(&drops),
                    ready: Arc::clone(&ready),
                },
            ))
        });
        Self {
            fetch,
            requests,
            body_polls,
            dropped,
            headers,
        }
    }
    /// Take the completed request transcript in invocation order.
    ///
    /// # Panics
    /// Panics when fixture data or controlled observations violate the test contract.
    #[must_use]
    pub fn calls(&self) -> Value {
        let requests = std::mem::take(&mut *self.requests.lock().unwrap());
        json!(
            requests
                .iter()
                .map(|r| {
                    assert_eq!(r.method, "GET");
                    assert!(r.body.is_empty());
                    assert!(r.signal.is_none());
                    json!({"url": r.url, "headers": r.headers})
                })
                .collect::<Vec<_>>()
        )
    }
    /// Count requests recorded since the last transcript was taken.
    ///
    /// # Panics
    /// Panics when fixture data or controlled observations violate the test contract.
    #[must_use]
    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
/// Observe selected optional fields in the recorded response format.
#[must_use]
pub fn release_value(
    release: Option<maestro_app::configuration::version_check::LatestMaestroRelease>,
) -> Value {
    release.map_or(Value::Null, |r| {
        let mut value = json!({"version": r.version});
        if let Some(package) = r.package_name {
            value["packageName"] = json!(package);
        }
        value
    })
}
/// Recorded controlled origin, never contacted by the fake transport.
pub const ORIGIN: &str = "https://controlled.invalid:9443";
/// Ambient policy keys explicitly set or removed in each child.
pub const FLAGS: &[&str] = &[
    "MAESTRO_TELEMETRY",
    "MAESTRO_OFFLINE",
    "MAESTRO_SKIP_VERSION_CHECK",
];
#[cfg(unix)]
use crate::child;
#[cfg(unix)]
/// Run the named test with all service flags explicitly absent.
///
/// # Panics
/// Panics when fixture data or controlled observations violate the test contract.
#[must_use]
pub fn isolated(name: &str) -> bool {
    if maestro_test_process::selection().is_child()
        && maestro_test_process::selection().runs_case(name)
    {
        return false;
    }
    child::probe(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case(name),
        &std::env::current_dir().unwrap(),
        &[],
        FLAGS,
    )
    .expect("child starts");
    true
}
#[cfg(unix)]
/// Run selected fixture rows in children with their own observed environment.
///
/// # Panics
/// Panics when fixture data or controlled observations violate the test contract.
#[must_use]
pub fn environment_cases(name: &str, group: &str, indices: &[usize]) -> Option<Case> {
    let cases = cases(group);
    if maestro_test_process::selection().is_child() {
        return Some(
            cases
                .into_iter()
                .enumerate()
                .find(|(index, _)| maestro_test_process::selection().runs_case(&index.to_string()))
                .expect("selected service row")
                .1,
        );
    }
    for &index in indices {
        let case = &cases[index];
        let index = index.to_string();
        let mut envs = Vec::new();
        if let Some(flag) = case.input["flag"].as_str()
            && let Some(value) = case.input["value"].as_str()
        {
            envs.push((flag, value));
        }
        if let Some(env) = case.input["env"].as_object() {
            for (key, value) in env {
                envs.push((key.as_str(), value.as_str().unwrap()));
            }
        }
        child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .case(&index),
            &std::env::current_dir().unwrap(),
            &envs,
            FLAGS,
        )
        .expect("child starts");
    }
    None
}
#[cfg(unix)]
/// Reuse the native child probe with explicit environment additions and removals.
///
/// # Panics
/// Panics when fixture data or controlled observations violate the test contract.
pub fn native_probe(name: &str, envs: &[(&str, &str)], removed: &[&str]) {
    child::probe(
        maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case(name),
        &std::env::current_dir().unwrap(),
        envs,
        removed,
    )
    .expect("child starts");
}

/// Owned observations and the delayed-header completion signal.
struct ResponseState {
    polls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    ready: Arc<tokio::sync::Notify>,
}
/// Produce headers and a separately controlled body.
async fn response(
    status: u16,
    body: String,
    mode: String,
    state: ResponseState,
) -> Result<HttpResponse, FetchError> {
    let guard = Release(state.drops);
    if mode == "FETCH_ERROR" {
        return Err(FetchError::Aborted);
    }
    if mode == "HEADERS_PENDING" {
        std::future::pending::<()>().await;
    }
    if mode == "HEADERS_DELAYED" {
        state.ready.notified().await;
    }
    Ok(HttpResponse {
        status,
        status_text: String::new(),
        headers: BTreeMap::new(),
        body: Box::pin(stream::once(async move {
            let _guard = guard;
            state.polls.fetch_add(1, Ordering::SeqCst);
            if matches!(mode.as_str(), "BODY_PENDING" | "HEADERS_DELAYED") {
                std::future::pending::<()>().await;
            }
            if mode == "BODY_ERROR" {
                Err(FetchError::Aborted)
            } else {
                Ok(body.into_bytes())
            }
        })),
    })
}
pub(crate) use crate::support::run_io as run;

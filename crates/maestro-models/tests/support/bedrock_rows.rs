//! Request rows: a hook edit applied to the conversation input, and what the SDK then sent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use maestro_models::providers::cloud::amazon_bedrock::{BedrockOptions, stream_bedrock};
use maestro_models::{AssistantMessage, DiagnosticErrorInfo, StreamOptions};
use serde::Deserialize;
use serde_json::{Map, Value};
use url::Url;

use crate::bedrock;
use crate::chat::TestResult;
use crate::transport;

const FIXTURE: &str = include_str!("../fixtures/cloud_conversation_requests.json");
const ENDPOINT: &str = "https://bedrock-runtime.eu-central-1.amazonaws.com";

/// A step of one request, in the order the conversation reaches it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// The payload hook ran.
    Hook,
    /// The sender received a request.
    Transport,
    /// The response hook ran.
    Response,
}

/// How a row changes the request or its setup.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Change {
    /// Members written over the hook input.
    patch: Option<Map<String, Value>>,
    /// The conversation to ask about instead of the default one.
    context: Option<Value>,
    /// Members removed from the hook input.
    #[serde(default)]
    remove: Vec<String>,
    /// Bearer token given in the options.
    bearer_token: Option<String>,
    /// Failure the payload hook reports.
    hook_error: Option<String>,
    /// Input returned in place of the whole hook input.
    replacement: Option<Value>,
    /// Tags given in the options.
    request_metadata: Option<IndexMap<String, String>>,
    /// Endpoint of the model.
    base_url: Option<String>,
}

/// What a row expects to observe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Steps reached, in order.
    sequence: Vec<Phase>,
    /// Input the payload hook received.
    hook: Option<Value>,
    /// Request path as sent.
    path: Option<String>,
    /// Request body as sent.
    body: Option<Value>,
    /// Stop reason of a failed request.
    stop_reason: Option<String>,
    /// Failure text of a failed request.
    error: Option<String>,
    /// Start of the Authorization header.
    authorization: Option<String>,
}

/// One request row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Name for diagnostics.
    pub id: String,
    /// Test that owns the row.
    pub test: String,
    /// Change applied.
    pub change: Change,
    /// Environment the request runs under; empty means the request needs none.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Observations expected.
    expected: Expected,
}

/// The rows a test owns.
pub fn rows(test: &str) -> TestResult<Vec<Row>> {
    let all: Vec<Row> = serde_json::from_str(FIXTURE)?;
    Ok(all.into_iter().filter(|row| row.test == test).collect())
}

/// What one run observed.
struct Observed {
    phases: Vec<Phase>,
    hook: Option<Value>,
    request: Option<maestro_models::HttpRequest>,
    message: AssistantMessage,
}

/// The payload hook of a row: record the input, then return what the change makes of it.
fn payload_hook(
    change: &Change,
    phases: &Arc<Mutex<Vec<Phase>>>,
    seen: &Arc<Mutex<Option<Value>>>,
) -> maestro_models::OnPayload {
    let (phases, seen) = (Arc::clone(phases), Arc::clone(seen));
    let hook_error = change.hook_error.clone();
    let replacement = change.replacement.clone();
    let patch = change.patch.clone().unwrap_or_default();
    let remove = change.remove.clone();
    Arc::new(move |mut input, _| {
        if let Ok(mut phases) = phases.lock() {
            phases.push(Phase::Hook);
        }
        if let Ok(mut seen) = seen.lock() {
            *seen = Some(input.clone());
        }
        let outcome = match (&hook_error, &replacement) {
            (Some(message), _) => Err(DiagnosticErrorInfo {
                name: None,
                message: message.clone(),
                stack: None,
                code: None,
            }),
            (None, Some(replacement)) => Ok(replacement.clone()),
            (None, None) => {
                edit(&mut input, &remove, &patch);
                Ok(input)
            }
        };
        Box::pin(std::future::ready(outcome))
    })
}

/// Remove the named members from a hook input, then write the patch over it.
fn edit(input: &mut Value, remove: &[String], patch: &Map<String, Value>) {
    if let Some(members) = input.as_object_mut() {
        for name in remove {
            members.shift_remove(name);
        }
        members.extend(patch.clone());
    }
}

/// Run a row's request and record what happened.
async fn observe(row: &Row) -> TestResult<Observed> {
    let phases = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(Mutex::new(None));
    let sent = Arc::clone(&phases);
    let answer = bedrock::answer("answer")?;
    let scripted = bedrock::scripted(move |_| {
        if let Ok(mut phases) = sent.lock() {
            phases.push(Phase::Transport);
        }
        let headers = [("x-amzn-requestid", "fixture-id")];
        Box::pin(std::future::ready(Ok(bedrock::response(
            200,
            &headers,
            answer.clone(),
        ))))
    });
    let responded = Arc::clone(&phases);
    let mut common = StreamOptions {
        fetch: Some(scripted.fetch.clone()),
        on_payload: Some(payload_hook(&row.change, &phases, &seen)),
        ..StreamOptions::default()
    };
    common.on_response = Some(Arc::new(move |_, _| {
        if let Ok(mut phases) = responded.lock() {
            phases.push(Phase::Response);
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    let options = BedrockOptions {
        common,
        bearer_token: row
            .change
            .bearer_token
            .clone()
            .or_else(|| row.env.is_empty().then(|| "fixture-token".to_owned())),
        request_metadata: row.change.request_metadata.clone(),
        ..BedrockOptions::default()
    };
    let context = match &row.change.context {
        Some(context) => serde_json::from_value(context.clone())?,
        None => bedrock::context("question")?,
    };
    let base_url = row.change.base_url.as_deref().unwrap_or(ENDPOINT);
    let stream = stream_bedrock(bedrock::model(base_url)?, context, Some(options));
    let (_, message) = transport::drain(&stream).await?;
    let phases = phases.lock().map_err(|e| e.to_string())?.clone();
    let hook = seen.lock().map_err(|e| e.to_string())?.clone();
    Ok(Observed {
        phases,
        hook,
        request: scripted.seen()?.into_iter().next(),
        message,
    })
}

/// Run a row and check every member it expects.
pub async fn check(row: &Row) -> TestResult {
    let observed = observe(row).await?;
    let id = &row.id;
    let expected = &row.expected;
    assert_eq!(observed.phases, expected.sequence, "{id}: steps");
    if let Some(hook) = &expected.hook {
        assert_eq!(observed.hook.as_ref(), Some(hook), "{id}: hook input");
    }
    match &observed.request {
        Some(request) => check_sent(row, request, &observed.message),
        None => check_failed(row, &observed.message),
    }
}

/// Check the request the SDK sent for a row.
fn check_sent(
    row: &Row,
    request: &maestro_models::HttpRequest,
    message: &AssistantMessage,
) -> TestResult {
    let id = &row.id;
    let expected = &row.expected;
    assert_eq!(
        Some(Url::parse(&request.url)?.path()),
        expected.path.as_deref(),
        "{id}: path"
    );
    let body: Value = serde_json::from_slice(&request.body)?;
    assert_eq!(Some(&body), expected.body.as_ref(), "{id}: body");
    let authorization = request.headers.get("authorization").map(String::as_str);
    match &expected.authorization {
        Some(prefix) => assert!(
            authorization.is_some_and(|value| value.starts_with(prefix)),
            "{id}: authorization"
        ),
        None => assert!(authorization.is_some(), "{id}: unsigned"),
    }
    assert_eq!(message.error_message, None, "{id}: unexpected failure");
    Ok(())
}

/// Check the failure a row ended with before anything was sent.
fn check_failed(row: &Row, message: &AssistantMessage) -> TestResult {
    let id = &row.id;
    let expected = &row.expected;
    let reason = serde_json::to_value(message)?["stopReason"].clone();
    assert_eq!(
        reason.as_str(),
        expected.stop_reason.as_deref(),
        "{id}: stop reason"
    );
    match &expected.error {
        Some(error) => assert_eq!(message.error_message.as_ref(), Some(error), "{id}: error"),
        None => assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| !text.is_empty()),
            "{id}: error text"
        ),
    }
    Ok(())
}

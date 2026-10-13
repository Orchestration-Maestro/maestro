//! Request rows: a hook edit applied to the conversation input, and what the SDK then sent.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use maestro_models::providers::cloud::amazon_bedrock::{stream_bedrock, stream_simple_bedrock};
use maestro_models::{
    AssistantMessage, AssistantMessageEventStream, BedrockOptions, BedrockThinkingDisplay,
    CacheRetention, DiagnosticErrorInfo, SimpleStreamOptions, StreamOptions, ThinkingBudgets,
    ThinkingLevel, ToolChoice,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use url::Url;

use crate::bedrock;
use crate::chat::{TestResult, block_on};
use crate::child_process;
use crate::json::canonical;
use crate::transport;

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

/// A tool choice spelled as the service does.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum ChoiceRow {
    /// `auto`, `any` or `none`.
    Mode(String),
    /// `{"type": "tool", "name": ...}`.
    Tool {
        /// Always `tool`.
        r#type: String,
        /// Tool name.
        name: String,
    },
}

/// Options a row gives the call, in the spellings of the fixture.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptionsRow {
    /// Cache lifetime selection.
    cache_retention: Option<CacheRetention>,
    /// Requested thinking level.
    reasoning: Option<ThinkingLevel>,
    /// Thinking display selection.
    thinking_display: Option<BedrockThinkingDisplay>,
    /// Per-level thinking token budgets.
    thinking_budgets: Option<ThinkingBudgets>,
    /// Tool selection policy.
    tool_choice: Option<ChoiceRow>,
    /// Fixed-budget interleaving override.
    interleaved_thinking: Option<bool>,
    /// Requested output token limit.
    max_tokens: Option<f64>,
    /// Sampling temperature.
    temperature: Option<f64>,
    /// Configured service region.
    region: Option<String>,
    /// Common session identifier.
    session_id: Option<String>,
    /// Common metadata.
    metadata: Option<Map<String, Value>>,
}

impl OptionsRow {
    /// The common options the row names.
    fn common(&self, common: StreamOptions) -> StreamOptions {
        StreamOptions {
            cache_retention: self.cache_retention,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            session_id: self.session_id.clone(),
            metadata: self.metadata.clone(),
            ..common
        }
    }

    /// The choice as the option type holds it.
    fn choice(&self) -> TestResult<Option<ToolChoice>> {
        Ok(match &self.tool_choice {
            None => None,
            Some(ChoiceRow::Mode(mode)) => Some(match mode.as_str() {
                "auto" => ToolChoice::Auto,
                "any" => ToolChoice::Required,
                "none" => ToolChoice::None,
                other => return Err(format!("unknown tool choice {other}").into()),
            }),
            Some(ChoiceRow::Tool { r#type, name }) => {
                assert_eq!(r#type, "tool");
                Some(ToolChoice::Function { name: name.clone() })
            }
        })
    }
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
    /// Members written over the fixture's base model.
    model: Option<Map<String, Value>>,
    /// Options given to the call.
    #[serde(default)]
    options: OptionsRow,
    /// Entry to call: `simple` for the simple entry, otherwise the direct one.
    entry: Option<String>,
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
    /// Native failure fragment.
    error_contains: Option<String>,
    /// Object key sequences indexed by JSON Pointer.
    body_keys: Option<IndexMap<String, Vec<String>>>,
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
    /// Row-specific variables for the child; an empty map declares none.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Observations expected.
    expected: Expected,
    /// Model of the fixture the row belongs to.
    #[serde(skip)]
    base_model: Option<Value>,
    /// Conversation of the fixture the row belongs to.
    #[serde(skip)]
    base_context: Option<Value>,
}

/// A fixture: bare rows, or rows with the model and conversation they start from.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Fixture {
    /// Rows that bring their own setup.
    Rows(Vec<Row>),
    /// Rows that change a shared model and conversation.
    Based {
        /// Shared model descriptor.
        model: Value,
        /// Shared conversation.
        context: Value,
        /// Owned test rows.
        rows: Vec<Row>,
    },
}

/// The rows of `fixture` that `test` owns.
fn rows(fixture: &str, test: &str) -> TestResult<Vec<Row>> {
    let (base, all) = match serde_json::from_str(fixture)? {
        Fixture::Rows(rows) => (None, rows),
        Fixture::Based {
            model,
            context,
            rows,
        } => (Some((model, context)), rows),
    };
    Ok(all
        .into_iter()
        .filter(|row| row.test == test)
        .map(|mut row| {
            if let Some((model, context)) = &base {
                row.base_model = Some(model.clone());
                row.base_context = Some(context.clone());
            }
            row
        })
        .collect())
}

/// Run every row `test` owns in `fixture` in an isolated child with its row-specific variables.
pub fn check_rows(fixture: &str, test: &str) -> TestResult {
    let owned = rows(fixture, test)?;
    assert!(!owned.is_empty(), "{test} owns no rows");
    if let Some(case) = child_process::child_case() {
        let row = owned
            .iter()
            .find(|row| row.id == case)
            .ok_or("unknown row")?;
        return block_on(true, check(row));
    }
    for row in &owned {
        let env = environment(row);
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, *v)).collect();
        child_process::rerun(test, &row.id, &env)?;
    }
    Ok(())
}

/// The variables a row's child runs with. The simple entry takes no token option, so its rows
/// get the token through the environment.
fn environment(row: &Row) -> BTreeMap<&str, &str> {
    let mut env: BTreeMap<&str, &str> = row
        .env
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    if row.change.entry.is_some() {
        env.insert("AWS_BEARER_TOKEN_BEDROCK", "fixture-token");
    }
    env
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
    let scripted = transport::transport(vec![transport::Attempt::streaming_with_headers(
        200,
        &[("x-amzn-requestid", "fixture-id")],
        move || {
            if let Ok(mut phases) = sent.lock() {
                phases.push(Phase::Transport);
            }
            Box::pin(futures_util::stream::iter(
                answer.clone().into_iter().map(Ok),
            ))
        },
    )]);
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
    let stream = start(row, common)?;
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

/// Build the fixture's model and options, then invoke its selected entry.
fn start(row: &Row, common: StreamOptions) -> TestResult<AssistantMessageEventStream> {
    let change = &row.change;
    let common = change.options.common(common);
    let bearer_token = change
        .bearer_token
        .clone()
        .or_else(|| row.env.is_empty().then(|| "fixture-token".to_owned()));
    let context = match change.context.as_ref().or(row.base_context.as_ref()) {
        Some(context) => serde_json::from_value(context.clone())?,
        None => bedrock::context("question")?,
    };
    let base_url = change.base_url.as_deref().unwrap_or(ENDPOINT);
    let model = match &row.base_model {
        Some(base) => {
            let mut model = base.clone();
            if let (Some(members), Some(changes)) = (model.as_object_mut(), &change.model) {
                members.extend(changes.clone());
            }
            serde_json::from_value(model)?
        }
        None => bedrock::model(base_url)?,
    };
    Ok(if change.entry.as_deref() == Some("simple") {
        let simple = SimpleStreamOptions {
            common: StreamOptions {
                api_key: None,
                ..common
            },
            reasoning: change.options.reasoning,
            thinking_budgets: change.options.thinking_budgets.clone(),
            ..SimpleStreamOptions::default()
        };
        stream_simple_bedrock(model, context, Some(simple))
    } else {
        let options = BedrockOptions {
            common,
            region: change.options.region.clone(),
            bearer_token,
            request_metadata: change.request_metadata.clone(),
            tool_choice: change.options.choice()?,
            reasoning: change.options.reasoning,
            thinking_budgets: change.options.thinking_budgets.clone(),
            interleaved_thinking: change.options.interleaved_thinking,
            thinking_display: change.options.thinking_display,
            ..BedrockOptions::default()
        };
        stream_bedrock(model, context, Some(options))
    })
}

/// Run a row and check every member it expects.
pub async fn check(row: &Row) -> TestResult {
    let observed = observe(row).await?;
    let id = &row.id;
    let expected = &row.expected;
    if let Some(hook) = &expected.hook {
        assert_eq!(
            observed.hook.clone().map(canonical).as_ref(),
            Some(hook),
            "{id}: hook input"
        );
    }
    assert_eq!(observed.phases, expected.sequence, "{id}: steps");
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
    if let Some(path) = &expected.path {
        assert_eq!(Url::parse(&request.url)?.path(), path, "{id}: path");
    }
    let body: Value = serde_json::from_slice(&request.body)?;
    assert_eq!(Some(&body), expected.body.as_ref(), "{id}: body");
    if let Some(objects) = &expected.body_keys {
        for (pointer, keys) in objects {
            let object = body
                .pointer(pointer)
                .and_then(Value::as_object)
                .ok_or("expected an object at the key-order pointer")?;
            assert_eq!(
                object.keys().collect::<Vec<_>>(),
                keys.iter().collect::<Vec<_>>(),
                "{id}: body keys"
            );
        }
    }
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
    if let Some(fragment) = &expected.error_contains {
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|text| text.contains(fragment)),
            "{id}: error fragment"
        );
    }
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

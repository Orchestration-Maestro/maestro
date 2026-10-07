use crate::*;
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};
mod host;

/// Content accepted by the assistant builder.
#[derive(Clone)]
pub enum FauxAssistantContent {
    /// One text block.
    Text(String),
    /// A single block.
    Block(FauxContentBlock),
    /// Blocks in content order.
    Blocks(Vec<FauxContentBlock>),
}
/// The existing assistant content union.
pub type FauxContentBlock = AssistantContent;
/// Optional tool-call identifier.
#[derive(Clone, Default)]
pub struct FauxToolCallOptions {
    /// Explicit identifier, including an empty identifier.
    pub id: Option<String>,
}
/// Build an unsigned text block.
pub fn faux_text(text: String) -> TextContent {
    TextContent {
        text,
        text_signature: None,
    }
}
/// Build an unsigned thinking block.
pub fn faux_thinking(thinking: String) -> ThinkingContent {
    ThinkingContent {
        thinking,
        thinking_signature: None,
        redacted: None,
    }
}
/// Build a call with a supplied or generated identifier.
pub fn faux_tool_call(
    name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
    options: FauxToolCallOptions,
) -> ToolCall {
    ToolCall {
        id: options
            .id
            .unwrap_or_else(|| random_id("tool", host::production().as_ref())),
        name,
        arguments,
        thought_signature: None,
    }
}
/// Optional assistant metadata.
#[derive(Clone, Default)]
pub struct FauxAssistantMessageOptions {
    /// Supplied completion reason.
    pub stop_reason: Option<StopReason>,
    /// Supplied failure text.
    pub error_message: Option<String>,
    /// Supplied response identifier.
    pub response_id: Option<String>,
    /// Supplied Unix milliseconds.
    pub timestamp: Option<f64>,
}
/// Configuration of an opt-in simulator.
#[derive(Clone, Default)]
pub struct RegisterFauxProviderOptions {
    /// Exact protocol identifier; absent generates one.
    pub api: Option<String>,
    /// Provider identity.
    pub provider: Option<String>,
    /// Ordered model definitions.
    pub models: Option<Vec<FauxModelDefinition>>,
    /// Estimated output tokens per second; absent yields without a pacing delay.
    pub tokens_per_second: Option<f64>,
    /// Random chunk-size bounds in estimated tokens.
    pub token_size: Option<FauxTokenSize>,
}
/// Optional numeric chunk-size bounds.
#[derive(Clone, Default)]
pub struct FauxTokenSize {
    /// Minimum estimated tokens.
    pub min: Option<f64>,
    /// Maximum estimated tokens.
    pub max: Option<f64>,
}
/// Per-model overrides, with no additional admission validation.
#[derive(Clone)]
pub struct FauxModelDefinition {
    /// Model identifier.
    pub id: String,
    /// Display name; absent uses the identifier.
    pub name: Option<String>,
    /// Reasoning capability.
    pub reasoning: Option<bool>,
    /// Accepted input kinds.
    pub input: Option<Vec<String>>,
    /// Token prices, ignored by simulated usage.
    pub cost: Option<TokenRates>,
    /// Context window.
    pub context_window: Option<f64>,
    /// Output limit.
    pub max_tokens: Option<f64>,
}
/// Request-observing asynchronous response factory.
#[cfg(not(target_arch = "wasm32"))]
pub type FauxResponseFactory = Arc<
    dyn Fn(
            Context,
            Option<ProviderStreamOptions>,
            Arc<RwLock<FauxProviderState>>,
            Model,
        ) -> Pin<Box<dyn Future<Output = Result<AssistantMessage, ThrownValue>> + Send>>
        + Send
        + Sync,
>;
/// Request-observing asynchronous response factory.
#[cfg(target_arch = "wasm32")]
pub type FauxResponseFactory = Arc<
    dyn Fn(
        Context,
        Option<ProviderStreamOptions>,
        Arc<RwLock<FauxProviderState>>,
        Model,
    ) -> Pin<Box<dyn Future<Output = Result<AssistantMessage, ThrownValue>>>>,
>;
/// Live request counter shared with factories.
#[derive(Clone, Default)]
pub struct FauxProviderState {
    /// Number of invocations, including exhaustion and cancellation.
    pub call_count: f64,
}
/// A queued response.
#[derive(Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "Queued messages are owned records, while factories are shared handles"
)]
pub enum FauxResponseStep {
    /// Supplied assistant response.
    Message(AssistantMessage),
    /// Request-observing factory.
    Factory(FauxResponseFactory),
}
/// Registered models with a shared response queue.
#[derive(Clone)]
pub struct FauxProviderRegistration {
    /// Registered protocol identifier.
    pub api: String,
    /// Models in definition order.
    pub models: Vec<Model>,
    /// Live request counter.
    pub state: Arc<RwLock<FauxProviderState>>,
    queue: Arc<Mutex<VecDeque<FauxResponseStep>>>,
    source: String,
    cache: Arc<Mutex<std::collections::HashMap<String, String>>>,
}
fn zero_usage() -> Usage {
    Usage {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}
/// Normalize assistant content with simulator defaults.
pub fn faux_assistant_message(
    content: FauxAssistantContent,
    options: FauxAssistantMessageOptions,
) -> AssistantMessage {
    let content = match content {
        FauxAssistantContent::Text(text) => vec![AssistantContent::Text(faux_text(text))],
        FauxAssistantContent::Block(block) => vec![block],
        FauxAssistantContent::Blocks(blocks) => blocks,
    };
    AssistantMessage {
        content,
        api: "faux".into(),
        provider: "faux".into(),
        model: "faux-1".into(),
        response_model: None,
        response_id: options.response_id,
        diagnostics: None,
        usage: zero_usage(),
        stop_reason: options.stop_reason.unwrap_or(StopReason::Stop),
        error_message: options.error_message,
        timestamp: options.timestamp.unwrap_or_else(host::clock),
    }
}
fn estimate_tokens(text: &str) -> f64 {
    (text.chars().count() as f64 / 4.0).ceil()
}
/// Install the opt-in simulator callbacks.
pub fn register_faux_provider(options: RegisterFauxProviderOptions) -> FauxProviderRegistration {
    register_with_host(options, host::production())
}
fn register_with_host(
    options: RegisterFauxProviderOptions,
    host: Arc<dyn host::Host>,
) -> FauxProviderRegistration {
    let size = options.token_size.unwrap_or_default();
    let min = 1.0_f64.max(size.min.unwrap_or(3.0).min(size.max.unwrap_or(5.0)));
    let max = min.max(size.max.unwrap_or(5.0));
    let rate = options.tokens_per_second;
    let api = options
        .api
        .unwrap_or_else(|| random_id("faux", host.as_ref()));
    let model = Model {
        id: "faux-1".into(),
        name: "Faux Model".into(),
        api: api.clone(),
        provider: options.provider.unwrap_or_else(|| "faux".into()),
        base_url: "http://localhost:0".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec!["text".into(), "image".into()],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128000.0,
        max_tokens: 16384.0,
        headers: None,
        compat: None,
    };
    let models = match options.models.filter(|m| !m.is_empty()) {
        None => vec![model],
        Some(definitions) => definitions
            .into_iter()
            .map(|d| Model {
                name: d.name.unwrap_or_else(|| d.id.clone()),
                id: d.id,
                reasoning: d.reasoning.unwrap_or(false),
                input: d.input.unwrap_or_else(|| model.input.clone()),
                cost: d.cost.unwrap_or_else(|| model.cost.clone()),
                context_window: d.context_window.unwrap_or(model.context_window),
                max_tokens: d.max_tokens.unwrap_or(model.max_tokens),
                ..model.clone()
            })
            .collect(),
    };
    let registration = FauxProviderRegistration {
        api: api.clone(),
        models,
        state: Arc::new(RwLock::new(FauxProviderState::default())),
        queue: Arc::new(Mutex::new(VecDeque::new())),
        source: random_id("faux-provider", host.as_ref()),
        cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };
    let owner = registration.clone();
    let raw: ApiStreamFunction = Arc::new(move |model, context, options| {
        let step = owner.queue.lock().unwrap().pop_front();
        owner.state.write().unwrap().call_count += 1.0;
        let events = create_assistant_message_event_stream();
        let output = events.clone();
        let state = owner.state.clone();
        let cache = owner.cache.clone();
        let identity = Model {
            api: owner.api.clone(),
            provider: owner.models[0].provider.clone(),
            ..model.clone()
        };
        let host = host.clone();
        let scheduler = host.clone();
        scheduler.spawn(Box::pin(async move {
            let hook_result =
                if let Some(hook) = options.as_ref().and_then(|o| o.base.on_response.as_ref()) {
                    hook(
                        ProviderResponse {
                            status: 200.0,
                            headers: serde_json::Map::new(),
                        },
                        model.clone(),
                    )
                    .await
                } else {
                    Ok(())
                };
            if let Err(value) = hook_result {
                caught_error(&output, value, &identity, host.as_ref());
                return;
            }
            let step_was_empty = step.is_none();
            let mut message = match step {
                Some(FauxResponseStep::Message(message)) => message,
                Some(FauxResponseStep::Factory(factory)) => {
                    let result =
                        factory(context.clone(), options.clone(), state, model.clone()).await;
                    match result {
                        Ok(message) => message,
                        Err(value) => {
                            caught_error(&output, value, &identity, host.as_ref());
                            return;
                        }
                    }
                }
                None => faux_assistant_message(
                    FauxAssistantContent::Blocks(vec![]),
                    FauxAssistantMessageOptions {
                        stop_reason: Some(StopReason::Error),
                        error_message: Some("No more faux responses queued".into()),
                        timestamp: Some(host.clock()),
                        ..Default::default()
                    },
                ),
            };
            for block in &mut message.content {
                if let AssistantContent::ToolCall(tool) = block {
                    let cloned = tool.read().unwrap().clone();
                    *tool = Arc::new(RwLock::new(cloned));
                }
            }
            message.api = identity.api;
            message.provider = identity.provider;
            message.model = identity.id;
            let prompt = serialize_context(&context);
            message.usage = zero_usage();
            message.usage.input = estimate_tokens(&prompt);
            message.usage.output = estimate_tokens(&assistant_content_to_text(&message.content));
            if let Some(session) = options
                .as_ref()
                .and_then(|o| o.base.session_id.as_ref())
                .filter(|s| !s.is_empty())
                && options
                    .as_ref()
                    .and_then(|o| o.base.cache_retention.as_ref())
                    != Some(&CacheRetention::None)
            {
                let previous = cache
                    .lock()
                    .unwrap()
                    .insert(session.clone(), prompt.clone());
                if let Some(previous) = previous.filter(|s| !s.is_empty()) {
                    let prefix = previous
                        .chars()
                        .zip(prompt.chars())
                        .take_while(|(a, b)| a == b)
                        .count();
                    message.usage.cache_read = (prefix as f64 / 4.0).ceil();
                    message.usage.cache_write =
                        ((prompt.chars().count() - prefix) as f64 / 4.0).ceil();
                    message.usage.input = (message.usage.input - message.usage.cache_read).max(0.0);
                } else {
                    message.usage.cache_write = message.usage.input;
                }
            }
            message.usage.total_tokens =
                message.usage.input + message.usage.output + message.usage.cache_read;
            if step_was_empty {
                finish(&output, message);
            } else {
                stream_with_deltas(
                    &output,
                    message,
                    min,
                    max,
                    rate,
                    options.as_ref().and_then(|o| o.base.signal.as_ref()),
                    host.as_ref(),
                )
                .await;
            }
        }));
        Ok(events)
    });
    let simple_raw = raw.clone();
    register_api_provider(
        ApiProvider {
            api,
            stream: raw,
            stream_simple: Arc::new(move |model, context, options| {
                simple_raw(
                    model,
                    context,
                    options.map(|o| {
                        let mut extra = serde_json::Map::new();
                        if let Some(reasoning) = o.reasoning {
                            extra.insert(
                                "reasoning".into(),
                                serde_json::to_value(reasoning).unwrap(),
                            );
                        }
                        if let Some(budgets) = o.thinking_budgets {
                            extra.insert(
                                "thinkingBudgets".into(),
                                serde_json::to_value(budgets).unwrap(),
                            );
                        }
                        ProviderStreamOptions {
                            base: o.base,
                            extra,
                        }
                    }),
                )
            }),
        },
        Some(registration.source.clone()),
    );
    registration
}
impl FauxProviderRegistration {
    /// Find a model, defaulting to the first for absent or empty IDs.
    pub fn get_model(&self, id: Option<&str>) -> Option<Model> {
        match id.filter(|s| !s.is_empty()) {
            None => self.models.first().cloned(),
            Some(id) => self.models.iter().find(|m| m.id == id).cloned(),
        }
    }
    /// Replace remaining responses without resetting state.
    pub fn set_responses(&self, responses: Vec<FauxResponseStep>) {
        let retired = std::mem::replace(&mut *self.queue.lock().unwrap(), responses.into());
        drop(retired);
    }
    /// Append responses without changing reserved steps.
    pub fn append_responses(&self, responses: Vec<FauxResponseStep>) {
        self.queue.lock().unwrap().extend(responses);
    }
    /// Count steps not yet reserved by invocation.
    pub fn get_pending_response_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
    /// Remove this source without cancelling active requests.
    pub fn unregister(&self) {
        unregister_api_providers(&self.source);
    }
}
fn content_to_text(content: &[InputContent]) -> String {
    content
        .iter()
        .map(|b| match b {
            InputContent::Text(t) => t.text.clone(),
            InputContent::Image(i) => {
                format!("[image:{}:{}]", i.mime_type, i.data.chars().count())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn assistant_content_to_text(content: &[AssistantContent]) -> String {
    content
        .iter()
        .map(|b| match b {
            AssistantContent::Text(t) => t.text.clone(),
            AssistantContent::Thinking(t) => t.thinking.clone(),
            AssistantContent::ToolCall(t) => {
                let t = t.read().unwrap().clone();
                format!(
                    "{}:{}",
                    t.name,
                    serde_json::to_string(&t.arguments).unwrap()
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn serialize_context(context: &Context) -> String {
    let mut parts = vec![];
    if let Some(system) = context.system_prompt.as_ref().filter(|s| !s.is_empty()) {
        parts.push(format!("system:{system}"));
    }
    for message in &context.messages {
        parts.push(match message {
            Message::User(u) => format!(
                "user:{}",
                match &u.content {
                    UserContent::Text(t) => t.clone(),
                    UserContent::Blocks(b) => content_to_text(b),
                }
            ),
            Message::Assistant(a) => format!("assistant:{}", assistant_content_to_text(&a.content)),
            Message::ToolResult(t) => format!(
                "toolResult:{}{}",
                t.tool_name,
                if t.content.is_empty() {
                    String::new()
                } else {
                    format!("\n{}", content_to_text(&t.content))
                }
            ),
        });
    }
    if let Some(tools) = context.tools.as_ref().filter(|t| !t.is_empty()) {
        parts.push(format!("tools:{}", serde_json::to_string(tools).unwrap()));
    }
    parts.join("\n\n")
}
fn finish(output: &AssistantMessageEventStream, message: AssistantMessage) {
    let reason = message.stop_reason.clone();
    let message = Arc::new(RwLock::new(message));
    let event = if matches!(reason, StopReason::Error | StopReason::Aborted) {
        AssistantMessageEvent::Error {
            reason,
            error: message.clone(),
        }
    } else {
        AssistantMessageEvent::Done {
            reason,
            message: message.clone(),
        }
    };
    output.push(event).unwrap();
    output.end(Some(message));
}
fn snapshot(message: &AssistantMessage) -> Arc<RwLock<AssistantMessage>> {
    Arc::new(RwLock::new(message.clone()))
}
fn split_string_by_token_size(
    text: &str,
    min: f64,
    max: f64,
    host: &dyn host::Host,
) -> Vec<String> {
    let units = text.chars().collect::<Vec<_>>();
    let mut chunks = vec![];
    let mut index = 0.0;
    while index < units.len() as f64 {
        let size = min + (host.random() * (max - min + 1.0)).floor();
        let chars = 1.0_f64.max(size * 4.0);
        let start = index as usize;
        let end = (index + chars) as usize;
        chunks.push(
            units[start.min(units.len())..end.min(units.len())]
                .iter()
                .collect(),
        );
        index += chars;
    }
    if chunks.is_empty() {
        chunks.push(String::new())
    }
    chunks
}
async fn schedule_chunk(host: &dyn host::Host, chunk: &str, rate: Option<f64>) {
    if let Some(rate) = rate.filter(|r| *r > 0.0) {
        let delay = estimate_tokens(chunk) / rate * 1000.0;
        host.timer(delay).await;
    } else {
        host.microtask().await;
    }
}
async fn stream_with_deltas(
    output: &AssistantMessageEventStream,
    message: AssistantMessage,
    min: f64,
    max: f64,
    rate: Option<f64>,
    signal: Option<&Cancellation>,
    host: &dyn host::Host,
) {
    let mut partial = message.clone();
    partial.content = vec![];
    let aborted = |partial: &AssistantMessage| {
        if signal.is_some_and(Cancellation::is_cancelled) {
            let mut error = partial.clone();
            error.stop_reason = StopReason::Aborted;
            error.error_message = Some("Request was aborted".into());
            error.timestamp = host.clock();
            finish(output, error);
            true
        } else {
            false
        }
    };
    if aborted(&partial) {
        return;
    }
    output
        .push(AssistantMessageEvent::Start {
            partial: snapshot(&partial),
        })
        .unwrap();
    for (index, block) in message.content.iter().enumerate() {
        if aborted(&partial) {
            return;
        }
        let content_index = index as f64;
        let (text, kind) = match block {
            AssistantContent::Text(t) => {
                partial
                    .content
                    .push(AssistantContent::Text(faux_text(String::new())));
                (t.text.clone(), 0)
            }
            AssistantContent::Thinking(t) => {
                partial
                    .content
                    .push(AssistantContent::Thinking(faux_thinking(String::new())));
                (t.thinking.clone(), 1)
            }
            AssistantContent::ToolCall(t) => {
                let mut t = t.read().unwrap().clone();
                let text = serde_json::to_string(&t.arguments).unwrap();
                t.arguments.clear();
                t.thought_signature = None;
                partial
                    .content
                    .push(AssistantContent::ToolCall(Arc::new(RwLock::new(t))));
                (text, 2)
            }
        };
        let start = match kind {
            0 => AssistantMessageEvent::TextStart {
                content_index,
                partial: snapshot(&partial),
            },
            1 => AssistantMessageEvent::ThinkingStart {
                content_index,
                partial: snapshot(&partial),
            },
            _ => AssistantMessageEvent::ToolcallStart {
                content_index,
                partial: snapshot(&partial),
            },
        };
        output.push(start).unwrap();
        let chunks = split_string_by_token_size(&text, min, max, host);
        for chunk in chunks {
            schedule_chunk(host, &chunk, rate).await;
            if aborted(&partial) {
                return;
            }
            let delta = chunk;
            match &mut partial.content[index] {
                AssistantContent::Text(t) => t.text.push_str(&delta),
                AssistantContent::Thinking(t) => t.thinking.push_str(&delta),
                _ => {}
            }
            let event = match kind {
                0 => AssistantMessageEvent::TextDelta {
                    content_index,
                    delta,
                    partial: snapshot(&partial),
                },
                1 => AssistantMessageEvent::ThinkingDelta {
                    content_index,
                    delta,
                    partial: snapshot(&partial),
                },
                _ => AssistantMessageEvent::ToolcallDelta {
                    content_index,
                    delta,
                    partial: snapshot(&partial),
                },
            };
            output.push(event).unwrap();
        }
        let end = match kind {
            0 => AssistantMessageEvent::TextEnd {
                content_index,
                content: text,
                partial: snapshot(&partial),
            },
            1 => AssistantMessageEvent::ThinkingEnd {
                content_index,
                content: text,
                partial: snapshot(&partial),
            },
            _ => {
                let AssistantContent::ToolCall(tool) = block else {
                    unreachable!()
                };
                let tool = tool.read().unwrap().clone();
                let mut partial_tool = tool.clone();
                partial_tool.thought_signature = None;
                partial.content[index] =
                    AssistantContent::ToolCall(Arc::new(RwLock::new(partial_tool)));
                let tool = Arc::new(RwLock::new(tool));
                AssistantMessageEvent::ToolcallEnd {
                    content_index,
                    tool_call: tool,
                    partial: snapshot(&partial),
                }
            }
        };
        output.push(end).unwrap();
    }
    finish(output, message);
}
fn caught_error(
    output: &AssistantMessageEventStream,
    value: ThrownValue,
    model: &Model,
    host: &dyn host::Host,
) {
    let text = match &value {
        ThrownValue::Error(error) => Ok(error.message.clone()),
        ThrownValue::Undefined => Ok("undefined".into()),
        ThrownValue::Json(value) => Ok(serde_json::to_string(value).unwrap()),
        ThrownValue::Number(value) => Ok(value.to_string()),
        ThrownValue::StringCoercion(convert) => convert(),
    };
    match text {
        Ok(text) => finish(
            output,
            AssistantMessage {
                content: vec![],
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                response_model: None,
                response_id: None,
                diagnostics: None,
                usage: zero_usage(),
                stop_reason: StopReason::Error,
                error_message: Some(text),
                timestamp: host.clock(),
            },
        ),
        Err(value) => {
            let text = match &value {
                ThrownValue::Error(error) => {
                    if error.message.is_empty() {
                        error.name.clone()
                    } else if error.name.is_empty() {
                        error.message.clone()
                    } else {
                        format!("{}: {}", error.name, error.message)
                    }
                }
                _ => format!("{value:?}"),
            };
            host.stderr(&format!("{text}\n"));
        }
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
fn random_id(prefix: &str, host: &dyn host::Host) -> String {
    let mut random = (host.random() * 9007199254740992.0) as u64;
    let mut digits = Vec::new();
    loop {
        digits.push(char::from_digit((random % 36) as u32, 36).unwrap());
        random /= 36;
        if random == 0 {
            break;
        }
    }
    format!(
        "{prefix}:{}:{}",
        host.clock(),
        digits.iter().rev().collect::<String>()
    )
}

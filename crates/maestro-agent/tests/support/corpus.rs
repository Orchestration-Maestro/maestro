//! Replay source-recorded public conversation scenarios.
use super::{assistant, ready, user};
use maestro_agent::*;
use maestro_models::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, RwLock},
};

/// One complete source observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Consumed scenario operands.
    input: BTreeMap<String, Value>,
    /// Complete expected observation tree.
    expected: Value,
}
/// Caller-defined message payload used by custom-tail cases.
struct Notice {
    /// Text converted at the model boundary.
    text: String,
}
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}
/// Shared observations for one replay, not a product callback registry.
#[derive(Default)]
struct Observations {
    /// Lifecycle snapshots.
    events: Vec<Value>,
    /// Callback order.
    trace: Vec<String>,
    /// Model requests.
    requests: Vec<Value>,
}
/// Immutable scenario choices consumed by callbacks.
struct Input {
    /// Consumed boolean source operands.
    flags: BTreeMap<&'static str, bool>,
    /// Trace label.
    id: String,
    /// Prompt selection.
    prompts: String,
    /// Existing tail selection.
    tail: String,
    /// Optional transformation.
    transform: String,
    /// Optional credential outcome.
    key: Option<String>,
    /// Tool inventory choice.
    tools: String,
    /// Argument preparation choice.
    prepare: String,
    /// Tool-call content.
    tool_calls: Option<Vec<AssistantContent>>,
    /// Assistant stop outcome.
    stop: StopReason,
    /// Stream event selection.
    stream: String,
    /// Progress count.
    progress: usize,
    /// Shared termination hint.
    terminate: Option<bool>,
    /// Per-call termination hints.
    terminations: Option<Vec<Value>>,
    /// Optional details override, retaining JSON null.
    details: Option<Value>,
    /// Failing lifecycle callback.
    emit_error: String,
}
/// Remove and decode an optional operand.
fn take<T: serde::de::DeserializeOwned>(
    input: &mut BTreeMap<String, Value>,
    name: &str,
) -> Option<T> {
    input
        .remove(name)
        .map(|value| serde_json::from_value(value).unwrap())
}
impl Input {
    /// Read an already-consumed boolean operand.
    fn flag(&self, name: &str) -> bool {
        self.flags[name]
    }

    /// Consume every declared operand and reject unread fixture fields.
    fn from(mut input: BTreeMap<String, Value>) -> Self {
        let flags = [
            "continue",
            "convertError",
            "streamError",
            "noConfiguredKey",
            "richOptions",
            "abort",
            "noSignal",
            "executeError",
        ]
        .into_iter()
        .map(|name| (name, take(&mut input, name).unwrap_or_default()))
        .collect();
        let value = Self {
            flags,
            id: take(&mut input, "id").unwrap(),
            prompts: take(&mut input, "prompts").unwrap_or_default(),
            tail: take(&mut input, "tail").unwrap_or_default(),
            transform: take(&mut input, "transform").unwrap_or_default(),
            key: take(&mut input, "key"),
            tools: take(&mut input, "tools").unwrap_or_default(),
            prepare: take(&mut input, "prepare").unwrap_or_default(),
            tool_calls: take(&mut input, "toolCalls"),
            stop: take(&mut input, "stop").unwrap_or(StopReason::Stop),
            stream: take(&mut input, "stream").unwrap_or_default(),
            progress: take(&mut input, "progress").unwrap_or_default(),
            terminate: take(&mut input, "terminate"),
            terminations: take(&mut input, "terminations"),
            details: input.remove("details"),
            emit_error: take(&mut input, "emitError").unwrap_or_default(),
        };
        assert!(input.is_empty(), "unread scenario fields");
        value
    }
}
/// Produce a diagnostic at the controlled failure boundary.
fn error(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        stack: None,
        code: None,
    }
}
/// Convert finite numeric storage variants and remove dynamic timestamps.
fn normalized(value: Value) -> Value {
    match value {
        Value::Number(number) => json!(number.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(normalized).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .filter(|(key, _)| key != "timestamp")
                .map(|(key, value)| (key, normalized(value)))
                .collect(),
        ),
        value => value,
    }
}
/// Compare values recursively with explicit object member-order evidence.
fn compare(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
            for (key, value) in actual {
                compare(value, &expected[key]);
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                compare(actual, expected);
            }
        }
        _ => assert!(actual == expected, "recorded scalar mismatch"),
    }
}
/// Project shared entries using their owning record serializers.
fn message(message: &AgentMessage<Notice>) -> Value {
    match message {
        AgentMessage::User(value) => serde_json::to_value(&*value.read().unwrap()).unwrap(),
        AgentMessage::Assistant(value) => serde_json::to_value(&*value.read().unwrap()).unwrap(),
        AgentMessage::ToolResult(value) => serde_json::to_value(&*value.read().unwrap()).unwrap(),
        AgentMessage::Custom(value) => {
            json!({"role":"notice","text":value.read().unwrap().text,"timestamp":1})
        }
    }
}
/// Project the event data in source field order.
fn event(event: &AgentEvent<Notice>) -> Value {
    match event {
        AgentEvent::AgentStart => json!({"type":"agent_start"}),
        AgentEvent::TurnStart => json!({"type":"turn_start"}),
        AgentEvent::AgentEnd { messages } => {
            json!({"type":"agent_end","messages":messages.iter().map(message).collect::<Vec<_>>()})
        }
        AgentEvent::TurnEnd {
            message,
            tool_results,
        } => {
            json!({"type":"turn_end","message":&*message.read().unwrap(),"toolResults":tool_results})
        }
        AgentEvent::MessageStart { message: entry } => {
            json!({"type":"message_start","message":message(entry)})
        }
        AgentEvent::MessageEnd { message: entry } => {
            json!({"type":"message_end","message":message(entry)})
        }
        AgentEvent::MessageUpdate {
            message,
            assistant_message_event,
        } => {
            json!({"type":"message_update","assistantMessageEvent":assistant_message_event,"message":&*message.read().unwrap()})
        }
        AgentEvent::ToolExecutionStart { tool_call } => {
            json!({"type":"tool_execution_start","toolCallId":tool_call.id,"toolName":tool_call.name,"args":tool_call.arguments})
        }
        AgentEvent::ToolExecutionUpdate {
            tool_call,
            partial_result,
        } => {
            json!({"type":"tool_execution_update","toolCallId":tool_call.id,"toolName":tool_call.name,"args":tool_call.arguments,"partialResult":tool_result(partial_result)})
        }
        AgentEvent::ToolExecutionEnd {
            tool_call,
            result,
            is_error,
        } => {
            json!({"type":"tool_execution_end","toolCallId":tool_call.id,"toolName":tool_call.name,"result":tool_result(result),"isError":is_error})
        }
    }
}
/// Project optional termination without inserting absent fields.
fn tool_result(result: &AgentToolResult) -> Value {
    let mut value = json!({"content":result.content,"details":result.details});
    if let Some(terminate) = result.terminate {
        value["terminate"] = json!(terminate);
    }
    value
}
/// Convert a plain helper user to the custom-payload conversation domain.
fn entry(text: &str) -> AgentMessage<Notice> {
    let AgentMessage::User(value) = user(text) else {
        unreachable!()
    };
    AgentMessage::User(value)
}
/// Admit caller custom input without early model conversion.
fn notice(text: &str) -> AgentMessage<Notice> {
    AgentMessage::Custom(Arc::new(RwLock::new(Notice { text: text.into() })))
}
/// Construct the source fixture's history and prompts.
fn context(input: &Input) -> (AgentContext<Notice>, Vec<AgentMessage<Notice>>) {
    let tail = match input.tail.as_str() {
        "empty" => vec![],
        "assistant" => {
            let value = assistant("");
            value.write().unwrap().content.clear();
            vec![AgentMessage::Assistant(value)]
        }
        "tool-result" => vec![AgentMessage::ToolResult(Arc::new(RwLock::new(
            ToolResultMessage {
                tool_call_id: "old".into(),
                tool_name: "echo".into(),
                content: vec![],
                details: Some(Value::Null),
                is_error: false,
                timestamp: 1.0,
            },
        )))],
        "custom" => vec![notice("custom")],
        _ => vec![entry("earlier")],
    };
    let prompts = match input.prompts.as_str() {
        "empty" => vec![],
        "batch" => ["zulu", "alpha", "zulu"].into_iter().map(entry).collect(),
        "custom" => vec![notice("custom prompt")],
        _ => vec![entry("new")],
    };
    (
        AgentContext {
            system_prompt: "instructions".into(),
            messages: Arc::new(RwLock::new(tail)),
            tools: None,
        },
        prompts,
    )
}
/// Replay selected disjoint fixture rows through the public operations.
pub async fn replay(ids: &[&str]) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/loop_cases.json")).unwrap();
    let selected: Vec<_> = cases
        .into_iter()
        .filter(|case| ids.contains(&case.input["id"].as_str().unwrap()))
        .collect();
    assert_eq!(selected.len(), ids.len());
    for case in selected {
        let input = Arc::new(Input::from(case.input));
        let actual = execute(input).await;
        compare(&normalized(actual), &normalized(case.expected));
    }
}
/// Drive one recorded operation and compare every retained observation.
async fn execute(input: Arc<Input>) -> Value {
    assert!(!input.id.is_empty());
    let observations = Arc::new(Mutex::new(Observations::default()));
    let (mut context, prompts) = context(&input);
    let history = if input.prompts == "custom" || input.tail == "custom" {
        context.clone().messages
    } else {
        Arc::clone(&context.messages)
    };
    install_tools(&mut context, &input, Arc::clone(&observations));
    let config = config(&input, Arc::clone(&observations));
    let signal = Cancellation::new();
    if input.flag("abort") {
        signal.abort();
    }
    let options = options(
        Arc::clone(&input),
        Arc::clone(&observations),
        signal.clone(),
        &config,
    );
    let emit = sink(Arc::clone(&input), Arc::clone(&observations));
    let result = if input.flag("continue") {
        run_agent_loop_continue(context, &config, emit, options).await
    } else {
        run_agent_loop(prompts, context, &config, emit, options).await
    };
    let observed = observations.lock().unwrap();
    let mut value = json!({"events":observed.events,"trace":observed.trace,"requests":observed.requests,"history":history.read().unwrap().iter().map(message).collect::<Vec<_>>()});
    match result {
        Ok(returned) => value["returned"] = json!(returned.iter().map(message).collect::<Vec<_>>()),
        Err(error) => value["error"] = json!(error.message),
    }
    value
}
/// Record a snapshot before any controlled listener failure.
fn sink(input: Arc<Input>, observations: Arc<Mutex<Observations>>) -> AgentEventSink<Notice> {
    Arc::new(move |value| {
        let value = event(&value);
        let kind = value["type"].as_str().unwrap();
        let trace = match value.get("toolCallId") {
            Some(id) => format!("{kind}:{}", id.as_str().unwrap()),
            None => kind.into(),
        };
        let result = if kind == input.emit_error {
            Err(error(&format!("sink failed: {kind}")))
        } else {
            Ok(())
        };
        let mut observed = observations.lock().unwrap();
        observed.trace.push(trace);
        observed.events.push(value);
        drop(observed);
        ready(result)
    })
}
/// Build conversion, transformation and credential callbacks.
fn config(input: &Arc<Input>, observations: Arc<Mutex<Observations>>) -> AgentLoopConfig<Notice> {
    let conversion_input = Arc::clone(input);
    let conversion_observed = Arc::clone(&observations);
    let mut options = SimpleStreamOptions::default();
    options.common.api_key = (!input.flag("noConfiguredKey")).then(|| "fallback".into());
    options.common.session_id = Some("session".into());
    options.common.temperature = Some(0.25);
    let mut config = AgentLoopConfig {
        model: super::model(),
        options,
        extra: JsonObject::from_iter([(
            "customField".into(),
            json!({"x":["zulu","alpha","zulu"]}),
        )]),
        objects: ProviderObjects::default(),
        convert_to_llm: Arc::new(move |messages| {
            conversion_observed
                .lock()
                .unwrap()
                .trace
                .push("convert".into());
            if conversion_input.flag("convertError") {
                return ready(Err(error("convert failed")));
            }
            let converted = messages.read().unwrap().iter().map(convert).collect();
            ready(Ok(converted))
        }),
        transform_context: None,
        get_api_key: None,
    };
    config.model.input = vec![ModelInput::Text];
    config.model.context_window = 8192.0;
    config.model.max_tokens = 2048.0;
    install_transform(&mut config, Arc::clone(input), Arc::clone(&observations));
    install_key(&mut config, input, observations);
    if input.flag("richOptions") {
        rich_options(&mut config);
    }
    config
}
/// Convert custom input only at the model boundary.
fn convert(message: &AgentMessage<Notice>) -> Message {
    match message {
        AgentMessage::User(value) => Message::User(value.read().unwrap().clone()),
        AgentMessage::Assistant(value) => Message::Assistant(value.read().unwrap().clone()),
        AgentMessage::ToolResult(value) => Message::ToolResult(value.read().unwrap().clone()),
        AgentMessage::Custom(value) => Message::User(UserMessage {
            content: UserContent::Text(value.read().unwrap().text.clone()),
            timestamp: 1.0,
        }),
    }
}
/// Install transformation only for scenarios that request it.
fn install_transform(
    config: &mut AgentLoopConfig<Notice>,
    input: Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    if input.transform.is_empty() {
        return;
    }
    config.transform_context = Some(Arc::new(move |messages, _| {
        observed.lock().unwrap().trace.push("transform".into());
        match input.transform.as_str() {
            "error" => ready(Err(error("transform failed"))),
            "inplace" => {
                messages.write().unwrap().insert(0, entry("injected"));
                ready(Ok(messages))
            }
            _ => {
                let tail = messages.read().unwrap().last().unwrap().clone();
                ready(Ok(Arc::new(RwLock::new(vec![tail]))))
            }
        }
    }));
}
/// Resolve the configured source credential case per invocation.
fn install_key(
    config: &mut AgentLoopConfig<Notice>,
    input: &Input,
    observed: Arc<Mutex<Observations>>,
) {
    let Some(key) = input.key.clone() else {
        return;
    };
    config.get_api_key = Some(Arc::new(move |provider| {
        let calls = {
            let mut observed = observed.lock().unwrap();
            observed.trace.push(format!("key:{provider}"));
            observed.requests.len()
        };
        ready(match key.as_str() {
            "error" => Err(error("key failed")),
            "missing" => Ok(None),
            "refresh" => Ok(Some(format!("fresh-{calls}"))),
            _ => Ok(Some(key.clone())),
        })
    }));
}
/// Install a prepared executable tool with complete progress and result data.
fn install_tools(
    context: &mut AgentContext<Notice>,
    input: &Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    if input.tools == "absent" {
        return;
    }
    let mut tools = vec![];
    if input.tools != "empty" {
        tools.push(tool(Arc::clone(input), observed));
    }
    if input.tools == "duplicates" {
        tools.push(Arc::new(RwLock::new(AgentTool {
            definition: Tool {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: schema(),
            },
            label: "Echo".into(),
            prepare_arguments: None,
            execution_mode: None,
            execute: Arc::new(|_, _, _, _| ready(Err(error("wrong duplicate")))),
        })));
    }
    context.tools = Some(Arc::new(RwLock::new(tools)));
}
/// Return the model-facing argument schema used by the source oracle.
fn schema() -> Value {
    json!({"type":"object","properties":{"value":{"type":"integer"}},"required":["value"]})
}
/// Build one source-compatible executable tool.
fn tool(input: Arc<Input>, observed: Arc<Mutex<Observations>>) -> SharedAgentTool {
    let preparation = if input.prepare.is_empty() {
        None
    } else {
        let input = Arc::clone(&input);
        let observed = Arc::clone(&observed);
        Some(Arc::new(move |args| {
            observed.lock().unwrap().trace.push("prepare".into());
            match input.prepare.as_str() {
                "error" => Err(error("prepare failed")),
                "identity" => Ok(args),
                _ => Ok(json!({"value":"8"})),
            }
        }) as PrepareArguments)
    };
    Arc::new(RwLock::new(AgentTool {
        definition: Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: schema(),
        },
        label: "Echo".into(),
        prepare_arguments: preparation,
        execution_mode: None,
        execute: Arc::new(move |id, args, signal, update| {
            let text = trace_json(&args);
            observed.lock().unwrap().trace.push(format!(
                "execute:{id}:{text}:aborted={}",
                signal.as_ref().is_some_and(Cancellation::is_aborted)
            ));
            for index in 0..input.progress {
                update.as_ref().unwrap()(AgentToolResult {
                    content: vec![UserBlock::Text(TextContent {
                        text: format!("partial-{index}"),
                        text_signature: None,
                    })],
                    details: json!({"index":index}),
                    terminate: None,
                });
            }
            ready(if input.flag("executeError") {
                Err(error("execution failed"))
            } else {
                Ok(output(&input, &id))
            })
        }),
    }))
}
/// Preserve detail forms and optional termination hints.
fn output(input: &Input, id: &str) -> AgentToolResult {
    let terminate = input
        .terminations
        .as_ref()
        .map_or(input.terminate, |values| {
            values[usize::from(id != "one")].as_bool()
        });
    AgentToolResult { content:serde_json::from_value(json!([{ "type":"text","text":"result" },{"type":"image","data":"AA==","mimeType":"image/png"}])).unwrap(),details:input.details.clone().unwrap_or_else(|| json!({"b":2,"a":1})),terminate }
}
/// Construct a replacement stream and capture the request settings.
fn options(
    input: Arc<Input>,
    observed: Arc<Mutex<Observations>>,
    signal: Cancellation,
    config: &AgentLoopConfig<Notice>,
) -> AgentLoopOptions {
    let invocation = (!input.flag("noSignal")).then_some(signal.clone());
    let expected = config.options.clone();
    let objects = config.objects.clone();
    AgentLoopOptions {
        signal: invocation,
        stream_fn: Some(Arc::new(move |_, context, options, open| {
            let mut request = json!({"context":context,"options":{}});
            request["options"] = request_options(&input, &options, &open, &signal, &expected);
            if input.flag("richOptions") {
                assert!(Arc::ptr_eq(
                    open.objects.get::<Arc<()>>().unwrap(),
                    objects.get::<Arc<()>>().unwrap()
                ));
            }
            let index = {
                let mut observed = observed.lock().unwrap();
                let index = observed.requests.len();
                observed.trace.push("request".into());
                observed.requests.push(request);
                index
            };
            if input.flag("streamError") {
                return ready(Err(error("stream failed")));
            }
            ready(Ok(stream(&input, index)))
        })),
    }
}
/// Build declared model event variants, without undeclared source-oracle fields.
fn stream(input: &Input, index: usize) -> AssistantMessageEventStream {
    let response = assistant("finished");
    let stop = if index == 0 {
        input.stop.clone()
    } else {
        StopReason::Stop
    };
    {
        let mut value = response.write().unwrap();
        value.stop_reason = stop.clone();
        if index == 0
            && let Some(calls) = &input.tool_calls
        {
            value.content.clone_from(calls);
            value.stop_reason = if input.stop == StopReason::Stop {
                StopReason::ToolUse
            } else {
                stop
            };
        }
    }
    let stream = AssistantMessageEventStream::new();
    let partial = assistant("partial");
    let mode = input.stream.as_str();
    if mode == "updates-before-start" {
        stream.push(AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "ignored".into(),
            partial: Arc::clone(&partial),
        });
    }
    if ["updates", "start-eof", "start-done"].contains(&mode) {
        stream.push(AssistantMessageEvent::Start {
            partial: Arc::clone(&partial),
        });
    }
    if mode == "updates" {
        updates(&stream, &partial);
    }
    if ["eof", "start-eof"].contains(&mode) {
        stream.end(Some(response));
    } else {
        terminal(&stream, response);
    }
    stream
}
/// Render the corpus's numeric argument leaves with source number spelling.
fn trace_json(value: &Value) -> String {
    fn numbers(value: &Value) -> Value {
        match value {
            Value::Number(number) => {
                serde_json::from_str(&number.as_f64().unwrap().to_string()).unwrap()
            }
            Value::Array(values) => Value::Array(values.iter().map(numbers).collect()),
            Value::Object(values) => Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), numbers(value)))
                    .collect(),
            ),
            value => value.clone(),
        }
    }
    serde_json::to_string(&numbers(value)).unwrap()
}
/// Carry all common/simple options and a shared typed object.
fn rich_options(config: &mut AgentLoopConfig<Notice>) {
    let common = &mut config.options.common;
    common.temperature = Some(0.75);
    common.max_tokens = Some(123.0);
    common.transport = Some(Transport::WebsocketCached);
    common.cache_retention = Some(CacheRetention::Long);
    common.session_id = Some("session-z".into());
    common.headers = Some(serde_json::from_value(json!({"X-Z":"z","X-A":"a"})).unwrap());
    common.timeout_ms = Some(0.0);
    common.max_retries = Some(2.0);
    common.max_retry_delay_ms = Some(42.0);
    common.metadata = Some(serde_json::from_value(json!({"z":1,"a":2})).unwrap());
    common.on_payload = Some(Arc::new(|value, _| ready(Ok(value))));
    common.on_response = Some(Arc::new(|_, _| ready(Ok(()))));
    common.fetch = Some(Arc::new(|_| ready(Err(FetchError::Aborted))));
    config.options.reasoning = Some(maestro_models::ThinkingLevel::High);
    config.options.thinking_budgets = Some(ThinkingBudgets {
        minimal: Some(1.0),
        low: Some(2.0),
        medium: Some(3.0),
        high: Some(4.0),
    });
    config.options.tool_choice = Some(ToolChoice::Function {
        name: "echo".into(),
    });
    config.objects.insert(Arc::new(()));
}
/// Project request settings while witnessing callback and typed-object identity.
fn request_options(
    input: &Input,
    options: &SimpleStreamOptions,
    open: &ProviderStreamOptions,
    signal: &Cancellation,
    expected: &SimpleStreamOptions,
) -> Value {
    let mut value = json!({"sessionId":options.common.session_id,"temperature":options.common.temperature,"customField":open.extra["customField"],"signalSame":options.common.signal.is_some() != input.flag("noSignal"),"signalAborted":options.common.signal.as_ref().is_some_and(Cancellation::is_aborted)});
    if let Some(key) = &options.common.api_key {
        let old = value.as_object_mut().unwrap();
        let mut ordered = JsonObject::from_iter([("apiKey".into(), json!(key))]);
        ordered.append(old);
        value = Value::Object(ordered);
    }
    if let Some(passed) = &options.common.signal {
        assert_eq!(passed.is_aborted(), signal.is_aborted());
    }
    assert!(
        options.common.api_key == open.common.api_key,
        "common key mismatch"
    );
    if !input.flag("richOptions") {
        return value;
    }
    compare_common_options(&options.common, &open.common);
    rich_request_fields(&mut value, options, open, expected);
    value
}
/// Inspect every rich option and callback identity.
fn rich_request_fields(
    value: &mut Value,
    options: &SimpleStreamOptions,
    open: &ProviderStreamOptions,
    expected: &SimpleStreamOptions,
) {
    let common = &options.common;
    let object = value.as_object_mut().unwrap();
    for (name, value) in [
        ("maxTokens", json!(common.max_tokens)),
        ("transport", json!(common.transport)),
        ("cacheRetention", json!(common.cache_retention)),
        ("headers", json!(common.headers)),
        ("timeoutMs", json!(common.timeout_ms)),
        ("maxRetries", json!(common.max_retries)),
        ("maxRetryDelayMs", json!(common.max_retry_delay_ms)),
        ("metadata", json!(common.metadata)),
        ("reasoning", json!(options.reasoning)),
        ("thinkingBudgets", json!(options.thinking_budgets)),
        (
            "toolChoice",
            json!({"type":"function","function":{"name":match &options.tool_choice { Some(ToolChoice::Function {name})=>name,_=>panic!("tool choice")}}}),
        ),
    ] {
        object.insert(name.into(), value);
    }
    object.insert(
        "onPayloadSame".into(),
        json!(Arc::ptr_eq(
            common.on_payload.as_ref().unwrap(),
            expected.common.on_payload.as_ref().unwrap()
        )),
    );
    object.insert(
        "onResponseSame".into(),
        json!(Arc::ptr_eq(
            common.on_response.as_ref().unwrap(),
            expected.common.on_response.as_ref().unwrap()
        )),
    );
    object.insert(
        "fetchSame".into(),
        json!(Arc::ptr_eq(
            common.fetch.as_ref().unwrap(),
            expected.common.fetch.as_ref().unwrap()
        )),
    );
    object.insert(
        "clientSame".into(),
        json!(open.objects.get::<Arc<()>>().is_some()),
    );
}
/// Push one event of every declared update variant in the source order.
fn updates(stream: &AssistantMessageEventStream, partial: &SharedAssistantMessage) {
    let events = [
        AssistantMessageEvent::TextStart {
            content_index: 0,
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta: "delta".into(),
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::TextEnd {
            content_index: 0,
            content: "content".into(),
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ThinkingStart {
            content_index: 0,
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ThinkingDelta {
            content_index: 0,
            delta: "delta".into(),
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ThinkingEnd {
            content_index: 0,
            content: "content".into(),
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ToolcallStart {
            content_index: 0,
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ToolcallDelta {
            content_index: 0,
            delta: "delta".into(),
            partial: Arc::clone(partial),
        },
        AssistantMessageEvent::ToolcallEnd {
            content_index: 0,
            tool_call: ToolCall {
                id: "one".into(),
                name: "echo".into(),
                arguments: serde_json::from_value(json!({"value":"7"})).unwrap(),
                thought_signature: None,
            },
            partial: Arc::clone(partial),
        },
    ];
    for event in events {
        stream.push(event);
    }
}
/// Publish the typed terminal reason supported by the model stream.
fn terminal(stream: &AssistantMessageEventStream, response: SharedAssistantMessage) {
    let stop = response.read().unwrap().stop_reason.clone();
    let event = match stop {
        StopReason::Error => AssistantMessageEvent::Error {
            reason: ErrorReason::Error,
            error: response,
        },
        StopReason::Aborted => AssistantMessageEvent::Error {
            reason: ErrorReason::Aborted,
            error: response,
        },
        StopReason::Length => AssistantMessageEvent::Done {
            reason: DoneReason::Length,
            message: response,
        },
        StopReason::ToolUse => AssistantMessageEvent::Done {
            reason: DoneReason::ToolUse,
            message: response,
        },
        StopReason::Stop => AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: response,
        },
    };
    stream.push(event);
}

/// Verify both replacement-stream representations retain every common option.
fn compare_common_options(simple: &StreamOptions, open: &StreamOptions) {
    fn scalar_fields(value: &StreamOptions) -> Value {
        json!({"temperature":value.temperature,"maxTokens":value.max_tokens,"transport":value.transport,"cacheRetention":value.cache_retention,"sessionId":value.session_id,"headers":value.headers,"timeoutMs":value.timeout_ms,"maxRetries":value.max_retries,"maxRetryDelayMs":value.max_retry_delay_ms,"metadata":value.metadata})
    }
    compare(&scalar_fields(simple), &scalar_fields(open));
    assert!(
        simple.api_key == open.api_key,
        "credential representations differ"
    );
    assert_eq!(simple.signal.is_some(), open.signal.is_some());
    assert!(Arc::ptr_eq(
        simple.on_payload.as_ref().unwrap(),
        open.on_payload.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        simple.on_response.as_ref().unwrap(),
        open.on_response.as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        simple.fetch.as_ref().unwrap(),
        open.fetch.as_ref().unwrap()
    ));
}

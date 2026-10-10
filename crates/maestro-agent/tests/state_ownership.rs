//! Public observations of retained conversation state.
#![cfg(test)]
use maestro_agent::{
    Agent, AgentInitialState, AgentMessage, AgentOptions, AgentState, AgentTool,
    CustomAgentMessages, QueueMode, SharedAgentTool, ThinkingLevel, ToolExecutionMode,
};
use maestro_models::{Model, UserContent, UserMessage};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};
/// A recorded public scenario with no unread top-level metadata.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Test family selector.
    name: String,
    /// Scenario operands, consumed individually.
    input: BTreeMap<String, Value>,
    /// Complete independently recorded result tree.
    expected: Value,
}
/// Select recorded cases for one public witness.
fn cases(name: &str) -> Vec<Case> {
    serde_json::from_str::<Vec<Case>>(include_str!("fixtures/state_cases.json"))
        .unwrap()
        .into_iter()
        .filter(|case| case.name == name)
        .collect()
}
/// Remove one required operand and decode its declared test type.
fn take<T: serde::de::DeserializeOwned>(input: &mut BTreeMap<String, Value>, key: &str) -> T {
    serde_json::from_value(input.remove(key).unwrap()).unwrap()
}
/// Enforce complete input consumption and compare the complete result tree.
fn check(case: Case, actual: Value) {
    assert!(case.input.is_empty(), "unread scenario operands");
    assert_eq!(numeric_tree(actual), numeric_tree(case.expected));
}
/// Compare JSON numbers as the shared finite numeric domain, not integer storage tags.
fn numeric_tree(value: Value) -> Value {
    match value {
        Value::Number(number) => json!(number.as_f64().unwrap()),
        Value::Array(values) => Value::Array(values.into_iter().map(numeric_tree).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, numeric_tree(value)))
                .collect(),
        ),
        value => value,
    }
}
/// Construct shared plain-text user input.
fn user(text: &str) -> AgentMessage {
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 123.0,
    })))
}
/// Replace a retained user's text.
fn edit_user(message: &AgentMessage, text: String) {
    let AgentMessage::User(entry) = message else {
        panic!("expected user")
    };
    entry.write().unwrap().content = UserContent::Text(text);
}
/// Project model-owned conversation entries without creating copied state.
fn messages<C: CustomAgentMessages>(entries: &[AgentMessage<C>]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|message| match message {
                AgentMessage::User(entry) => serde_json::to_value(&*entry.read().unwrap()).unwrap(),
                AgentMessage::Assistant(entry) => {
                    serde_json::to_value(&*entry.read().unwrap()).unwrap()
                }
                AgentMessage::ToolResult(entry) => {
                    serde_json::to_value(&*entry.read().unwrap()).unwrap()
                }
                AgentMessage::Custom(_) => panic!("custom projection has its own typed witness"),
            })
            .collect(),
    )
}
/// Project every scoped live-state observation.
fn state<C: CustomAgentMessages>(state: &AgentState<C>) -> Value {
    let tools: Vec<_> = state
        .tools()
        .read()
        .unwrap()
        .iter()
        .map(|entry| {
            let guard = entry.read().unwrap();
            let tool = guard.downcast_ref::<Value, Value>().unwrap();
            let mut value = json!({"name": tool.definition.name, "label": tool.label,
            "description": tool.definition.description, "parameters": tool.definition.parameters});
            if let Some(mode) = tool.execution_mode {
                value["executionMode"] = json!(match mode {
                    ToolExecutionMode::Sequential => "sequential",
                    ToolExecutionMode::Parallel => "parallel",
                });
            }
            value
        })
        .collect();
    json!({"systemPrompt": state.system_prompt, "model": *state.model.read().unwrap(),
        "thinkingLevel": state.thinking_level, "tools": tools,
        "messages": messages(&state.messages().read().unwrap()), "isStreaming": state.is_streaming(),
        "streamingMessage": state.streaming_message().map(|entry| messages(std::slice::from_ref(entry))),
        "pendingToolCalls": state.pending_tool_calls().as_ref(), "errorMessage": state.error_message()})
}
/// Construct the nondefault descriptor used by state retention scenarios.
fn chosen_model() -> Arc<RwLock<Model>> {
    Arc::new(RwLock::new(
        serde_json::from_value(json!({
            "id":"model-2", "name":"Second", "api":"test-api", "provider":"local",
            "baseUrl":"http://example.invalid", "reasoning":true, "input":["text","image"],
            "cost":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4},
            "contextWindow":8192,"maxTokens":2048,
        }))
        .unwrap(),
    ))
}
/// Convert the two recorded queue policy spellings.
fn mode(text: &str) -> QueueMode {
    match text {
        "all" => QueueMode::All,
        "one-at-a-time" => QueueMode::OneAtATime,
        _ => panic!("unknown mode"),
    }
}
/// Render a queue policy.
fn mode_text(mode: QueueMode) -> &'static str {
    match mode {
        QueueMode::All => "all",
        QueueMode::OneAtATime => "one-at-a-time",
    }
}
/// Replace a policy with its other admitted value.
fn opposite(mode: QueueMode) -> QueueMode {
    match mode {
        QueueMode::All => QueueMode::OneAtATime,
        QueueMode::OneAtATime => QueueMode::All,
    }
}
/// Observe both policies and queue presence.
fn policies(agent: &Agent) -> Value {
    json!({"steeringMode": mode_text(agent.steering_mode()),
        "followUpMode": mode_text(agent.follow_up_mode()), "hasQueuedMessages": agent.has_queued_messages()})
}
/// Default state is inert and its complete descriptor is shared.
#[test]
fn maestro_new_agent_has_inert_defaults() {
    let agent: Agent = Agent::default();
    let other: Agent = Agent::default();
    let retained = agent.state().read().unwrap();
    let same_model = Arc::ptr_eq(&retained.model, &other.state().read().unwrap().model);
    for case in cases("maestro_new_agent_has_inert_defaults") {
        check(
            case,
            json!({"state": state(&retained), "sameDefaultModel": same_model,
            "steeringMode":mode_text(agent.steering_mode()), "followUpMode":mode_text(agent.follow_up_mode()),
            "hasQueuedMessages":agent.has_queued_messages()}),
        );
    }
}
/// Supplied initialization preserves every thinking level and caller record.
#[test]
fn maestro_initial_state_uses_supplied_values() {
    for mut case in cases("maestro_initial_state_uses_supplied_values") {
        let entries: Vec<UserMessage> = take(&mut case.input, "messages");
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                system_prompt: Some(take(&mut case.input, "systemPrompt")),
                model: Some(Arc::new(RwLock::new(take(&mut case.input, "model")))),
                thinking_level: Some(take::<ThinkingLevel>(&mut case.input, "thinkingLevel")),
                messages: entries
                    .into_iter()
                    .map(|entry| AgentMessage::User(Arc::new(RwLock::new(entry))))
                    .collect(),
                ..Default::default()
            },
            ..Default::default()
        });
        let actual = state(&agent.state().read().unwrap());
        check(case, actual);
    }
}
/// Replacing history preserves the prior collection and shared entry identity.
#[test]
fn maestro_message_collections_preserve_aliases() {
    for mut case in cases("maestro_message_collections_preserve_aliases") {
        let seed = user(&take::<String>(&mut case.input, "initial"));
        let mut original = vec![seed.clone()];
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                messages: original.clone(),
                ..Default::default()
            },
            ..Default::default()
        });
        original.push(user(&take::<String>(&mut case.input, "outside")));
        edit_user(&seed, take(&mut case.input, "edit"));
        let captured = Arc::clone(agent.state().read().unwrap().messages());
        captured
            .write()
            .unwrap()
            .push(user(&take::<String>(&mut case.input, "append")));
        if take::<bool>(&mut case.input, "repeatSeed") {
            captured.write().unwrap().push(seed);
        }
        let replay: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                messages: captured.read().unwrap().clone(),
                ..Default::default()
            },
            ..Default::default()
        });
        let before = messages(&replay.state().read().unwrap().messages().read().unwrap());
        agent
            .state()
            .write()
            .unwrap()
            .set_messages(vec![user(&take::<String>(&mut case.input, "replacement"))]);
        captured
            .write()
            .unwrap()
            .push(user(&take::<String>(&mut case.input, "oldAppend")));
        let current = Arc::clone(agent.state().read().unwrap().messages());
        check(
            case,
            json!({"before":before, "old":messages(&captured.read().unwrap()),
            "current":messages(&current.read().unwrap()), "copiedInitial":captured.read().unwrap().len() != original.len(),
            "newSlot":!Arc::ptr_eq(&captured, &current)}),
        );
    }
}
/// Steering stores input without changing history or streaming.
#[test]
fn maestro_steering_only_enqueues() {
    for mut case in cases("maestro_steering_only_enqueues") {
        let agent: Agent = Agent::default();
        assert_eq!(take::<String>(&mut case.input, "queue"), "steer");
        agent.steer(user(&take::<String>(&mut case.input, "text")));
        let retained = agent.state().read().unwrap();
        check(
            case,
            json!({"messages":messages(&retained.messages().read().unwrap()),
            "hasQueuedMessages":agent.has_queued_messages(), "isStreaming":retained.is_streaming()}),
        );
    }
}
/// Follow-up stores input without changing history or streaming.
#[test]
fn maestro_follow_up_only_enqueues() {
    for mut case in cases("maestro_follow_up_only_enqueues") {
        let agent: Agent = Agent::default();
        assert_eq!(take::<String>(&mut case.input, "queue"), "followUp");
        agent.follow_up(user(&take::<String>(&mut case.input, "text")));
        let retained = agent.state().read().unwrap();
        check(
            case,
            json!({"messages":messages(&retained.messages().read().unwrap()),
            "hasQueuedMessages":agent.has_queued_messages(), "isStreaming":retained.is_streaming()}),
        );
    }
}
/// Clearing one queue leaves input in the other queue untouched.
#[test]
fn maestro_clears_only_selected_queues() {
    for mut case in cases("maestro_clears_only_selected_queues") {
        let agent: Agent = Agent::default();
        let present: String = take(&mut case.input, "present");
        if matches!(present.as_str(), "steering" | "both") {
            agent.steer(user("s"));
        }
        if matches!(present.as_str(), "followUp" | "both") {
            agent.follow_up(user("f"));
        }
        match take::<String>(&mut case.input, "clear").as_str() {
            "clearSteeringQueue" => agent.clear_steering_queue(),
            "clearFollowUpQueue" => agent.clear_follow_up_queue(),
            "clearAllQueues" => agent.clear_all_queues(),
            _ => panic!("unknown clear operation"),
        }
        check(
            case,
            json!({"hasQueuedMessages":agent.has_queued_messages(),
            "messages":messages(&agent.state().read().unwrap().messages().read().unwrap())}),
        );
    }
}
/// Initial queue policies can be independently replaced without clearing input.
#[test]
fn maestro_queue_modes_are_independent() {
    for mut case in cases("maestro_queue_modes_are_independent") {
        let steering = mode(&take::<String>(&mut case.input, "steeringMode"));
        let follow = mode(&take::<String>(&mut case.input, "followUpMode"));
        let agent: Agent = Agent::new(AgentOptions {
            steering_mode: Some(steering),
            follow_up_mode: Some(follow),
            ..Default::default()
        });
        agent.steer(user("retained-steering"));
        agent.follow_up(user("retained-followup"));
        agent.set_steering_mode(opposite(steering));
        let after_steering = policies(&agent);
        agent.set_follow_up_mode(opposite(follow));
        let after_follow = policies(&agent);
        agent.clear_steering_queue();
        let follow_remains = agent.has_queued_messages();
        agent.clear_follow_up_queue();
        check(
            case,
            json!({"afterSteering":after_steering,"afterFollowUp":after_follow,
            "followUpRemains":follow_remains,"emptyAfterClear":!agent.has_queued_messages()}),
        );
    }
}
/// Retained state and selected model remain live through reset.
#[test]
fn maestro_state_and_model_handles_stay_live() {
    for mut case in cases("maestro_state_and_model_handles_stay_live") {
        let model = chosen_model();
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                model: Some(Arc::clone(&model)),
                ..Default::default()
            },
            ..Default::default()
        });
        model.write().unwrap().id = take(&mut case.input, "modelId");
        let retained = Arc::clone(agent.state());
        {
            let mut live = retained.write().unwrap();
            live.system_prompt = take(&mut case.input, "systemPrompt");
            live.thinking_level = take(&mut case.input, "thinkingLevel");
        }
        agent.reset();
        let live = retained.read().unwrap();
        check(
            case,
            json!({"sameState":Arc::ptr_eq(&retained,agent.state()),
            "sameModel":Arc::ptr_eq(&live.model,&model),"state":state(&live)}),
        );
    }
}
/// Construct a retained executable tool with fail-fast callback sentinels.
fn tool(
    name: &str,
    label: &str,
    execution_mode: Option<ToolExecutionMode>,
) -> (SharedAgentTool, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let preparation = Arc::clone(&calls);
    let execution = Arc::clone(&calls);
    let entry: SharedAgentTool = Arc::new(RwLock::new(AgentTool::<Value, Value> {
        definition: maestro_models::Tool {
            name: name.into(),
            description: format!("{name} tool"),
            parameters: json!({"type":"object"}),
        },
        label: label.into(),
        prepare_arguments: Some(Arc::new(move |_| {
            preparation.fetch_add(1, Ordering::SeqCst);
            panic!("unexpected preparation")
        })),
        execute: Arc::new(move |_, _, _, _| {
            execution.fetch_add(1, Ordering::SeqCst);
            panic!("unexpected execution")
        }),
        execution_mode,
    }));
    (entry, calls)
}
/// Reset replaces history while preserving selected configuration and tools.
#[test]
fn maestro_reset_replaces_history_and_clears_queues() {
    for mut case in cases("maestro_reset_replaces_history_and_clears_queues") {
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                system_prompt: Some(take(&mut case.input, "systemPrompt")),
                model: Some(chosen_model()),
                thinking_level: Some(take(&mut case.input, "thinkingLevel")),
                tools: vec![tool("kept", "Kept", None).0],
                messages: vec![user("discard")],
            },
            steering_mode: Some(mode(&take::<String>(&mut case.input, "steeringMode"))),
            follow_up_mode: Some(mode(&take::<String>(&mut case.input, "followUpMode"))),
        });
        let old = Arc::clone(agent.state().read().unwrap().messages());
        let pending = Arc::clone(agent.state().read().unwrap().pending_tool_calls());
        agent.steer(user("s"));
        agent.follow_up(user("f"));
        agent.reset();
        let live = agent.state().read().unwrap();
        assert!(
            !Arc::ptr_eq(&pending, live.pending_tool_calls()),
            "reset replaces pending collection"
        );
        check(
            case,
            json!({"state":state(&live),"oldMessages":messages(&old.read().unwrap()),
            "newSlot":!Arc::ptr_eq(&old,live.messages()),"hasQueuedMessages":agent.has_queued_messages(),
            "steeringMode":mode_text(agent.steering_mode()),"followUpMode":mode_text(agent.follow_up_mode())}),
        );
    }
}
/// Retained tools preserve executable identities and both outer-copy boundaries.
#[test]
fn maestro_tool_collections_retain_executable_entries() {
    assert_tool_order_and_duplicates();
    for mut case in cases("maestro_tool_collections_retain_executable_entries") {
        let name: String = take(&mut case.input, "name");
        let (entry, calls) = tool(
            &name,
            "Echo",
            Some(tool_mode(&take::<String>(&mut case.input, "mode"))),
        );
        let mut original = vec![Arc::clone(&entry)];
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                tools: original.clone(),
                ..Default::default()
            },
            ..Default::default()
        });
        original.clear();
        let captured = Arc::clone(agent.state().read().unwrap().tools());
        with_tool(&entry, |tool| tool.label = take(&mut case.input, "label"));
        captured.write().unwrap().push(Arc::clone(&entry));
        let before: Vec<_> = captured
            .read()
            .unwrap()
            .iter()
            .map(|entry| {
                let guard = entry.read().unwrap();
                guard.downcast_ref::<Value, Value>().unwrap().label.clone()
            })
            .collect();
        let mut replacement = vec![Arc::clone(&entry)];
        agent
            .state()
            .write()
            .unwrap()
            .set_tools(replacement.clone());
        replacement.clear();
        captured.write().unwrap().push(Arc::clone(&entry));
        let replacement_mode = tool_mode(&take::<String>(&mut case.input, "replacementMode"));
        with_tool(&entry, |tool| tool.execution_mode = Some(replacement_mode));
        let current = Arc::clone(agent.state().read().unwrap().tools());
        let current_guard = current.read().unwrap();
        let stored_guard = current_guard[0].read().unwrap();
        let stored = stored_guard.downcast_ref::<Value, Value>().unwrap();
        let supplied_guard = entry.read().unwrap();
        let supplied = supplied_guard.downcast_ref::<Value, Value>().unwrap();
        check(
            case,
            json!({"called":calls.load(Ordering::SeqCst),"copiedOuter":captured.read().unwrap().len()!=original.len(),
            "copiedAssignment":current_guard.len()!=replacement.len(),
            "sameEntry":Arc::ptr_eq(&captured.read().unwrap()[0],&entry),
            "samePrepare":Arc::ptr_eq(stored.prepare_arguments.as_ref().unwrap(),supplied.prepare_arguments.as_ref().unwrap()),
            "sameExecute":Arc::ptr_eq(&stored.execute,&supplied.execute),"before":before,
            "oldCount":captured.read().unwrap().len(),"currentCount":current_guard.len(),
            "currentMode":match stored.execution_mode.unwrap() {ToolExecutionMode::Sequential=>"sequential",ToolExecutionMode::Parallel=>"parallel"}}),
        );
    }
}
/// Mutate a retained default-typed tool through its shared handle.
fn with_tool(entry: &SharedAgentTool, change: impl FnOnce(&mut AgentTool)) {
    change(
        entry
            .write()
            .unwrap()
            .downcast_mut::<Value, Value>()
            .unwrap(),
    );
}
/// Tools with different parameter and detail types share one collection and keep their identity.
#[test]
fn maestro_heterogeneous_typed_tools_are_retained_by_identity() {
    let typed = |name: &str| maestro_models::Tool {
        name: name.into(),
        description: String::new(),
        parameters: json!({}),
    };
    let numeric: SharedAgentTool = Arc::new(RwLock::new(AgentTool::<u32, String> {
        definition: typed("numeric"),
        label: "Numeric".into(),
        prepare_arguments: None,
        execute: Arc::new(|_, _, _, _| panic!("unexpected execution")),
        execution_mode: None,
    }));
    let textual: SharedAgentTool = Arc::new(RwLock::new(AgentTool::<String, Vec<u8>> {
        definition: typed("textual"),
        label: "Textual".into(),
        prepare_arguments: None,
        execute: Arc::new(|_, _, _, _| panic!("unexpected execution")),
        execution_mode: None,
    }));
    let agent: Agent = Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            tools: vec![Arc::clone(&numeric), Arc::clone(&textual)],
            ..Default::default()
        },
        ..Default::default()
    });
    agent
        .state()
        .write()
        .unwrap()
        .set_tools(vec![Arc::clone(&textual), Arc::clone(&numeric)]);
    let live = agent.state().read().unwrap();
    let tools = live.tools().read().unwrap();
    assert!(Arc::ptr_eq(&tools[0], &textual) && Arc::ptr_eq(&tools[1], &numeric));
    let first = tools[0].read().unwrap();
    assert!(first.downcast_ref::<u32, String>().is_none());
    assert_eq!(
        first.downcast_ref::<String, Vec<u8>>().unwrap().label,
        "Textual"
    );
    let second = tools[1].read().unwrap();
    assert_eq!(
        second.downcast_ref::<u32, String>().unwrap().label,
        "Numeric"
    );
}
/// Parse the admitted tool scheduling preferences.
fn tool_mode(text: &str) -> ToolExecutionMode {
    match text {
        "sequential" => ToolExecutionMode::Sequential,
        "parallel" => ToolExecutionMode::Parallel,
        _ => panic!("unknown tool mode"),
    }
}
/// A non-Clone caller-defined message with a destruction observer.
struct Widget {
    /// Open caller-selected role.
    role: String,
    /// Mutable typed payload.
    text: String,
    /// Optional synchronous lifetime observer.
    on_drop: Option<Box<dyn Fn() + Send + Sync>>,
}
impl CustomAgentMessages for Widget {
    fn role(&self) -> &str {
        &self.role
    }
}
impl Drop for Widget {
    fn drop(&mut self) {
        if let Some(observer) = &self.on_drop {
            observer();
        }
    }
}
/// Typed custom messages preserve role and mutable payload without cloning data.
#[test]
fn maestro_custom_messages_keep_typed_payload_identity() {
    for mut case in cases("maestro_custom_messages_keep_typed_payload_identity") {
        let entry = Arc::new(RwLock::new(Widget {
            role: take(&mut case.input, "role"),
            text: "retained".into(),
            on_drop: None,
        }));
        let agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                messages: vec![AgentMessage::Custom(Arc::clone(&entry)).clone()],
                ..Default::default()
            },
            ..Default::default()
        });
        entry.write().unwrap().text = take(&mut case.input, "text");
        let live = agent.state().read().unwrap();
        let collection = live.messages().read().unwrap();
        let AgentMessage::Custom(stored) = &collection[0] else {
            panic!("expected custom")
        };
        let payload = stored.read().unwrap();
        check(
            case,
            json!({"sameEntry":Arc::ptr_eq(stored,&entry),
            "messages":[{"role":payload.role(),"payload":{"text":payload.text}}]}),
        );
    }
}
/// Construct a standard variant independently of its recorded final result.
fn standard(role: &str) -> AgentMessage {
    match role {
        "assistant" => AgentMessage::Assistant(Arc::new(RwLock::new(serde_json::from_value(json!({
            "role":"assistant","content":[{"type":"text","text":"before"}],
            "api":"fixture","provider":"fixture","model":"fixture",
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
                "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
            "stopReason":"stop","timestamp":123,
        })).unwrap()))),
        "toolResult" => AgentMessage::ToolResult(Arc::new(RwLock::new(serde_json::from_value(json!({
            "role":"toolResult","toolCallId":"call","toolName":"echo",
            "content":[{"type":"text","text":"before"}],"details":{"kept":1},"isError":false,"timestamp":123,
        })).unwrap()))),
        _ => panic!("unknown standard role"),
    }
}
/// Assistant and tool-result variants retain later changes through the actual handles.
#[test]
fn maestro_standard_message_variants_remain_shared() {
    for mut case in cases("maestro_standard_message_variants_remain_shared") {
        let entry = standard(&take::<String>(&mut case.input, "role"));
        let agent: Agent = Agent::new(AgentOptions {
            initial_state: AgentInitialState {
                messages: vec![entry.clone()],
                ..Default::default()
            },
            ..Default::default()
        });
        let text: String = take(&mut case.input, "text");
        match &entry {
            AgentMessage::Assistant(record) => {
                let mut record = record.write().unwrap();
                let maestro_models::AssistantContent::Text(content) = &mut record.content[0] else {
                    panic!("expected text")
                };
                content.text = text;
            }
            AgentMessage::ToolResult(record) => {
                let mut record = record.write().unwrap();
                let maestro_models::UserBlock::Text(content) = &mut record.content[0] else {
                    panic!("expected text")
                };
                content.text = text;
            }
            _ => panic!("expected assistant or tool result"),
        }
        let live = agent.state().read().unwrap();
        let collection = live.messages().read().unwrap();
        let same = match (&entry, &collection[0]) {
            (AgentMessage::Assistant(a), AgentMessage::Assistant(b)) => Arc::ptr_eq(a, b),
            (AgentMessage::ToolResult(a), AgentMessage::ToolResult(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        check(
            case,
            json!({"sameEntry":same,"messages":messages(&collection)}),
        );
    }
}
/// Assignment isolates outer storage while retaining entry handles.
#[test]
fn maestro_assignment_moves_outer_storage_not_entries() {
    for mut case in cases("maestro_assignment_moves_outer_storage_not_entries") {
        let mut supplied = vec![user(&take::<String>(&mut case.input, "text"))];
        let agent: Agent = Agent::default();
        agent
            .state()
            .write()
            .unwrap()
            .set_messages(supplied.clone());
        let stored = Arc::clone(agent.state().read().unwrap().messages());
        supplied.push(user(&take::<String>(&mut case.input, "outside")));
        edit_user(&supplied[0], take(&mut case.input, "edit"));
        check(
            case,
            json!({"copiedOuter":stored.read().unwrap().len()!=supplied.len(),"messages":messages(&stored.read().unwrap())}),
        );
    }
}
/// Last-alias release is synchronous and Agent-owned state guards are released first.
#[test]
fn maestro_history_releases_payload_after_last_alias() {
    let payload = Arc::new(RwLock::new(Widget {
        role: "owned".into(),
        text: "payload".into(),
        on_drop: None,
    }));
    let weak = Arc::downgrade(&payload);
    let agent = Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            messages: vec![AgentMessage::Custom(payload)],
            ..Default::default()
        },
        ..Default::default()
    });
    let old = Arc::clone(agent.state().read().unwrap().messages());
    agent.state().write().unwrap().set_messages(Vec::new());
    assert!(weak.upgrade().is_some());
    let captured_entry = old.write().unwrap().pop().unwrap();
    drop(old);
    assert!(weak.upgrade().is_some());
    drop(captured_entry);
    assert!(weak.upgrade().is_none(), "last alias must release payload");
    let payload = Arc::new(RwLock::new(Widget {
        role: "owned".into(),
        text: "unaliased".into(),
        on_drop: None,
    }));
    let weak = Arc::downgrade(&payload);
    let unaliased = Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            messages: vec![AgentMessage::Custom(payload)],
            ..Default::default()
        },
        ..Default::default()
    });
    unaliased.state().write().unwrap().set_messages(Vec::new());
    assert!(
        weak.upgrade().is_none(),
        "unaliased initial history must release payload"
    );
    assert_sole_owned_releases();
}
/// Sole-owned slot and queue payloads release at the documented guard boundary.
fn assert_sole_owned_releases() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let agent: Arc<Agent<Widget>> = Arc::new(Agent::default());
    for action in 0..5 {
        let observed = Arc::new(AtomicBool::new(false));
        let witness = Arc::clone(&observed);
        let owner = Arc::downgrade(&agent);
        let payload = Arc::new(RwLock::new(Widget {
            role: "probe".into(),
            text: "probe".into(),
            on_drop: Some(Box::new(move || {
                let owner = owner.upgrade().unwrap();
                if action != 0 {
                    assert!(
                        owner.state().try_write().is_ok(),
                        "Agent-owned state guard must be released"
                    );
                }
                witness.store(true, Ordering::SeqCst);
            })),
        }));
        let weak = Arc::downgrade(&payload);
        let entry = AgentMessage::Custom(payload);
        match action {
            0 => {
                agent.state().write().unwrap().set_messages(vec![entry]);
                agent.state().write().unwrap().set_messages(Vec::new());
            }
            1 => {
                agent.steer(entry);
                agent.clear_steering_queue();
            }
            2 => {
                agent.follow_up(entry);
                agent.clear_follow_up_queue();
            }
            3 => {
                agent.state().write().unwrap().set_messages(vec![entry]);
                agent.reset();
            }
            _ => {
                agent.follow_up(entry);
                agent.reset();
            }
        }
        assert!(
            observed.load(Ordering::SeqCst),
            "drop effect must complete synchronously"
        );
        assert!(
            weak.upgrade().is_none(),
            "released slot must not retain payload"
        );
    }
}

/// Initialization and assignment preserve distinguishable tool order and repeated handles.
fn assert_tool_order_and_duplicates() {
    let zulu = tool("zulu", "Zulu", None).0;
    let alpha = tool("alpha", "Alpha", None).0;
    let entries = vec![Arc::clone(&zulu), alpha, zulu];
    let agent: Agent = Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            tools: entries.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    for assignment in [false, true] {
        if assignment {
            agent.state().write().unwrap().set_tools(entries.clone());
        }
        let live = agent.state().read().unwrap();
        let collection = live.tools().read().unwrap();
        let names: Vec<_> = collection
            .iter()
            .map(|entry| {
                let guard = entry.read().unwrap();
                guard
                    .downcast_ref::<Value, Value>()
                    .unwrap()
                    .definition
                    .name
                    .clone()
            })
            .collect();
        assert_eq!(names, ["zulu", "alpha", "zulu"]);
        assert!(Arc::ptr_eq(&collection[0], &collection[2]));
    }
}

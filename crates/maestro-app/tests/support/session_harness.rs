//! What the application's own tests add to the session harness: the recorded corpus a
//! session is compared with, recorded views, and extensions and tools for single cases.
#![cfg(not(target_arch = "wasm32"))]
use indexmap::IndexMap;
use maestro_app::agent_session::ToolDefinitionHandle;
use maestro_app::test_support::{
    Definition, HarnessOptions, TS, assistant, harness, message_text, recorded_number,
    session_event_view, tool_definition,
};
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_extensions::NativeAgentMessage;
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ExtensionAPI, ExtensionExecution,
    ExtensionFactory, ExtensionRunner, InputEvent, InputEventResult, RegisteredToolInput,
    SessionBeforeCompactEvent, SessionBeforeCompactResult,
};
use maestro_models::{DiagnosticErrorInfo, Local, Model, Tool, UserContent};
use maestro_models_registry::ScopedModel;
use maestro_resources::{SyntheticSourceOptions, create_synthetic_source_info};
use maestro_session::{
    CompactionResult, Presence, SessionManager,
    message_records::{CustomAgentMessages, CustomMessageInput},
};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// An extension that summarizes every compaction as `summary` instead of calling a provider.
pub fn summary_provider(summary: &'static str) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                Box::pin(std::future::ready(Ok(Some(summary_result(event, summary)))))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// A supplied summary retaining the compaction preparation's boundary and token count.
pub fn summary_result(
    event: &SessionBeforeCompactEvent,
    summary: &str,
) -> SessionBeforeCompactResult {
    SessionBeforeCompactResult {
        cancel: Presence::Missing,
        compaction: Presence::Present(CompactionResult {
            summary: summary.to_owned(),
            first_kept_entry_id: event.data.preparation.first_kept_entry_id.clone(),
            tokens_before: event.data.preparation.tokens_before,
            details: Presence::Present(json!({})),
        }),
    }
}

/// The recorded corpus file `name`.
pub fn corpus(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/agent_session/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The recorded case `name` of the corpus file `file`.
pub fn case(file: &str, name: &str) -> Value {
    let cases = corpus(file);
    let found = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name);
    found
        .unwrap_or_else(|| panic!("no recorded case {name}"))
        .clone()
}

/// The recorded prompt and queue case `name`.
pub fn expected(name: &str) -> Value {
    corpus("prompt_queue")[name].clone()
}

/// The recorded case `name` of the keyed corpus file `file`.
pub fn recorded(file: &str, name: &str) -> Value {
    let cases = corpus(file);
    cases
        .get(name)
        .unwrap_or_else(|| panic!("no recorded case {name}"))
        .clone()
}

/// A session and the owners the tests observe, over its own disposable root.
pub type Harness = maestro_app::test_support::Harness<maestro_test_directory::Scratch>;

/// What the application's own tests read from a harness beside its fields.
pub trait HarnessExt {
    /// The extensions' runner now in effect.
    fn runner(&self) -> Rc<ExtensionRunner<Local>>;

    /// The scripted model `id` renamed to `name` with a thinking map.
    fn variant(&self, id: &str, name: &str, map: Value) -> Arc<RwLock<Model>>;

    /// The model `id` under another provider, which has no credentials.
    fn elsewhere(&self, id: &str) -> Arc<RwLock<Model>>;

    /// A scoped entry for `model` with an optional explicit level.
    fn scoped(model: Arc<RwLock<Model>>, level: Option<&str>) -> ScopedModel;
}

impl HarnessExt for Harness {
    fn runner(&self) -> Rc<ExtensionRunner<Local>> {
        self.session.extension_runner()
    }

    fn variant(&self, id: &str, name: &str, map: Value) -> Arc<RwLock<Model>> {
        let mut model = serde_json::to_value(self.faux.get_model(Some(id)).unwrap()).unwrap();
        model["id"] = json!(name);
        model["name"] = json!(name);
        model["thinkingLevelMap"] = map;
        Arc::new(RwLock::new(serde_json::from_value(model).unwrap()))
    }

    fn elsewhere(&self, id: &str) -> Arc<RwLock<Model>> {
        let model = self.model(id).unwrap();
        model.write().unwrap().provider = "elsewhere".into();
        model.write().unwrap().id = "x".into();
        model
    }

    fn scoped(model: Arc<RwLock<Model>>, level: Option<&str>) -> ScopedModel {
        ScopedModel {
            model,
            thinking_level: level.map(|level| serde_json::from_value(json!(level)).unwrap()),
        }
    }
}

/// `provider/id` of a model.
pub fn model_view(model: &Arc<RwLock<Model>>) -> String {
    let model = model.read().unwrap();
    format!("{}/{}", model.provider, model.id)
}

/// `definition` retained as an SDK tool definition.
pub fn sdk_definition(definition: Definition) -> ToolDefinitionHandle {
    Local::retain_definition(RegisteredToolInput::from(definition))
}

/// The recorded tools: whitespace-heavy snippets, blank and repeated guidelines.
pub fn sdk_tools() -> Vec<ToolDefinitionHandle> {
    [
        tool_definition(
            "alpha",
            Some("  Read\n  files\r\n quickly  "),
            Some(&["  Prefer read ", "", "Prefer read", " Keep it short", "\t"]),
        ),
        tool_definition("beta", Some("   "), None),
        tool_definition("gamma", Some("\u{feff}x\u{85}y"), Some(&["Use gamma"])),
        tool_definition("delta", None, Some(&[])),
    ]
    .into_iter()
    .map(sdk_definition)
    .collect()
}

/// A user message record.
pub fn user(content: &Value, offset: u64) -> Value {
    json!({"role": "user", "content": content, "timestamp": TS + offset})
}

/// Stamps `message` at the compaction summary of `manager`'s context.
///
/// The message then counts as given after the compaction without reading the clock.
///
/// # Panics
/// Panics when the session context cannot be built or holds no compaction summary.
pub fn after_summary(manager: &SessionManager, mut message: Value) -> Value {
    let context = manager.build_session_context().unwrap();
    let summary = context
        .messages
        .iter()
        .find(|message| message["role"] == "compactionSummary")
        .expect("a compaction summary in context");
    message["timestamp"] = summary["timestamp"].clone();
    message
}

/// Tree entry identities by name.
pub type Names = IndexMap<String, String>;

/// The recorded tree: two branches from `a1`, the first ending in a custom message.
pub fn build_tree(manager: &mut SessionManager) -> Names {
    let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
    let mut names = Names::new();
    let mut add = |name: &str, id: String| names.insert(name.to_owned(), id);
    add(
        "u1",
        manager.append_message(user(&json!("first"), 1)).unwrap(),
    );
    add(
        "a1",
        manager
            .append_message(assistant("answer one", 2, &json!({})))
            .unwrap(),
    );
    let second =
        json!([{"type": "text", "text": "second"}, {"type": "text", "text": " part"}, image]);
    add("u2", manager.append_message(user(&second, 3)).unwrap());
    add(
        "a2",
        manager
            .append_message(assistant("answer two", 4, &json!({})))
            .unwrap(),
    );
    let content =
        json!([{"type": "text", "text": "custom "}, image, {"type": "text", "text": "words"}]);
    let content = serde_json::from_value(content).unwrap();
    let custom = manager.append_custom_message_entry("note", content, true, Presence::Missing);
    add("c1", custom.unwrap());
    manager.branch(&names["a1"]).unwrap();
    names.insert(
        "u3".into(),
        manager.append_message(user(&json!("alt"), 5)).unwrap(),
    );
    names.insert(
        "a3".into(),
        manager
            .append_message(assistant("answer alt", 6, &json!({})))
            .unwrap(),
    );
    names
}

/// The text a message shows: string content unchanged, or its text blocks in order joined
/// by one newline each; anything without content shows nothing, not its summary.
pub fn get_message_text(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .map(|block| block["text"].as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The text of each user message in the session history, in order.
pub fn get_user_texts(harness: &Harness) -> Vec<String> {
    texts_matching(harness, |message| {
        matches!(message, maestro_agent::AgentMessage::User(_))
    })
}

/// The text of each assistant message in the session history, in order.
pub fn get_assistant_texts(harness: &Harness) -> Vec<String> {
    texts_matching(harness, |message| {
        matches!(message, maestro_agent::AgentMessage::Assistant(_))
    })
}

/// The message fields the prompt and queue checks compare: the role and text, a custom
/// message's own fields, a tool result's error flag and, for a message with images, its
/// content blocks in order.
pub fn delivery_message_view(record: &Value) -> Value {
    let mut view = json!({"role": record["role"], "text": get_message_text(record)});
    if record["role"] == "custom" {
        for key in ["customType", "display", "details"] {
            view[key] = record[key].clone();
        }
    }
    if record["role"] == "toolResult" {
        view["isError"] = record["isError"].clone();
    }
    if let Some(content) = record["content"].as_array()
        && content.iter().any(|part| part["type"] == "image")
    {
        view["content"] = json!(content);
    }
    view
}

/// The delivered history and queue mirrors of `h`.
pub fn delivery_view(h: &Harness) -> Value {
    let messages: Vec<Value> = {
        let messages = h.session.messages();
        let messages = messages.read().unwrap();
        messages
            .iter()
            .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
            .collect()
    };
    json!({
        "users": get_user_texts(h),
        "assistants": get_assistant_texts(h),
        "messages": messages,
        "pending": h.session.pending_message_count(),
        "steering": h.session.get_steering_messages(),
        "followUp": h.session.get_follow_up_messages(),
    })
}

/// The contexts the scripted stream received, as delivery messages, in request order.
pub fn delivery_requests(h: &Harness) -> Value {
    let requests: Vec<Vec<Value>> = h
        .requests
        .0
        .borrow()
        .iter()
        .map(|context| {
            context
                .messages
                .iter()
                .map(|message| delivery_message_view(&serde_json::to_value(message).unwrap()))
                .collect()
        })
        .collect();
    json!(requests)
}

/// The text of each history message `keep` selects, in order.
fn texts_matching(harness: &Harness, keep: fn(&NativeAgentMessage) -> bool) -> Vec<String> {
    let messages = harness.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .filter(|message| keep(message))
        .map(|message| get_message_text(&serde_json::to_value(message).unwrap()))
        .collect()
}

/// Replaces the agent's live history with the stored history.
pub fn load_history(h: &Harness) {
    let records = h
        .session
        .session_manager()
        .borrow()
        .build_session_context()
        .unwrap()
        .messages;
    let history: Vec<NativeAgentMessage> = serde_json::from_value(json!(records)).unwrap();
    h.session
        .agent()
        .state()
        .write()
        .unwrap()
        .set_messages(history);
}

/// The agent history as roles and texts.
pub fn history_view(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| {
            let record = serde_json::to_value(message).unwrap();
            json!({"role": record["role"], "text": message_text(&record)})
        })
        .collect()
}

/// Whether code running now could change `session`: its history, its settings and its agent
/// state are all free to borrow mutably.
pub fn free_to_change(session: &AgentSession<Local>) -> bool {
    session.session_manager().try_borrow_mut().is_ok()
        && session.settings_manager().try_borrow_mut().is_ok()
        && session.state().try_write().is_ok()
}

/// What the prompt, queue, message and shell cases record beside the session.
#[derive(Clone, Default)]
pub struct Transcript {
    /// Input and before-agent-start events as the recording extension saw them.
    pub extension: Rc<RefCell<Vec<Value>>>,
    /// Failures reported to the extension error listeners.
    pub errors: Rc<RefCell<Vec<Value>>>,
    /// Agent run and message events, and the session's own message and queue
    /// announcements, in arrival order.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// The base system prompt, recorded as `$BASE`.
    pub base: Rc<RefCell<String>>,
}

impl Transcript {
    /// The recorded events of `kind`, in order.
    pub fn events_of(&self, kinds: &[&str]) -> Vec<Value> {
        of_kinds(&self.events.borrow(), kinds)
    }
}

/// The values whose `type` is one of `kinds`, in order.
pub fn of_kinds(values: &[Value], kinds: &[&str]) -> Vec<Value> {
    values
        .iter()
        .filter(|value| kinds.iter().any(|kind| value["type"] == *kind))
        .cloned()
        .collect()
}

/// The input sources the session's handlers saw, in order.
pub fn input_sources(transcript: &Transcript) -> Vec<Value> {
    of_kinds(&transcript.extension.borrow(), &["input"])
        .iter()
        .map(|event| event["source"].clone())
        .collect()
}

/// The extension that records input and before-agent-start events into `transcript`,
/// loaded before the case's own extensions.
pub fn recording_extension(transcript: &Transcript) -> ExtensionFactory<Local> {
    let transcript = transcript.clone();
    Rc::new(move |api| {
        let inputs = Rc::clone(&transcript.extension);
        let starts = Rc::clone(&transcript.extension);
        let base = Rc::clone(&transcript.base);
        let input = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                inputs.borrow_mut().push(json!({
                    "type": "input", "text": event.text, "images": image_count(&event.images),
                    "source": event.source,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let start = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |event, _| {
                let prompt = if event.system_prompt == *base.borrow() {
                    "$BASE".to_owned()
                } else {
                    event.system_prompt.clone()
                };
                starts.borrow_mut().push(json!({
                    "type": "before_agent_start", "prompt": event.prompt,
                    "images": image_count(&event.images), "systemPrompt": prompt,
                }));
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        let registered = input.and(start).map(drop);
        Box::pin(std::future::ready(registered))
    })
}

/// The number of supplied images, or null when none were supplied.
fn image_count(images: &Presence<Vec<maestro_models::ImageContent>>) -> Value {
    match images {
        Presence::Present(images) => json!(images.len()),
        _ => Value::Null,
    }
}

/// A session over `options` in a fresh disposable root, recorded as [`transcribed_in`]
/// records it.
pub async fn transcribed(options: HarnessOptions) -> (Harness, Transcript) {
    transcribed_in(Scratch::new().unwrap(), options).await
}

/// A session over `options` below `scratch` with the recording extension loaded first, and
/// its base system prompt, errors, agent events and session announcements recorded.
pub async fn transcribed_in(scratch: Scratch, options: HarnessOptions) -> (Harness, Transcript) {
    let transcript = Transcript::default();
    let mut factories = vec![recording_extension(&transcript)];
    factories.extend(options.factories);
    let h = harness(
        scratch,
        HarnessOptions {
            factories,
            ..options
        },
    )
    .await
    .unwrap();
    *transcript.base.borrow_mut() = h.session.system_prompt();
    let errors = Rc::clone(&transcript.errors);
    let _unsubscribe = h.runner().on_error(Rc::new(move |error| {
        errors.borrow_mut().push(json!({
            "extensionPath": error.extension_path, "event": error.event, "error": error.error,
        }));
        Ok(())
    }));
    let events = Rc::clone(&transcript.events);
    let _unsubscribe = h.session.subscribe(Rc::new(move |event| match event {
        AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } => events.borrow_mut().push(json!({
            "type": "queue_update", "steering": steering, "followUp": follow_up,
        })),
        AgentSessionEvent::Agent(event) => {
            events
                .borrow_mut()
                .push(agent_event_view(&event.read().unwrap()));
        }
        _ => {}
    }));
    (h, transcript)
}

/// The recorded form of an agent event: run boundaries by type, message events with the
/// message.
fn agent_event_view(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> Value {
    use maestro_agent::AgentEvent;
    match event {
        AgentEvent::AgentStart => json!({"type": "agent_start"}),
        AgentEvent::AgentEnd { .. } => json!({"type": "agent_end"}),
        AgentEvent::MessageStart { message } => {
            json!({"type": "message_start", "message": message_view(&serde_json::to_value(message).unwrap())})
        }
        AgentEvent::MessageEnd { message } => {
            json!({"type": "message_end", "message": message_view(&serde_json::to_value(message).unwrap())})
        }
        _ => json!({"type": "other"}),
    }
}

/// The recorded form of message content: plain text, or blocks with text or image type.
fn content_view(content: &Value) -> Value {
    match content {
        Value::String(text) => json!({"text": text}),
        Value::Array(blocks) => {
            json!({"blocks": blocks.iter().map(|block| if block["type"] == "text" {
            json!({"type": "text", "text": block["text"]})
        } else {
            json!({"type": block["type"], "mimeType": block["mimeType"]})
        }).collect::<Vec<_>>()})
        }
        _ => json!({"absent": true}),
    }
}

/// A recorded field: the value, or null when absent.
fn or_null(record: &Value, field: &str) -> Value {
    record.get(field).cloned().unwrap_or(Value::Null)
}

/// The recorded form of a message record.
pub fn message_view(record: &Value) -> Value {
    match record["role"].as_str() {
        Some("user") => json!({"role": "user", "content": content_view(&record["content"])}),
        Some("assistant") => json!({
            "role": "assistant", "text": message_text(record), "stopReason": record["stopReason"],
            "errorMessage": or_null(record, "errorMessage"),
        }),
        Some("custom") => json!({
            "role": "custom", "customType": record["customType"],
            "content": content_view(&record["content"]), "display": record["display"],
            "details": or_null(record, "details"),
        }),
        Some("bashExecution") => json!({
            "role": "bashExecution", "command": record["command"], "output": record["output"],
            "exitCode": record["exitCode"].as_f64().map_or(Value::Null, recorded_number),
            "cancelled": record["cancelled"],
            "truncated": record["truncated"],
            "fullOutputPath": record.get("fullOutputPath").map(|_| json!({"present": true})),
            "excludeFromContext": or_null(record, "excludeFromContext"),
        }),
        _ => json!({"role": record["role"], "summary": or_null(record, "summary")}),
    }
}

/// The agent history in its recorded form.
pub fn messages_view(h: &Harness) -> Vec<Value> {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .map(|message| message_view(&serde_json::to_value(message).unwrap()))
        .collect()
}

/// The stored session entries in their recorded form, numbered by position.
pub fn entries_view(h: &Harness) -> Vec<Value> {
    use maestro_session::session_manager::ReadonlySessionManager;
    let manager = h.session.session_manager().borrow();
    manager
        .get_entries()
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut view = json!({"type": entry["type"], "id": format!("e{index}")});
            if entry["type"] == "message" {
                view["message"] = message_view(&entry["message"]);
            } else if entry["type"] == "custom_message" {
                view["customType"] = entry["customType"].clone();
                view["content"] = content_view(&entry["content"]);
                view["display"] = entry["display"].clone();
                view["details"] = or_null(entry, "details");
            }
            view
        })
        .collect()
}

/// The queue mirrors, streaming flag and system prompt in their recorded form.
pub fn queue_view(h: &Harness, transcript: &Transcript) -> Value {
    let prompt = h.session.system_prompt();
    json!({
        "steering": h.session.get_steering_messages(),
        "followUp": h.session.get_follow_up_messages(),
        "pendingMessageCount": h.session.pending_message_count(),
        "isStreaming": h.session.is_streaming(),
        "systemPrompt": if prompt == *transcript.base.borrow() { "$BASE".to_owned() } else { prompt },
    })
}

/// `value` with every `(from, to)` spelling replaced, in order, in its strings.
pub fn substituting(value: Value, paths: &[(&str, &str)]) -> Value {
    mapping_strings(value, &|text| {
        paths
            .iter()
            .fold(text, |text, (from, to)| text.replace(from, to))
    })
}

/// `value` with `map` applied to each of its strings.
fn mapping_strings(value: Value, map: &impl Fn(String) -> String) -> Value {
    match value {
        Value::String(text) => json!(map(text)),
        Value::Array(items) => items
            .into_iter()
            .map(|item| mapping_strings(item, map))
            .collect(),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, item)| (key, mapping_strings(item, map)))
                .collect(),
        ),
        other => other,
    }
}

/// `value` with each `(path, placeholder)` path spelled as its placeholder, as the recorded
/// cases spell it. On Windows a string that names a placeholder also spells its native
/// separators `/`, as the recordings do.
pub fn as_recorded(value: Value, paths: &[(&str, &str)]) -> Value {
    mapping_strings(value, &|text| {
        let text = paths.iter().fold(text, |text, (path, placeholder)| {
            text.replace(path, placeholder)
        });
        let named = paths
            .iter()
            .any(|(_, placeholder)| text.contains(placeholder));
        if cfg!(windows) && named {
            text.replace('\\', "/")
        } else {
            text
        }
    })
}

/// `value` with the scratch root spelled `$ROOT`, as the recorded cases spell it.
pub fn with_placeholders(value: Value, h: &Harness) -> Value {
    let root = h.scratch.path().display().to_string();
    as_recorded(value, &[(&root, "$ROOT")])
}

/// The agent run and message events, as the session's listeners would see them.
pub const MESSAGE_EVENTS: [&str; 4] = ["agent_start", "message_start", "message_end", "agent_end"];

/// Asserts what the session produced against `case`: the history, the run and message
/// events, the extension events, the reported errors and the number of provider requests.
/// A run end's messages are compared through the history, since their timestamps are the
/// clock's.
pub fn assert_transcript(h: &Harness, t: &Transcript, case: &Value) {
    assert_eq!(
        with_placeholders(json!(messages_view(h)), h),
        case["messages"]
    );
    let recorded = of_kinds(case["sessionEvents"].as_array().unwrap(), &MESSAGE_EVENTS);
    assert_eq!(
        with_placeholders(json!(t.events_of(&MESSAGE_EVENTS)), h),
        json!(recorded)
    );
    assert_eq!(
        with_placeholders(json!(*t.extension.borrow()), h),
        case["extensionEvents"]
    );
    assert_eq!(
        with_placeholders(json!(*t.errors.borrow()), h),
        case["errorEvents"]
    );
    assert_eq!(json!(h.faux.state.call_count()), case["contexts"]["calls"]);
}

/// The recorded project files: the `review` skill with frontmatter and a padded body, and
/// a `gone` skill whose file is missing.
pub fn recorded_skills(scratch: &maestro_test_directory::Scratch) -> Vec<maestro_resources::Skill> {
    let dir = scratch.path().join("project/skills/review");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("SKILL.md");
    std::fs::write(
        &file,
        "---\ndescription: Review\n---\n\n  Check the diff.  \n",
    )
    .unwrap();
    let base_dir = dir.display().to_string();
    let skill = |name: &str, file_path: String| maestro_resources::Skill {
        name: name.into(),
        description: "Review".into(),
        source_info: create_synthetic_source_info(
            file_path.clone(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
        file_path,
        base_dir: base_dir.clone(),
        disable_model_invocation: false,
    };
    vec![
        skill("review", file.display().to_string()),
        skill("gone", dir.join("missing.md").display().to_string()),
    ]
}

/// The recorded form of an operation's outcome.
pub fn outcome(result: Result<(), DiagnosticErrorInfo>) -> Value {
    match result {
        Ok(()) => json!({"ok": {"undefined": true}}),
        Err(error) => json!({"error": error.message}),
    }
}

/// The stored shell records of `entries` without their position, which also counts the
/// turn messages the agent-event persistence stores.
pub fn shell_entries(entries: &[Value]) -> Vec<Value> {
    entries
        .iter()
        .filter(|entry| entry["message"]["role"] == "bashExecution")
        .map(|entry| json!({"type": entry["type"], "message": entry["message"]}))
        .collect()
}

/// A labelled recorded observation.
pub fn labelled(label: &str, value: impl serde::Serialize) -> Value {
    json!({"label": label, "value": value})
}

/// Starts `text` as a prompt on the current task set.
pub fn start_prompt(
    h: &Harness,
    text: &'static str,
) -> tokio::task::JoinHandle<Result<(), DiagnosticErrorInfo>> {
    let session = Rc::clone(&h.session);
    tokio::task::spawn_local(async move {
        session
            .prompt(text, maestro_app::agent_session::PromptOptions::default())
            .await
    })
}

/// Prompt options whose preflight outcomes are collected in the returned list.
pub fn observed_preflight() -> (
    Rc<RefCell<Vec<bool>>>,
    maestro_app::agent_session::PromptOptions,
) {
    let accepted = Rc::new(RefCell::new(Vec::new()));
    let record = Rc::clone(&accepted);
    let options = maestro_app::agent_session::PromptOptions {
        preflight_result: Some(Rc::new(move |ok| record.borrow_mut().push(ok))),
        ..maestro_app::agent_session::PromptOptions::default()
    };
    (accepted, options)
}

/// Runs native tool cases in a checked child with disposable temporary and home directories.
/// Returns true when the parent has finished the child.
pub fn run_native_tool_child(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let root = maestro_test_directory::Scratch::new().unwrap();
    let mut child = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .nocapture()
        .temp_dir(root.path());
    for key in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME"] {
        child = child.env(key, root.path());
    }
    for key in [
        "BASH_ENV",
        "ENV",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
    ] {
        child = child.env_remove(key);
    }
    let output = child
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", root.path().join("empty-gitconfig"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

/// The recorded form of a shell result, with an artifact spelled `$ARTIFACT`.
pub fn bash_result_view(result: &maestro_tools::BashResult) -> Value {
    let mut view = json!({
        "output": result.output, "cancelled": result.cancelled, "truncated": result.truncated,
    });
    if let Some(code) = result.exit_code {
        view["exitCode"] = recorded_number(code);
    }
    if result.full_output_path.is_some() {
        view["fullOutputPath"] = json!("$ARTIFACT");
    }
    view
}

/// Notified once the session has delivered the end of an agent run to its listeners.
pub fn session_run_ended(h: &Harness) -> Rc<tokio::sync::Notify> {
    let ended = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && matches!(
                *event.read().unwrap(),
                maestro_agent::AgentEvent::AgentEnd { .. }
            )
        {
            notify.notify_one();
        }
    })));
    ended
}

/// Notified once an agent run ends.
pub fn agent_ended(h: &Harness) -> Rc<tokio::sync::Notify> {
    let ended = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        if matches!(
            *event.read().unwrap(),
            maestro_agent::AgentEvent::AgentEnd { .. }
        ) {
            notify.notify_one();
        }
        Box::pin(std::future::ready(Ok(())))
    })));
    ended
}

/// The retry observations among the delivered events, in the order they arrived:
/// `start:<attempt>` when a retry opens and `end:<success>` when it settles.
pub fn retry_order(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event["type"].as_str() {
            Some("auto_retry_start") => Some(format!("start:{}", event["attempt"])),
            Some("auto_retry_end") => Some(format!("end:{}", event["success"])),
            _ => None,
        })
        .collect()
}

/// Starts `operation` with one poll and drops it unfinished, as a caller that stops
/// observing it.
pub fn start_and_abandon(operation: impl std::future::Future) {
    let mut operation = std::pin::pin!(operation);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(operation.as_mut().poll(&mut context).is_pending());
}

/// A supplied failure carrying every diagnostic field.
pub fn supplied_failure() -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("RemoteError".into()),
        message: "connection reset".into(),
        stack: Some("RemoteError: connection reset\n    at remote".into()),
        code: Some(maestro_models::DiagnosticCode::Text("ECONNRESET".into())),
    }
}

/// The event type of an agent event, as the session's listeners name it.
pub fn agent_event_type(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> &'static str {
    use maestro_agent::AgentEvent;
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
    }
}

/// Every session event `session` delivers from now on, in order: agent events by type, with
/// a stream update's own type and delta, and the session's announcements in their recorded
/// form.
pub fn delivered_events(session: &AgentSession<Local>) -> Rc<RefCell<Vec<Value>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    drop(session.subscribe(Rc::new(move |event| {
        let view = match event {
            AgentSessionEvent::Agent(event) => Some(delivered_agent_event(&event.read().unwrap())),
            other => session_event_view(other),
        };
        recorded.borrow_mut().extend(view);
    })));
    events
}

/// The delivered form of an agent event.
fn delivered_agent_event(event: &maestro_agent::AgentEvent<CustomAgentMessages>) -> Value {
    let mut view = json!({"type": agent_event_type(event)});
    if let maestro_agent::AgentEvent::MessageUpdate {
        assistant_message_event,
        ..
    } = event
    {
        let update = serde_json::to_value(assistant_message_event).unwrap();
        view["update"] = update["type"].clone();
        view["delta"] = update.get("delta").cloned().unwrap_or(Value::Null);
    }
    view
}

/// A displayed custom `note` message that only records `text`.
pub fn custom_note(text: &str) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "note".into(),
        content: UserContent::Text(text.into()),
        display: true,
        details: Presence::Missing,
    }
}

/// An executable tool that reports its entry and answers `released` once released.
pub fn waiting_tool(
    started: Rc<tokio::sync::Notify>,
    release: Rc<tokio::sync::Notify>,
) -> maestro_agent::SharedAgentTool<Local> {
    text_tool("wait", "Wait for release", "Wait", "released", move || {
        started.notify_one();
        let release = Rc::clone(&release);
        Box::pin(async move { release.notified().await })
    })
}

/// An executable tool named `name` that calls `on_execute` and answers `ok`.
pub fn callback_tool(
    name: &str,
    on_execute: impl Fn() + 'static,
) -> maestro_agent::SharedAgentTool<Local> {
    text_tool(name, "Run a callback", name, "ok", move || {
        on_execute();
        Box::pin(std::future::ready(()))
    })
}

/// An executable tool with no parameters that awaits the work `run` starts, then answers
/// `text`.
fn text_tool(
    name: &str,
    description: &str,
    label: &str,
    text: &'static str,
    run: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + 'static,
) -> maestro_agent::SharedAgentTool<Local> {
    let tool = maestro_agent::AgentToolRecord::<Local> {
        definition: Tool {
            name: name.into(),
            description: description.into(),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        description: None,
        label: label.into(),
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _| {
            let work = run();
            Box::pin(async move {
                work.await;
                Ok(maestro_agent::AgentToolResult {
                    content: vec![maestro_models::UserBlock::Text(
                        maestro_models::TextContent::new(text.into()),
                    )],
                    details: Some(json!({})),
                    terminate: None,
                })
            })
        }),
        execution_mode: None,
    };
    Rc::new(RwLock::new(tool))
}

/// Notified once `session` announces the start of a retry.
pub fn retry_started(session: &AgentSession<Local>) -> Rc<tokio::sync::Notify> {
    let started = Rc::new(tokio::sync::Notify::new());
    let notify = Rc::clone(&started);
    drop(session.subscribe(Rc::new(move |event| {
        if matches!(event, AgentSessionEvent::AutoRetryStart { .. }) {
            notify.notify_one();
        }
    })));
    started
}

/// The extension whose before-agent-start handler answers every prompt with the system
/// prompt `prompt`.
pub fn system_prompt_extension(prompt: &'static str) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |_, _| {
                Box::pin(std::future::ready(Ok(Some(BeforeAgentStartEventResult {
                    message: Presence::Missing,
                    system_prompt: Presence::Present(prompt.to_owned()),
                }))))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// Prompts `text` and waits until the session has delivered the run's end.
pub async fn prompt_to_end(
    h: &Harness,
    text: &str,
    options: maestro_app::agent_session::PromptOptions,
) {
    let ended = session_run_ended(h);
    h.session.prompt(text, options).await.unwrap();
    h.session.agent().wait_for_idle().await;
    ended.notified().await;
}

/// The echo tool, in place of the built-in tools: it marks `executed` and answers `echoed`.
pub fn echo_tools(
    executed: Rc<RefCell<bool>>,
) -> IndexMap<String, maestro_agent::SharedAgentTool<Local>> {
    let tool = maestro_agent::AgentToolRecord::<Local> {
        definition: Tool {
            name: "echo".into(),
            description: "Echo back".into(),
            parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}})
                .into(),
        },
        description: None,
        label: "Echo".into(),
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _| {
            *executed.borrow_mut() = true;
            let echoed = maestro_models::TextContent::new("echoed".into());
            Box::pin(std::future::ready(Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(echoed)],
                details: Some(json!({})),
                terminate: None,
            })))
        }),
        execution_mode: None,
    };
    IndexMap::from([("echo".to_owned(), Rc::new(RwLock::new(tool)))])
}

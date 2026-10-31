//! The session-event subscriber: the terminal title, the thinking-level refresh, the working
//! row and the streamed assistant and tool rows.
use super::super::replay::aborted_notice;
use super::extension_ui::port;
use super::transcript::{
    Definition, assistant, call, history, kind_value, message, pending, recorded, registering,
    replay, text, tool, tool_params,
};
use super::{Setup, World, cases, isolated, started};
use crate::ToolExecutionComponent;
use maestro_app::configuration::config::APP_TITLE;
use maestro_app::messages::CustomAgentMessages;
use maestro_app::{
    AgentEvent, AgentMessage, AgentSessionEvent, AgentToolResult, AssistantMessageEvent,
    AssistantMessageEventStream, PromptOptions, ReadonlySessionManager, SessionManager,
    SharedAssistantMessage, TextContent, ToolCall, ToolRenderResult, UserBlock,
    WorkingIndicatorOptions,
};
use maestro_theme::ThemeColor;
use maestro_tui::{Component, KeybindingKeys};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
    time::Duration,
};

/// The session-event cases these witnesses drive.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_events.json");

/// The children `init` adds: seven containers and the footer.
const INITIALIZED_CHILDREN: usize = 8;

/// One recorded session event or the live state it reads.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// An event before or after initialization.
    Init {
        /// Whether the mode was initialized when the event arrived.
        initialized: bool,
    },
    /// The subscription through a real producer and its removal operation.
    Subscribe,
    /// A stored name over a working directory.
    Title {
        /// The stored working directory.
        cwd: String,
        /// The stored name; absent clears it.
        name: Option<String>,
    },
    /// One stored name field of every admitted kind.
    TitleDomain {
        /// Which name record the session file holds.
        kind: String,
        /// The stored name record.
        name: Value,
    },
    /// A thinking-level change while the editor holds ordinary or shell input.
    Level {
        /// Whether the editor holds a shell command.
        bash: bool,
    },
}

/// The recorded presentation after a case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Presentation {
    /// The transcript rows.
    chat: Vec<String>,
    /// The status rows.
    status: Vec<String>,
    /// The pending-message rows.
    #[serde(rename = "pendingRows")]
    pending_rows: Vec<String>,
    /// The unresolved tool calls.
    pending: Vec<String>,
    /// The editor history.
    history: Vec<String>,
    /// Whether an assistant row is streaming.
    streaming: bool,
    /// What a registered result renderer received, in call order: whether the details were
    /// missing, the details or null, whether the result streamed, whether the row failed and
    /// whether the details were negative zero.
    #[serde(default)]
    received: Option<Vec<Value>>,
}

/// The recorded observation of one case.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// The title written to the terminal.
    Title {
        /// The title text.
        title: String,
    },
    /// The failure a stored name outside the field domain causes.
    Failed {
        /// The failure category.
        #[serde(rename = "errorKind")]
        error_kind: String,
    },
    /// The rows and registrations shown.
    Shown(Presentation),
}

/// The collected emulated terminal effects.
type Effects = Rc<RefCell<Vec<String>>>;

/// Collects the effects the terminal is asked to emulate from now on; a title is emulated
/// rather than recorded as a frame write.
fn tap(world: &World) -> Effects {
    let effects: Effects = Rc::default();
    let collect = Rc::clone(&effects);
    world
        .terminal
        .set_tap(move |effect| collect.borrow_mut().push(effect.to_owned()));
    effects
}

/// The titles `effects` holds, with their terminal framing removed.
fn titles(effects: &Effects) -> Vec<String> {
    effects
        .borrow()
        .iter()
        .filter_map(|effect| {
            effect
                .strip_prefix("\u{1b}]0;")
                .and_then(|title| title.strip_suffix('\u{7}'))
                .map(str::to_owned)
        })
        .collect()
}

/// A session-metadata event whose own payload names no session.
fn session_info() -> AgentSessionEvent {
    AgentSessionEvent::SessionInfoChanged { name: None }
}

/// Replaces the session's manager with one whose newest name record holds `name`, absent when
/// the session file has no name record, over `cwd`.
fn install_name(world: &World, directory: &Path, cwd: &str, name: Option<&Value>) {
    let record = match name {
        Some(name) => format!("{{\"type\":\"session_info\",\"id\":\"info\",\"name\":{name}}}\n"),
        None => String::new(),
    };
    let path = directory.join("stored-name.jsonl");
    fs::write(
        &path,
        format!("{{\"type\":\"session\",\"id\":\"session\"}}\n{record}"),
    )
    .unwrap();
    let manager = Rc::clone(world.host.session().session_manager());
    *manager.borrow_mut() = SessionManager::open(path.to_str().unwrap(), None, Some(cwd)).unwrap();
}

/// Asserts the recorded transcript, status and pending rows, unresolved calls, editor history
/// and streaming row.
fn shown(world: &World, expected: &Presentation) {
    assert_eq!(world.mode.chat_container.render(100), expected.chat);
    assert_eq!(world.mode.status_container.render(100), expected.status);
    assert_eq!(
        world.mode.pending_messages_container.render(100),
        expected.pending_rows
    );
    assert_eq!(pending(world), expected.pending);
    assert_eq!(history(world), expected.history);
    assert_eq!(world.mode.streaming.borrow().is_some(), expected.streaming);
}

/// The editor's painted top border at width 20.
fn border(world: &World) -> String {
    world.mode.editor.render(20)[0].clone()
}

/// The top border painted by one of the published theme's roles.
fn painted(world: &World, role: &ThemeColor) -> String {
    world
        .themes
        .theme()
        .get()
        .unwrap()
        .fg(role, "─")
        .unwrap()
        .repeat(20)
}

/// Drives one event before or after initialization and a second one after it.
async fn initialization(world: &World, initialized: bool) {
    if initialized {
        world.mode.init().await.unwrap();
        world.runtime.settle().unwrap();
    }
    let effects = tap(world);
    world.mode.handle_event(&session_info()).unwrap();
    for work in world.runtime.take_futures() {
        work.await.unwrap();
    }
    world.runtime.settle().unwrap();
    assert!(world.mode.is_initialized.get());
    assert_eq!(
        world.mode.ui.children().borrow().len(),
        INITIALIZED_CHILDREN
    );
    // The uninitialized event initializes first, which writes the title before dispatch does.
    let after_first = if initialized { 1 } else { 2 };
    assert_eq!(titles(&effects).len(), after_first);
    world.mode.handle_event(&session_info()).unwrap();
    world.runtime.settle().unwrap();
    assert_eq!(titles(&effects).len(), after_first + 1);
    assert_eq!(
        world.mode.ui.children().borrow().len(),
        INITIALIZED_CHILDREN
    );
}

/// Drives the subscription, a second initialization, the removal operation and the release.
async fn subscription(world: World, unchanged_rows: &Presentation) {
    let session = world.host.session();
    let basename = maestro_path::basename(session.session_manager().borrow().get_cwd(), None);
    world.mode.init().await.unwrap();
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    let effects = tap(&world);
    // A second initialization adds no listener: one event writes one title.
    session.set_session_name("named").unwrap();
    assert_eq!(
        titles(&effects),
        vec![format!("{APP_TITLE} - named - {basename}")]
    );
    shown(&world, unchanged_rows);
    let removal = world.mode.unsubscribe.borrow_mut().take().unwrap();
    // The listener keeps no strong reference: the mode is released while it is still
    // subscribed, and the retained removal operation still detaches it afterwards.
    let released = Rc::downgrade(&world.mode);
    drop(world.mode);
    assert!(released.upgrade().is_none());
    removal();
    session.set_session_name("other").unwrap();
    assert_eq!(titles(&effects).len(), 1);
}

/// Calls the removal operation while the mode is alive: the next event writes no title.
async fn removal_while_alive(world: World) {
    let session = world.host.session();
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    let effects = tap(&world);
    let removal = world.mode.unsubscribe.borrow_mut().take().unwrap();
    removal();
    session.set_session_name("detached").unwrap();
    world.runtime.settle().unwrap();
    assert!(
        titles(&effects).is_empty(),
        "removal stops delivery to a live mode"
    );
}

#[test]
fn events_subscription_initializes_once_and_releases() {
    let test = "events_subscription_initializes_once_and_releases";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Expected::Shown(unchanged_rows) = expected else {
                panic!("a metadata case");
            };
            let world = Setup::default().build(directory);
            match input {
                Input::Init { initialized } => {
                    initialization(&world, initialized).await;
                    shown(&world, &unchanged_rows);
                }
                Input::Subscribe => {
                    subscription(world, &unchanged_rows).await;
                    removal_while_alive(Setup::default().build(directory)).await;
                }
                _ => panic!("a subscription case"),
            }
        }
    });
}

#[test]
fn events_title_uses_native_lexical_basename_and_optional_name() {
    let test = "events_title_uses_native_lexical_basename_and_optional_name";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::Title { cwd, name } = input else {
                panic!("a title case");
            };
            let Expected::Title { title } = expected else {
                panic!("a written title");
            };
            let world = started(directory).await;
            let stored = name.map(Value::String);
            install_name(&world, directory, &cwd, stored.as_ref());
            let effects = tap(&world);
            world.mode.handle_event(&session_info()).unwrap();
            world.runtime.settle().unwrap();
            assert_eq!(titles(&effects), vec![title]);
        }
    });
}

#[test]
fn events_title_keeps_the_session_name_producers_field_domain() {
    let test = "events_title_keeps_the_session_name_producers_field_domain";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::TitleDomain { kind, name } = input else {
                panic!("a stored-name case");
            };
            let world = started(directory).await;
            let stored = (kind != "missing").then_some(&name);
            install_name(&world, directory, "/work/project", stored);
            let effects = tap(&world);
            let outcome = world.mode.handle_event(&session_info());
            world.runtime.settle().unwrap();
            match expected {
                Expected::Title { title } => {
                    outcome.unwrap();
                    assert_eq!(titles(&effects), vec![title]);
                }
                Expected::Failed { error_kind } => {
                    let error = outcome.unwrap_err();
                    assert_eq!(error_kind, "invalid-session-name");
                    // The invalid stored name keeps its invalid-data category.
                    let invalid = error.downcast_ref::<std::io::Error>().unwrap();
                    assert_eq!(invalid.kind(), std::io::ErrorKind::InvalidData);
                    assert!(titles(&effects).is_empty());
                }
                Expected::Shown(_) => panic!("a stored-name observation"),
            }
        }
    });
}

#[test]
fn events_thinking_level_refreshes_footer_and_border() {
    let test = "events_thinking_level_refreshes_footer_and_border";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Input::Level { bash } = input else {
                panic!("a thinking-level case");
            };
            let Expected::Shown(unchanged_rows) = expected else {
                panic!("a metadata case");
            };
            let world = started(directory).await;
            let session = world.host.session();
            session.model().write().unwrap().reasoning = true;
            world.mode.is_bash_mode.set(bash);
            session
                .set_thinking_level(serde_json::from_value(json!("high")).unwrap())
                .unwrap();
            world.runtime.settle().unwrap();
            let (selected, sibling) = if bash {
                (ThemeColor::BashMode, ThemeColor::ThinkingHigh)
            } else {
                (ThemeColor::ThinkingHigh, ThemeColor::BashMode)
            };
            assert_eq!(border(&world), painted(&world, &selected));
            assert_ne!(border(&world), painted(&world, &sibling));
            // A stale payload does not repaint the border: the session's live level does.
            world
                .mode
                .handle_event(&AgentSessionEvent::ThinkingLevelChanged {
                    level: serde_json::from_value(json!("off")).unwrap(),
                })
                .unwrap();
            world.runtime.settle().unwrap();
            assert_eq!(border(&world), painted(&world, &selected));
            shown(&world, &unchanged_rows);
        }
    });
}

/// The queued texts a case starts with.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Queued {
    /// Session steering texts.
    steering: Vec<String>,
    /// Session follow-up texts.
    follow_up: Vec<String>,
}

/// A recorded working indicator.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordedIndicator {
    /// The frames.
    frames: Vec<String>,
    /// The interval the recording passed under a name the indicator does not read.
    #[serde(default)]
    interval: Option<f64>,
}

/// One recorded agent-event operation.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StreamOp {
    /// An agent start with the working row's visibility and message.
    Start {
        /// Whether the working row is shown.
        visible: bool,
        /// Whether terminal progress is shown.
        progress: bool,
        /// The custom working message, absent for the default.
        message: Option<String>,
    },
    /// A message start of `role`.
    MessageStart {
        /// The message role.
        role: String,
    },
    /// Two assistant updates, with or without a started stream.
    Update {
        /// Whether an assistant stream started first.
        active: bool,
    },
    /// A streamed message's end with one pending call.
    End {
        /// The stop reason.
        stop: String,
        /// The session's retry attempts.
        retry: u32,
        /// The error message, when recorded.
        error: Option<String>,
    },
    /// A tool's start, progress and end.
    Tool {
        /// Whether a replayed row of the call exists first.
        seeded: bool,
        /// Whether the end reports a failure.
        is_error: bool,
    },
    /// A progress or end event of an unregistered call.
    Unknown {
        /// `update` or `end`.
        phase: String,
    },
    /// An agent end over a working row and a streamed row.
    AgentEnd {
        /// Whether a working row is shown.
        loading: bool,
        /// Whether an assistant row is streaming.
        streaming: bool,
        /// Whether a shutdown was requested; only unrequested cases are recorded here.
        shutdown: bool,
    },
    /// An agent end with terminal progress hidden.
    AgentEndProgressDisabled {
        /// Whether a working row is shown.
        loading: bool,
    },
    /// A queue update whose payload differs from the session's queues.
    Queue {
        /// The session's queues.
        q: Queued,
    },
    /// A thinking toggle while an assistant row streams.
    ThinkingDefault {
        /// Whether thinking starts hidden.
        hide: bool,
    },
    /// A message end of `role` without a started stream.
    UnstartedEnd {
        /// The message role.
        role: String,
    },
    /// The working row's visibility set while a response streams or not.
    Visible {
        /// Whether a response streams.
        streaming: bool,
        /// The visibility set.
        visible: bool,
    },
    /// The working indicator set while the row runs.
    Indicator {
        /// The indicator set.
        indicator: RecordedIndicator,
    },
    /// The hidden-thinking label set before the thinking toggle while a row streams.
    Thinking {
        /// Whether thinking starts hidden.
        hide: bool,
        /// The label set, absent to restore the default.
        label: Option<String>,
    },
    /// A registered tool's progress and end with details of `kind`.
    Details {
        /// The JSON kind of the details, or `missing`.
        kind: String,
        /// Whether the end reports a failure.
        failed: bool,
    },
}

/// A working indicator of `frames`.
fn frames(frames: &[&str]) -> WorkingIndicatorOptions {
    WorkingIndicatorOptions {
        frames: Some(frames.iter().map(|frame| (*frame).to_owned()).collect()),
        interval_ms: None,
    }
}

/// Terminal progress shown.
const PROGRESS_ON: &str = "\u{1b}]9;4;3\u{7}";

/// Terminal progress cleared.
const PROGRESS_OFF: &str = "\u{1b}]9;4;0;\u{7}";

/// The progress effects `effects` holds, in order.
fn progress(effects: &Effects) -> Vec<String> {
    let effects = effects.borrow();
    let shown = effects
        .iter()
        .filter(|effect| effect.starts_with("\u{1b}]9;4;"));
    shown.cloned().collect()
}

/// A started mode over the recorded palette, with terminal progress shown when `progress`;
/// startup is quiet, so the transcript starts empty, as recorded.
async fn streamed(directory: &Path, progress: bool, setup: Setup) -> World {
    let configure = setup.configure;
    let world = recorded(
        directory,
        Setup {
            configure: Box::new(move |settings| {
                settings.set_quiet_startup(true);
                settings.set_show_terminal_progress(progress);
                configure(settings);
            }),
            ..setup
        },
    );
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// Delivers one agent event to the mode as its session would.
pub(super) fn dispatch(world: &World, event: AgentEvent<CustomAgentMessages>) {
    let event = AgentSessionEvent::Agent(Arc::new(RwLock::new(event)));
    world.mode.handle_event(&event).unwrap();
}

/// Draws the frames due now, leaving a working row's animation waiting.
fn drawn(world: &World) {
    for result in world.runtime.run_due() {
        result.unwrap();
    }
}

/// A shared assistant message with `content`, its stop reason and an optional error.
pub(super) fn shared(content: &Value, stop: &str, error: Option<&str>) -> SharedAssistantMessage {
    let AgentMessage::Assistant(shared) = message(assistant(content, stop, error)) else {
        panic!("an assistant record");
    };
    shared
}

/// Starts an assistant stream in `world` showing `content`.
pub(super) fn start(world: &World, content: &Value) {
    dispatch(
        world,
        AgentEvent::MessageStart {
            message: AgentMessage::Assistant(shared(content, "stop", None)),
        },
    );
}

/// An assistant update showing `message`.
fn update(message: &SharedAssistantMessage) -> AgentEvent<CustomAgentMessages> {
    AgentEvent::MessageUpdate {
        message: Arc::clone(message),
        assistant_message_event: AssistantMessageEvent::Start {
            partial: Arc::clone(message),
        },
    }
}

/// A live tool call `id` of `name` with `arguments`.
fn tool_call(id: &str, name: &str, arguments: &Value) -> ToolCall {
    serde_json::from_value(
        json!({"type": "toolCall", "id": id, "name": name, "arguments": arguments}),
    )
    .unwrap()
}

/// A tool result with one text block and `details`, absent when `None`.
fn tool_result(text: &str, details: Option<Value>) -> AgentToolResult {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text.into()))],
        details,
        usage: None,
        terminate: None,
    }
}

/// Registers a pending call `id` with no transcript row.
pub(super) fn stale(world: &World, id: &str) {
    let row = ToolExecutionComponent::new(tool_params(world, id)).unwrap();
    world
        .mode
        .pending_tools
        .borrow_mut()
        .insert(id.into(), Rc::new(row));
}

/// The record of a message start or end of `role`, as recorded.
fn role_record(role: &str, text: &str) -> Value {
    match role {
        "custom" => json!({"role": "custom", "customType": "note", "content": "CUSTOM",
            "display": true, "timestamp": 0}),
        "user" => json!({"role": "user", "content": "USER", "timestamp": 0}),
        "toolResult" => json!({"role": "toolResult", "content": [], "toolCallId": "none",
            "toolName": "unknown_tool", "isError": false, "timestamp": 0}),
        _ => assistant(&json!([{"type": "text", "text": text}]), "stop", None),
    }
}

#[test]
fn events_agent_start_selects_working_visibility_and_message() {
    let test = "events_agent_start_selects_working_visibility_and_message";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let StreamOp::Start {
                visible,
                progress: shown_progress,
                message,
            } = input
            else {
                panic!("an agent start");
            };
            let Expected::Shown(expected) = expected else {
                panic!("a presentation");
            };
            let world = streamed(directory, shown_progress, Setup::default()).await;
            if !visible || message.is_some() {
                // The recorded extension controls use a one-frame indicator.
                let ui = port(&world.mode);
                ui.set_working_indicator(Some(frames(&["FRAME"]))).unwrap();
                ui.set_working_message(message).unwrap();
                ui.set_working_visible(visible).unwrap();
            }
            stale(&world, "stale");
            let effects = tap(&world);
            // A second start replaces the first start's row.
            dispatch(&world, AgentEvent::AgentStart);
            dispatch(&world, AgentEvent::AgentStart);
            drawn(&world);
            let started = vec![PROGRESS_ON.to_owned(); 2];
            assert_eq!(
                progress(&effects),
                if shown_progress { started } else { Vec::new() }
            );
            shown(&world, &expected);
            assert_eq!(
                world.mode.working.loading_animation.borrow().is_some(),
                visible
            );
            // The live setting is read at each event, in both directions.
            let settings = Rc::clone(world.host.session().settings_manager());
            settings
                .borrow_mut()
                .set_show_terminal_progress(!shown_progress);
            dispatch(&world, AgentEvent::AgentStart);
            let now = usize::from(!shown_progress) + if shown_progress { 2 } else { 0 };
            assert_eq!(progress(&effects).len(), now);
        }
    });
}

#[test]
fn events_message_roles_use_one_retained_stream() {
    let test = "events_message_roles_use_one_retained_stream";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::MessageStart { role }, Expected::Shown(expected)) = (input, expected)
            else {
                panic!("a message start");
            };
            let world = streamed(directory, false, Setup::default()).await;
            let started = message(role_record(&role, "PARTIAL"));
            dispatch(
                &world,
                AgentEvent::MessageStart {
                    message: started.clone(),
                },
            );
            shown(&world, &expected);
            if let AgentMessage::Assistant(sent) = &started {
                let streaming = world.mode.streaming.borrow().clone().unwrap();
                assert!(Arc::ptr_eq(&streaming.1, sent));
                let children = world.mode.chat_container.children();
                let component: Rc<dyn Component> = streaming.0;
                assert!(Rc::ptr_eq(&children.borrow()[0], &component));
            }
        }
        // Only a user start rebuilds the pending rows from the session's queues.
        for (role, refreshed) in [("user", true), ("custom", false)] {
            let world = streamed(directory, false, Setup::default()).await;
            world.host.session().steer("queued", &[]).unwrap();
            world.mode.pending_messages_container.clear();
            dispatch(
                &world,
                AgentEvent::MessageStart {
                    message: message(role_record(role, "")),
                },
            );
            let rows = world.mode.pending_messages_container.render(100);
            assert_eq!(!rows.is_empty(), refreshed);
        }
    });
}

#[test]
fn events_updates_reuse_assistant_and_tool_rows() {
    let test = "events_updates_reuse_assistant_and_tool_rows";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Update { active }, Expected::Shown(expected)) = (input, expected) else {
                panic!("an update case");
            };
            let world = streamed(directory, false, Setup::default()).await;
            if active {
                let first = shared(&json!([{"type": "text", "text": "FIRST"}]), "stop", None);
                dispatch(
                    &world,
                    AgentEvent::MessageStart {
                        message: AgentMessage::Assistant(first),
                    },
                );
            }
            let (z, a) = (call("z", "unknown_tool"), call("a", "unknown_tool"));
            let second = json!([{"type": "text", "text": "SECOND"}, z, a]);
            dispatch(&world, update(&shared(&second, "toolUse", None)));
            let rows = world.mode.chat_container.children().borrow().clone();
            let mut changed = z.clone();
            changed["arguments"] = json!({"changed": 2});
            let third = shared(
                &json!([{"type": "text", "text": "THIRD"}, changed, a]),
                "toolUse",
                None,
            );
            dispatch(&world, update(&third));
            shown(&world, &expected);
            // The second update edits the rows the first one made, in place.
            let after = world.mode.chat_container.children().borrow().clone();
            assert_eq!(after.len(), rows.len());
            assert!(
                after
                    .iter()
                    .zip(&rows)
                    .all(|(after, row)| Rc::ptr_eq(after, row))
            );
            if active {
                let streaming = world.mode.streaming.borrow().clone().unwrap();
                assert!(Arc::ptr_eq(&streaming.1, &third));
            }
        }
    });
}

/// Streams one assistant message into `world` whose update holds one call `z`, then ends it
/// with `stop` and `error`, returning the ended message.
fn end_streamed(world: &World, stop: &str, error: Option<&str>) -> SharedAssistantMessage {
    start(world, &json!([]));
    let content = json!([call("z", "unknown_tool")]);
    dispatch(world, update(&shared(&content, "toolUse", None)));
    let ended = shared(&content, stop, error);
    dispatch(
        world,
        AgentEvent::MessageEnd {
            message: AgentMessage::Assistant(Arc::clone(&ended)),
        },
    );
    ended
}

#[test]
fn events_message_end_preserves_failure_and_completion_branches() {
    let test = "events_message_end_preserves_failure_and_completion_branches";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::End { stop, retry, error }, Expected::Shown(expected)) =
                (input, expected)
            else {
                panic!("a message end");
            };
            let world = streamed(directory, false, Setup::default()).await;
            let ended = if retry == 0 {
                end_streamed(&world, &stop, error.as_deref())
            } else {
                retried(&world, retry).await
            };
            let rows = world.mode.chat_container.render(100);
            assert!(rows.ends_with(&expected.chat));
            assert_eq!(world.mode.status_container.render(100), expected.status);
            assert_eq!(pending(&world), expected.pending);
            assert_eq!(history(&world), expected.history);
            assert_eq!(world.mode.streaming.borrow().is_some(), expected.streaming);
            if retry == 0 {
                shown(&world, &expected);
            }
            // An abort writes its notice into the same shared message.
            let written = ended.read().unwrap().error_message.clone();
            let notice = (stop == "aborted").then(|| aborted_notice(retry));
            assert_eq!(written, notice.or(error));
        }
    });
}

#[test]
fn events_message_end_ignores_user_and_unstarted_assistant() {
    let test = "events_message_end_ignores_user_and_unstarted_assistant";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::UnstartedEnd { role }, Expected::Shown(expected)) = (input, expected)
            else {
                panic!("an unstarted end");
            };
            let world = streamed(directory, false, Setup::default()).await;
            let record = if role == "assistant" {
                assistant(&json!([]), "stop", None)
            } else {
                role_record(&role, "")
            };
            dispatch(
                &world,
                AgentEvent::MessageEnd {
                    message: message(record),
                },
            );
            // A user end returns before requesting a frame; the others request one.
            assert_eq!(world.runtime.pending(), usize::from(role != "user"));
            world.runtime.settle().unwrap();
            shown(&world, &expected);
        }
    });
}

#[test]
fn events_tool_lifecycle_reuses_replayed_pending_rows() {
    let test = "events_tool_lifecycle_reuses_replayed_pending_rows";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Tool { seeded, is_error }, Expected::Shown(expected)) =
                (input, expected)
            else {
                panic!("a tool lifecycle");
            };
            let world = streamed(directory, false, Setup::default()).await;
            if seeded {
                let content = json!([call("z", "unknown_tool")]);
                replay(&world, vec![assistant(&content, "toolUse", None)]);
            }
            let started = tool_call("z", "unknown_tool", &json!({"z": 2}));
            dispatch(
                &world,
                AgentEvent::ToolExecutionStart {
                    tool_call: started.clone(),
                },
            );
            assert_eq!(pending(&world), ["z"]);
            dispatch(
                &world,
                AgentEvent::ToolExecutionUpdate {
                    tool_call: started.clone(),
                    partial_result: tool_result("PARTIAL_RESULT", Some(Value::Null)),
                },
            );
            let partial = world.mode.chat_container.render(100).join("\n");
            assert!(partial.contains("PARTIAL_RESULT"));
            dispatch(
                &world,
                AgentEvent::ToolExecutionEnd {
                    tool_call: started,
                    result: tool_result("FINAL_RESULT", Some(json!({"z": 1, "a": 2}))),
                    is_error,
                },
            );
            shown(&world, &expected);
        }
    });
}

#[test]
fn events_unknown_tools_do_not_add_or_render_rows() {
    let test = "events_unknown_tools_do_not_add_or_render_rows";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Unknown { phase }, Expected::Shown(expected)) = (input, expected) else {
                panic!("an unknown call");
            };
            let world = streamed(directory, false, Setup::default()).await;
            let missing = tool_call("missing", "unknown_tool", &json!({}));
            let event = match phase.as_str() {
                "update" => AgentEvent::ToolExecutionUpdate {
                    tool_call: missing,
                    partial_result: tool_result("", None),
                },
                _ => AgentEvent::ToolExecutionEnd {
                    tool_call: missing,
                    result: tool_result("", None),
                    is_error: false,
                },
            };
            dispatch(&world, event);
            assert_eq!(world.runtime.pending(), 0);
            shown(&world, &expected);
        }
    });
}

/// A registered tool `fixture` whose call row reads `CALL` and whose result renderer records
/// what it receives in `received`, as recorded: negative zero is written as the recording's
/// JSON text writes it, 0, once its sign is noted.
fn recording(received: Rc<RefCell<Vec<Value>>>) -> Definition {
    let mut fixture = tool("fixture");
    fixture.render_call = Some(Rc::new(|_, _, _| Ok(text("CALL"))));
    fixture.render_result = Some(ToolRenderResult::<Value, ()>::Raw(Rc::new(
        move |result, options, _, context| {
            let details = result.details.cloned();
            let signed_zero = details
                .as_ref()
                .and_then(Value::as_f64)
                .is_some_and(|value| value == 0.0 && value.is_sign_negative());
            let written = if signed_zero {
                json!(0)
            } else {
                details.clone().unwrap_or(Value::Null)
            };
            received
                .borrow_mut()
                .push(json!({"detailsMissing": details.is_none(),
                "details": written, "streaming": options.is_partial,
                "failed": context.is_error, "signedZero": signed_zero}));
            Ok(text("RESULT"))
        },
    )));
    fixture
}

#[test]
fn events_renderer_fields_survive_progress_and_completion() {
    let test = "events_renderer_fields_survive_progress_and_completion";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Details { kind, failed }, Expected::Shown(expected)) = (input, expected)
            else {
                panic!("a details case");
            };
            let received = Rc::new(RefCell::new(Vec::new()));
            let setup = Setup {
                extension: Some(registering(vec![recording(Rc::clone(&received))])),
                ..Setup::default()
            };
            let world = streamed(directory, false, setup).await;
            let started = tool_call("z", "fixture", &json!({"z": 1, "a": 2}));
            dispatch(
                &world,
                AgentEvent::ToolExecutionStart {
                    tool_call: started.clone(),
                },
            );
            let details = (kind != "missing").then(|| kind_value(&kind));
            dispatch(
                &world,
                AgentEvent::ToolExecutionUpdate {
                    tool_call: started.clone(),
                    partial_result: tool_result("VALUE", details.clone()),
                },
            );
            dispatch(
                &world,
                AgentEvent::ToolExecutionEnd {
                    tool_call: started,
                    result: tool_result("VALUE", details),
                    is_error: failed,
                },
            );
            let received = received.take();
            if kind == "object" {
                let keys: Vec<_> = received[1]["details"].as_object().unwrap().keys().collect();
                assert_eq!(keys, ["z", "a"]);
            }
            assert_eq!(Some(received), expected.received);
            shown(&world, &expected);
        }
    });
}

/// The assistant message of each message end `world`'s session delivers from now on.
fn ended_messages(world: &World) -> Rc<RefCell<Vec<SharedAssistantMessage>>> {
    let ended = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&ended);
    let removal = world
        .host
        .session()
        .subscribe(Rc::new(move |event: &AgentSessionEvent| {
            if let AgentSessionEvent::Agent(event) = event
                && let AgentEvent::MessageEnd {
                    message: AgentMessage::Assistant(message),
                } = &*event.read().unwrap()
            {
                seen.borrow_mut().push(Arc::clone(message));
            }
        }));
    // The subscription lasts as long as the session.
    drop(removal);
    ended
}

/// Prompts `run` through `world`'s real session, answering each provider request with
/// `respond`, and waits for the prompt and every retry to settle under a paused clock.
pub(super) async fn prompted(
    world: &World,
    respond: impl Fn(usize, &AssistantMessageEventStream) + 'static,
) {
    let session = world.host.session();
    let services = world.host.services();
    services.auth_storage.set_runtime_api_key("unknown", "key");
    let requests = Cell::new(0);
    *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, _, _, _| {
        let stream = AssistantMessageEventStream::new();
        respond(requests.get(), &stream);
        requests.set(requests.get() + 1);
        Box::pin(std::future::ready(Ok(stream)))
    }));
    // The prompt starts its work when called, so it is called inside the task set.
    let prompt = async { session.prompt("run", PromptOptions::default()).await };
    tokio::time::pause();
    prompt.await.unwrap();
    tokio::time::resume();
}

/// Prompts `world`'s session so that its first `retries` responses fail with a retryable
/// error and the next one streams one call `z` and ends aborted; returns that message as
/// its end delivered it.
async fn retried(world: &World, retries: u32) -> SharedAssistantMessage {
    let session = world.host.session();
    let settings = Rc::clone(session.settings_manager());
    settings
        .borrow_mut()
        .set_global_key("retry", "baseDelayMs", 1);
    let ended = ended_messages(world);
    let retries = usize::try_from(retries).unwrap();
    prompted(world, move |request, stream| {
        if request < retries {
            stream.end(Some(shared(&json!([]), "error", Some("overloaded"))));
            return;
        }
        let content = json!([call("z", "unknown_tool")]);
        stream.push(AssistantMessageEvent::Start {
            partial: shared(&json!([]), "stop", None),
        });
        stream.push(AssistantMessageEvent::ToolcallEnd {
            content_index: 0,
            tool_call: tool_call("z", "unknown_tool", &json!({"z": 1, "a": 2})),
            partial: shared(&content, "toolUse", None),
        });
        stream.end(Some(shared(&content, "aborted", None)));
    })
    .await;
    let ended = ended.borrow();
    assert_eq!(ended.len(), retries + 1);
    Arc::clone(ended.last().unwrap())
}

#[test]
fn events_agent_end_clears_transient_rows_before_shutdown() {
    let test = "events_agent_end_clears_transient_rows_before_shutdown";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (loading, streaming, shown_progress) = match input {
                StreamOp::AgentEnd {
                    loading,
                    streaming,
                    shutdown,
                } => {
                    assert!(!shutdown);
                    (loading, streaming, true)
                }
                StreamOp::AgentEndProgressDisabled { loading } => (loading, false, false),
                _ => panic!("an agent end"),
            };
            let Expected::Shown(expected) = expected else {
                panic!("a presentation");
            };
            let world = streamed(directory, shown_progress, Setup::default()).await;
            if loading {
                dispatch(&world, AgentEvent::AgentStart);
            }
            if streaming {
                let unfinished = json!([{"type": "text", "text": "UNFINISHED"}]);
                start(&world, &unfinished);
            }
            stale(&world, "stale");
            let effects = tap(&world);
            dispatch(
                &world,
                AgentEvent::AgentEnd {
                    messages: Vec::new(),
                },
            );
            let cleared = vec![PROGRESS_OFF.to_owned()];
            assert_eq!(
                progress(&effects),
                if shown_progress { cleared } else { Vec::new() }
            );
            assert!(world.mode.working.loading_animation.borrow().is_none());
            // The stopped row leaves no animation waiting: a second later nothing is pending.
            let later = Duration::from_millis(world.runtime.millis() + 1000);
            for result in world.runtime.advance_to(later) {
                result.unwrap();
            }
            assert_eq!(world.runtime.pending(), 0);
            shown(&world, &expected);
        }
        completed_rows_stay(directory).await;
    });
}

/// Ends a run over replayed rows and an unfinished assistant row: only the unfinished row
/// goes, and the replayed call's registration is forgotten.
async fn completed_rows_stay(directory: &Path) {
    let world = streamed(directory, true, Setup::default()).await;
    let content = json!([call("z", "unknown_tool")]);
    replay(&world, vec![assistant(&content, "toolUse", None)]);
    let completed = world.mode.chat_container.render(100);
    let unfinished = json!([{"type": "text", "text": "UNFINISHED"}]);
    start(&world, &unfinished);
    dispatch(
        &world,
        AgentEvent::AgentEnd {
            messages: Vec::new(),
        },
    );
    assert_eq!(world.mode.chat_container.render(100), completed);
    assert!(pending(&world).is_empty());
}

#[test]
fn events_queue_update_reads_current_session_display() {
    let test = "events_queue_update_reads_current_session_display";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Queue { q }, Expected::Shown(expected)) = (input, expected) else {
                panic!("a queue update");
            };
            let world = streamed(directory, false, Setup::default()).await;
            // The recording had no binding for the dequeue action.
            let unbound = Some(KeybindingKeys::Multiple(Vec::new()));
            let bindings = [("app.message.dequeue".to_owned(), unbound)];
            maestro_tui::get_keybindings().set_user_bindings(bindings.into_iter().collect());
            let session = world.host.session();
            for text in &q.steering {
                session.steer(text, &[]).unwrap();
            }
            for text in &q.follow_up {
                session.follow_up(text, &[]).unwrap();
            }
            world.mode.pending_messages_container.clear();
            let decoy = AgentSessionEvent::QueueUpdate {
                steering: vec!["DECOY".into()],
                follow_up: vec!["DECOY".into()],
            };
            world.mode.handle_event(&decoy).unwrap();
            assert_eq!(world.runtime.pending(), 1);
            world.runtime.settle().unwrap();
            shown(&world, &expected);
        }
    });
}

#[test]
fn events_thinking_toggle_keeps_the_default_streaming_tail() {
    let test = "events_thinking_toggle_keeps_the_default_streaming_tail";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::ThinkingDefault { hide }, Expected::Shown(expected)) = (input, expected)
            else {
                panic!("a thinking toggle");
            };
            let setup = Setup {
                configure: Box::new(move |settings| settings.set_hide_thinking_block(hide)),
                ..Setup::default()
            };
            let world = streamed(directory, false, setup).await;
            let content = json!([{"type": "thinking", "thinking": "REASON"},
                {"type": "text", "text": "ANSWER"}]);
            let started = shared(&content, "stop", None);
            dispatch(
                &world,
                AgentEvent::MessageStart {
                    message: AgentMessage::Assistant(Arc::clone(&started)),
                },
            );
            world.terminal.send_input("\x14");
            shown(&world, &expected);
            let streaming = world.mode.streaming.borrow().clone().unwrap();
            assert!(Arc::ptr_eq(&streaming.1, &started));
            // The re-added row keeps streaming: a later update and the end reach it.
            let answered = json!([{"type": "thinking", "thinking": "REASON"},
                {"type": "text", "text": "LATER"}]);
            dispatch(&world, update(&shared(&answered, "stop", None)));
            let rows = world.mode.chat_container.render(100).join("\n");
            assert!(rows.contains("LATER") && !rows.contains("ANSWER"));
            dispatch(
                &world,
                AgentEvent::MessageEnd {
                    message: AgentMessage::Assistant(shared(&answered, "stop", None)),
                },
            );
            assert!(world.mode.streaming.borrow().is_none());
            assert!(history(&world).is_empty());
        }
    });
}

/// A registered tool `name` whose call renderer records each row phase's argument
/// completion in `phases`.
fn phased(name: &str, phases: Rc<RefCell<Vec<bool>>>) -> Definition {
    let mut definition = tool(name);
    definition.render_call = Some(Rc::new(move |_, _, context| {
        phases.borrow_mut().push(context.phase.args_complete);
        Ok(text("CALL"))
    }));
    definition
}

#[test]
fn events_message_end_completes_arguments_only_without_failure() {
    isolated(
        "events_message_end_completes_arguments_only_without_failure",
        async |directory| {
            for (stop, completed) in [("stop", true), ("toolUse", true), ("error", false)] {
                let phases = Rc::new(RefCell::new(Vec::new()));
                let setup = Setup {
                    extension: Some(registering(vec![phased("phased", Rc::clone(&phases))])),
                    ..Setup::default()
                };
                let world = streamed(directory, false, setup).await;
                start(&world, &json!([]));
                let content = json!([call("z", "phased")]);
                dispatch(&world, update(&shared(&content, "toolUse", None)));
                assert_eq!(phases.borrow().last(), Some(&false));
                dispatch(
                    &world,
                    AgentEvent::MessageEnd {
                        message: AgentMessage::Assistant(shared(&content, stop, None)),
                    },
                );
                assert_eq!(phases.borrow().last(), Some(&completed));
                assert_eq!(pending(&world).is_empty(), !completed);
            }
        },
    );
}

/// A registered tool `progress` that reports `HALFWAY` and finishes with `FINISHED`.
fn progressing() -> Definition {
    let mut progress = tool("progress");
    progress.execute = Rc::new(|_, _, _, on_update, _| {
        if let Some(update) = on_update {
            update(tool_result("HALFWAY", None));
        }
        Box::pin(std::future::ready(Ok(tool_result("FINISHED", None))))
    });
    progress
}

/// What the mode showed when one session event had been delivered.
struct Seen {
    /// The event's record type.
    kind: String,
    /// Whether the working row was shown.
    loading: bool,
    /// Whether an assistant row was streaming.
    streaming: bool,
    /// The unresolved calls.
    pending: Vec<String>,
    /// The transcript rows, joined.
    chat: String,
}

/// What the mode of `world` shows as each later session event is delivered: this listener
/// is registered after the mode's own, so each record follows the mode's handling.
fn seen(world: &World) -> Rc<RefCell<Vec<Seen>>> {
    let records = Rc::new(RefCell::new(Vec::new()));
    let (sink, mode) = (Rc::clone(&records), Rc::downgrade(&world.mode));
    let removal = world
        .host
        .session()
        .subscribe(Rc::new(move |event: &AgentSessionEvent| {
            let mode = mode.upgrade().unwrap();
            let kind = serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned();
            sink.borrow_mut().push(Seen {
                kind,
                loading: mode.working.loading_animation.borrow().is_some(),
                streaming: mode.streaming.borrow().is_some(),
                pending: mode.pending_tools.borrow().keys().cloned().collect(),
                chat: mode.chat_container.render(100).join("\n"),
            });
        }));
    // The subscription lasts as long as the session.
    drop(removal);
    records
}

/// The position of the first record of `kind` in `seen`.
fn first(seen: &[Seen], kind: &str) -> usize {
    seen.iter().position(|seen| seen.kind == kind).unwrap()
}

/// The controlled provider's answers: first text and a call of `progress`, then `DONE`.
fn answer_with_progress(request: usize, stream: &AssistantMessageEventStream) {
    if request > 0 {
        let done = json!([{"type": "text", "text": "DONE"}]);
        stream.end(Some(shared(&done, "stop", None)));
        return;
    }
    let answer = json!([{"type": "text", "text": "ANSWER"}]);
    let content = json!([{"type": "text", "text": "ANSWER"}, call("call", "progress")]);
    stream.push(AssistantMessageEvent::Start {
        partial: shared(&json!([]), "stop", None),
    });
    stream.push(AssistantMessageEvent::TextDelta {
        content_index: 0,
        delta: "ANSWER".into(),
        partial: shared(&answer, "stop", None),
    });
    stream.push(AssistantMessageEvent::ToolcallEnd {
        content_index: 1,
        tool_call: tool_call("call", "progress", &json!({"z": 1, "a": 2})),
        partial: shared(&content, "toolUse", None),
    });
    stream.end(Some(shared(&content, "toolUse", None)));
}

#[test]
fn events_registered_session_orders_agent_message_and_tool_events() {
    isolated(
        "events_registered_session_orders_agent_message_and_tool_events",
        async |directory| {
            let setup = Setup {
                extension: Some(registering(vec![progressing()])),
                ..Setup::default()
            };
            let world = streamed(directory, false, setup).await;
            let records = seen(&world);
            prompted(&world, answer_with_progress).await;
            let records = records.take();
            let at = |kind| &records[first(&records, kind)];
            assert!(at("agent_start").loading);
            let streaming = at("message_update");
            assert!(streaming.streaming && streaming.chat.contains("ANSWER"));
            assert_eq!(at("tool_execution_start").pending, ["call"]);
            assert!(at("tool_execution_update").chat.contains("HALFWAY"));
            let ended = at("tool_execution_end");
            assert!(ended.pending.is_empty() && ended.chat.contains("FINISHED"));
            let finished = at("agent_end");
            assert!(!finished.loading && !finished.streaming);
            let order = [
                "agent_start",
                "message_update",
                "tool_execution_start",
                "tool_execution_update",
                "tool_execution_end",
                "agent_end",
            ];
            let positions: Vec<_> = order.iter().map(|kind| first(&records, kind)).collect();
            assert!(positions.is_sorted());
            let chat = world.mode.chat_container.render(100).join("\n");
            let shown_in_order =
                ["run", "ANSWER", "FINISHED", "DONE"].map(|text| chat.find(text).unwrap());
            assert!(shown_in_order.is_sorted());
            assert!(!chat.contains("HALFWAY"));
            assert!(world.mode.status_container.render(100).is_empty());
            failed_response_completes_pending_rows(directory).await;
        },
    );
}

/// Prompts through a real session whose provider fails the response: the failure is an
/// assistant error event, not a failed prompt, and its end completes the pending row with the
/// provider's error.
async fn failed_response_completes_pending_rows(directory: &Path) {
    let world = streamed(directory, false, Setup::default()).await;
    prompted(&world, |_, stream| {
        let content = json!([call("failed", "unknown_tool")]);
        stream.push(AssistantMessageEvent::Start {
            partial: shared(&json!([]), "stop", None),
        });
        stream.push(AssistantMessageEvent::ToolcallEnd {
            content_index: 0,
            tool_call: tool_call("failed", "unknown_tool", &json!({})),
            partial: shared(&content, "toolUse", None),
        });
        stream.end(Some(shared(&content, "error", Some("PROVIDER_FAILED"))));
    })
    .await;
    let chat = world.mode.chat_container.render(100).join("\n");
    assert!(chat.contains("PROVIDER_FAILED"));
    assert!(pending(&world).is_empty());
}

#[test]
fn events_incremental_frames_keep_one_writer() {
    isolated(
        "events_incremental_frames_keep_one_writer",
        async |directory| {
            let world = streamed(directory, false, Setup::default()).await;
            let redraws = world.mode.ui.full_redraws();
            let text = |text: &str| json!([{"type": "text", "text": text}]);
            start(&world, &text("one"));
            world.runtime.settle().unwrap();
            world.terminal.clear_writes();
            dispatch(&world, update(&shared(&text("one\n\ntwo"), "stop", None)));
            world.runtime.settle().unwrap();
            let grown = world.terminal.writes().concat();
            world.terminal.clear_writes();
            dispatch(&world, update(&shared(&text("one\n\nthree"), "stop", None)));
            world.runtime.settle().unwrap();
            let changed = world.terminal.writes().concat();
            // Each update rewrites from its first changed row in the same writer, never the
            // whole screen: growing moves the message's end marker off the first row, and a
            // later change keeps the unchanged first row.
            assert_eq!(world.mode.ui.full_redraws(), redraws);
            assert!(grown.contains("two"));
            assert!(changed.contains("three") && !changed.contains("one"));
        },
    );
}

#[test]
fn events_stream_work_is_linear() {
    isolated("events_stream_work_is_linear", async |directory| {
        let mut work = Vec::new();
        for calls in [16, 32, 64] {
            let rendered = Rc::new(RefCell::new(Vec::new()));
            let setup = Setup {
                extension: Some(registering(vec![phased("counted", Rc::clone(&rendered))])),
                ..Setup::default()
            };
            let world = streamed(directory, false, setup).await;
            start(&world, &json!([]));
            let ids: Vec<String> = (0..calls).map(|id: usize| id.to_string()).collect();
            let content: Vec<Value> = ids.iter().map(|id| call(id, "counted")).collect();
            let message = shared(&Value::Array(content), "toolUse", None);
            dispatch(&world, update(&message));
            dispatch(&world, update(&message));
            assert_eq!(pending(&world), ids);
            let renders = rendered.borrow().len();
            assert!(renders <= 4 * calls);
            work.push(renders);
            complete_without_shifting(&world, &ids);
            retain_one_across_turns(&world, calls);
        }
        // Doubling the calls doubles the renderer work: each update visits each call once.
        assert_eq!(work[1], 2 * work[0]);
        assert_eq!(work[2], 2 * work[1]);
    });
}

/// Completes calls in encounter order, observing movement of retained entries rather than
/// elapsed time, and checks that their final output remains in the transcript.
fn complete_without_shifting(world: &World, ids: &[String]) {
    let mut moved = 0;
    for id in ids {
        let before: Vec<_> = world
            .mode
            .pending_tools
            .borrow()
            .values()
            .map(std::ptr::from_ref)
            .collect();
        dispatch(
            world,
            AgentEvent::ToolExecutionEnd {
                tool_call: tool_call(id, "counted", &json!({})),
                result: tool_result("COMPLETED", None),
                is_error: false,
            },
        );
        let pending = world.mode.pending_tools.borrow();
        moved += before
            .iter()
            .skip(1)
            .zip(pending.values())
            .filter(|(previous, row)| **previous != std::ptr::from_ref(*row))
            .count();
    }
    assert!(pending(world).is_empty());
    assert!(
        world
            .mode
            .chat_container
            .render(100)
            .join("\n")
            .contains("COMPLETED")
    );
    assert!(
        moved <= ids.len(),
        "completion relocated {moved} retained rows for {} calls",
        ids.len()
    );
}

/// Keeps one unresolved call across assistant ends, bounding the backing-slot work of the
/// actual pending-row traversal even after many other calls have completed.
fn retain_one_across_turns(world: &World, turns: usize) {
    stale(world, "retained");
    let mut scanned = 0;
    for _ in 0..turns {
        end_streamed(world, "toolUse", None);
        let bound = world
            .mode
            .pending_tools
            .borrow()
            .values()
            .size_hint()
            .1
            .unwrap();
        scanned += bound;
        dispatch(
            world,
            AgentEvent::ToolExecutionEnd {
                tool_call: tool_call("z", "unknown_tool", &json!({})),
                result: tool_result("COMPLETED", None),
                is_error: false,
            },
        );
        assert_eq!(pending(world), ["retained"]);
        assert!(
            bound <= 4,
            "assistant end scans up to {bound} slots for two live calls"
        );
    }
    assert!(scanned <= 4 * turns);
}

/// Sets the working row's visibility while a response streams through `world`'s real
/// session, after starting it hidden when it is to be shown; returns the status rows and
/// whether a working row ran right after the change.
async fn visible_while_streaming(world: &World, visible: bool) -> (Vec<String>, bool) {
    let ui = port(&world.mode);
    if visible {
        ui.set_working_visible(false).unwrap();
    }
    let seen = Rc::new(RefCell::new(None));
    let (sink, mode) = (Rc::clone(&seen), Rc::downgrade(&world.mode));
    prompted(world, move |_, stream| {
        let mode = mode.upgrade().unwrap();
        port(&mode).set_working_visible(visible).unwrap();
        let running = mode.working.loading_animation.borrow().is_some();
        sink.replace(Some((mode.status_container.render(100), running)));
        stream.end(Some(shared(&json!([]), "stop", None)));
    })
    .await;
    seen.take().unwrap()
}

#[test]
fn events_working_controls_update_retained_loader() {
    let test = "events_working_controls_update_retained_loader";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let Expected::Shown(expected) = expected else {
                panic!("a presentation");
            };
            let world = streamed(directory, false, Setup::default()).await;
            let ui = port(&world.mode);
            match input {
                StreamOp::Visible {
                    streaming: true,
                    visible,
                } => {
                    // The recording hides the indicator, so no frame advances.
                    ui.set_working_indicator(Some(frames(&[]))).unwrap();
                    let (status, running) = visible_while_streaming(&world, visible).await;
                    assert_eq!(status, expected.status);
                    assert_eq!(running, visible);
                    continue;
                }
                StreamOp::Visible {
                    streaming: false,
                    visible,
                } => {
                    ui.set_working_indicator(Some(frames(&[]))).unwrap();
                    ui.set_working_visible(visible).unwrap();
                    assert!(world.mode.working.loading_animation.borrow().is_none());
                }
                StreamOp::Indicator { indicator } => {
                    ui.set_working_indicator(Some(frames(&["OLD"]))).unwrap();
                    dispatch(&world, AgentEvent::AgentStart);
                    let options = WorkingIndicatorOptions {
                        frames: Some(indicator.frames),
                        interval_ms: indicator.interval,
                    };
                    ui.set_working_indicator(Some(options)).unwrap();
                }
                _ => panic!("a working control"),
            }
            drawn(&world);
            shown(&world, &expected);
        }
        hidden_row_releases_its_timer(directory).await;
    });
}

/// Hides a running working row with the toolkit's animated indicator: the row goes and its
/// animation leaves nothing pending.
async fn hidden_row_releases_its_timer(directory: &Path) {
    let world = streamed(directory, false, Setup::default()).await;
    let ui = port(&world.mode);
    ui.set_working_indicator(None).unwrap();
    dispatch(&world, AgentEvent::AgentStart);
    drawn(&world);
    assert!(world.runtime.pending() > 0);
    ui.set_working_visible(false).unwrap();
    assert!(world.mode.working.loading_animation.borrow().is_none());
    assert!(world.mode.status_container.render(100).is_empty());
    let later = Duration::from_millis(world.runtime.millis() + 1000);
    for result in world.runtime.advance_to(later) {
        result.unwrap();
    }
    assert_eq!(world.runtime.pending(), 0);
}

#[test]
fn events_thinking_toggle_and_label_keep_streaming_tail() {
    let test = "events_thinking_toggle_and_label_keep_streaming_tail";
    isolated(test, async |directory| {
        for (input, expected) in cases(FIXTURES, test) {
            let (StreamOp::Thinking { hide, label }, Expected::Shown(expected)) = (input, expected)
            else {
                panic!("a thinking label");
            };
            let setup = Setup {
                configure: Box::new(move |settings| settings.set_hide_thinking_block(hide)),
                ..Setup::default()
            };
            let world = streamed(directory, false, setup).await;
            let content = json!([{"type": "thinking", "thinking": "REASON"},
                {"type": "text", "text": "ANSWER"}]);
            start(&world, &content);
            port(&world.mode).set_hidden_thinking_label(label).unwrap();
            world.terminal.send_input("\x14");
            shown(&world, &expected);
        }
        label_reaches_replayed_and_later_rows(directory).await;
    });
}

/// Sets a label over a replayed assistant row: that row shows it, and so does a later
/// streaming row.
async fn label_reaches_replayed_and_later_rows(directory: &Path) {
    let setup = Setup {
        configure: Box::new(|settings| settings.set_hide_thinking_block(true)),
        ..Setup::default()
    };
    let world = streamed(directory, false, setup).await;
    let content = json!([{"type": "thinking", "thinking": "REASON"},
        {"type": "text", "text": "ANSWER"}]);
    replay(&world, vec![assistant(&content, "stop", None)]);
    port(&world.mode)
        .set_hidden_thinking_label(Some("LABEL".into()))
        .unwrap();
    let labels = |world: &World| {
        let chat = world.mode.chat_container.render(100).join("\n");
        chat.matches("LABEL").count()
    };
    assert_eq!(labels(&world), 1);
    start(&world, &content);
    assert_eq!(labels(&world), 2);
}

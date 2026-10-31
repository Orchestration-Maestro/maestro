//! Session commands typed in the editor or bound to its keys: a new session and a manual
//! compaction, through the real runtime, session and progress subscriber.

use super::super::bash::component_address;
use super::typed::{poll, recalled, waiting};
use super::{
    ManualRuntime, Setup, World, admitted, cases, isolated, isolated_case, isolated_output,
    painted, scripted, submit, with_held_response,
};
use crate::InteractiveMode;
use maestro_app::{
    AgentSession, AgentSessionEvent, CompactionResult, CreateAgentSessionRuntimeFactory,
    DiagnosticErrorInfo, ExtensionFactory, Local, Presence, ReadonlySessionManager,
    SessionBeforeCompactEvent, SessionBeforeCompactResult, SessionBeforeSwitchEvent,
    SessionBeforeSwitchResult, SessionManager, SettingsManager, strip_ansi,
};
use maestro_tui::{Component, KeybindingKeys};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    path::Path,
    rc::{Rc, Weak},
};

/// Every recorded command case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_commands.json");

/// The key binding file that binds the new-session action at construction.
const BOUND: &str = r#"{"app.session.new": "alt+n"}"#;

/// What the mode looked like at each new-session request: whether its working row was
/// released and its status rows were empty.
type Requests = Rc<RefCell<Vec<(bool, bool)>>>;

/// An extension deciding new-session requests and observing the mode at each one.
#[derive(Clone, Default)]
struct Guard {
    /// Whether the next request is vetoed.
    veto: Rc<Cell<bool>>,
    /// The mode observed at each request, set once it exists.
    mode: Rc<RefCell<Weak<InteractiveMode>>>,
    /// What each request observed.
    requests: Requests,
    /// How the next compaction is answered.
    compaction: Rc<Cell<Compaction>>,
    /// The instructions each compaction received.
    instructions: Rc<RefCell<Vec<Option<String>>>>,
}

/// How the extension answers a compaction.
#[derive(Clone, Copy, Default)]
enum Compaction {
    /// It cancels the compaction.
    #[default]
    Cancel,
    /// It supplies the summary `summary`.
    Supply,
    /// It lets the session summarize, which fails without a registered provider.
    Continue,
}

impl Guard {
    /// The extension answering every new-session request through [`Guard::decide`].
    fn extension(&self) -> ExtensionFactory<Local> {
        let guard = self.clone();
        Rc::new(move |api| {
            let (switching, compacting) = (guard.clone(), guard.clone());
            let registered = api.on::<SessionBeforeSwitchEvent, SessionBeforeSwitchResult>(
                "session_before_switch".into(),
                Rc::new(move |_, _| Box::pin(std::future::ready(Ok(Some(switching.decide()))))),
            );
            let compacted = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
                "session_before_compact".into(),
                Rc::new(move |event, _| {
                    Box::pin(std::future::ready(Ok(compacting.compact(event))))
                }),
            );
            Box::pin(std::future::ready(registered.and(compacted)))
        })
    }

    /// Records and prints the mode's working row and status rows, so a child that ends the
    /// process reports them too, then vetoes while `veto` holds.
    fn decide(&self) -> SessionBeforeSwitchResult {
        if let Some(mode) = self.mode.borrow().upgrade() {
            let released = mode.working.loading_animation.borrow().is_none();
            let empty = mode.status_container.children().borrow().is_empty();
            println!("\nPROBE released={released} empty={empty}");
            self.requests.borrow_mut().push((released, empty));
        }
        let cancel = if self.veto.get() {
            Presence::Present(true)
        } else {
            Presence::Missing
        };
        SessionBeforeSwitchResult { cancel }
    }

    /// Records the compaction's instructions and answers it as `compaction` selects.
    fn compact(&self, event: &SessionBeforeCompactEvent) -> Option<SessionBeforeCompactResult> {
        let instructions = match &event.custom_instructions {
            Presence::Present(text) => Some(text.clone()),
            Presence::Missing | Presence::Null => None,
        };
        self.instructions.borrow_mut().push(instructions);
        let (cancel, compaction) = match self.compaction.get() {
            Compaction::Continue => return None,
            Compaction::Cancel => (Presence::Present(true), Presence::Missing),
            Compaction::Supply => (
                Presence::Missing,
                Presence::Present(CompactionResult {
                    summary: "controlled summary".to_owned(),
                    first_kept_entry_id: event.preparation.first_kept_entry_id.clone(),
                    tokens_before: 123.0,
                    details: Presence::Missing,
                    usage: Presence::Missing,
                }),
            ),
        };
        Some(SessionBeforeCompactResult { cancel, compaction })
    }
}

/// A started mode over replaceable sessions whose shell runs `shell`, decided by `guard`,
/// replacing sessions through `create` when supplied.
async fn commanding(
    directory: &Path,
    shell: Option<String>,
    guard: &Guard,
    create: Option<CreateAgentSessionRuntimeFactory>,
) -> World {
    let world = Setup {
        configure: Box::new(move |settings: &mut SettingsManager| {
            settings.set_shell_path(shell);
        }),
        extension: Some(guard.extension()),
        replacing: true,
        create,
        ..Setup::default()
    }
    .build(directory);
    *guard.mode.borrow_mut() = Rc::downgrade(&world.mode);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world
}

/// Awaits the work the mode handed its terminal runtime and the work it handed over in
/// turn, returning the failures that work reported.
async fn settled(world: &World) -> Vec<Box<dyn Error>> {
    let mut errors = Vec::new();
    while world.runtime.futures() > 0 {
        errors.extend(world.runtime.drain_futures());
        tokio::task::yield_now().await;
    }
    errors
}

/// Whether any of `rows` contains `text` once styling is removed.
fn shown(rows: &[String], text: &str) -> bool {
    rows.iter()
        .any(|row| strip_ansi(row).unwrap().contains(text))
}

/// The addresses of the retained pending shell rows, in order.
fn pending_shells(world: &World) -> Vec<usize> {
    let pending = world.mode.pending_shells.borrow();
    pending.iter().map(component_address).collect()
}

/// Gives the transcript an old row and the compaction queue an old message.
fn seed_old_state(world: &World) {
    world
        .mode
        .chat_container
        .add_child(Rc::new(maestro_tui::Text::new(
            "old transcript".to_owned(),
            0,
            0,
            None,
        )));
    world.mode.compaction_queued_messages.borrow_mut().push(
        super::super::queues::CompactionQueuedMessage {
            text: "old queue".to_owned(),
            mode: maestro_app::messages::UserMessageDelivery::FollowUp,
        },
    );
}

/// One recorded submission.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Submission {
    /// The submitted text.
    text: String,
}

/// Where a recorded submission went.
#[derive(Deserialize)]
#[serde(tag = "route", rename_all = "camelCase", deny_unknown_fields)]
enum Route {
    /// The ordinary input received `delivered` and the history recorded it.
    Input {
        /// The text the waiting input received.
        delivered: String,
    },
    /// The new-session command ran.
    New,
    /// The compaction command ran with `instructions`.
    Compact {
        /// The instructions the compaction received, absent when none were given.
        #[serde(default)]
        instructions: Option<String>,
    },
}

/// Gives the current session two messages and a key for the controlled provider, so a
/// compaction reaches the extension.
fn compactable(world: &World) {
    let session = world.host.session();
    let mut manager = session.session_manager().borrow_mut();
    manager
        .append_message(super::transcript::user_record("question"))
        .unwrap();
    manager
        .append_message(super::transcript::assistant(&json!([]), "stop", None))
        .unwrap();
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("unknown", "key");
}

#[test]
fn command_submission_selects_new_and_compact_with_exact_whitespace() {
    const TEST: &str = "command_submission_selects_new_and_compact_with_exact_whitespace";
    isolated(TEST, async |directory| {
        let guard = Guard::default();
        guard.veto.set(true);
        let world = commanding(directory, None, &guard, None).await;
        compactable(&world);
        for (Submission { text }, route) in cases::<Submission, Route>(FIXTURES, TEST) {
            let requests = guard.requests.borrow().len();
            let compactions = guard.instructions.borrow().len();
            world.mode.editor.set_text("draft");
            let mut waiter = waiting(&world.mode);
            submit(&world.mode, &text);
            let received = poll(&mut waiter);
            let errors = settled(&world).await;
            assert!(errors.is_empty(), "{text:?}: {errors:?}");
            let history = recalled(&world.mode.editor);
            match route {
                Route::Input { delivered } => {
                    assert_eq!(received.as_ref(), Some(&delivered), "{text:?}");
                    assert_eq!(history.first(), Some(&delivered), "{text:?}");
                    assert_eq!(guard.requests.borrow().len(), requests, "{text:?}");
                    assert_eq!(guard.instructions.borrow().len(), compactions, "{text:?}");
                }
                Route::Compact { instructions } => {
                    assert_eq!(received, None, "{text:?}");
                    assert_ne!(history.first().map(String::as_str), Some(text.trim()));
                    let seen = guard.instructions.borrow();
                    assert_eq!(seen[compactions..], [instructions], "{text:?}");
                    assert_eq!(world.mode.editor.get_text(), "", "{text:?}");
                }
                Route::New => {
                    assert_eq!(received, None, "{text:?}");
                    assert_ne!(history.first().map(String::as_str), Some(text.trim()));
                    assert_eq!(guard.requests.borrow().len(), requests + 1, "{text:?}");
                    assert_eq!(world.mode.editor.get_text(), "", "{text:?}");
                }
            }
        }
    });
}

/// How a recorded new-session request ends.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    /// The replacement starts.
    Success,
    /// An extension vetoes it.
    Veto,
    /// The replacement fails.
    Reject,
    /// The replacement starts but its replay fails.
    ReplayFailure,
}

/// One recorded new-session request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewInput {
    /// How it ends.
    outcome: Outcome,
    /// Whether working progress is shown when it starts.
    loader: bool,
}

/// What a recorded new-session request leaves.
#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
enum NewExpected {
    /// The transcript holds exactly these rows.
    Notice {
        /// The transcript rows, role markers in place of styling.
        chat: Vec<String>,
    },
    /// The session, transcript and queued input stay as they were.
    Kept,
    /// The process ends with failure after showing `line`.
    Fatal {
        /// The final error line.
        line: String,
    },
}

/// A replacement factory whose creation fails.
fn rejecting() -> CreateAgentSessionRuntimeFactory {
    Rc::new(|_| {
        Box::pin(async { Err(DiagnosticErrorInfo::error("controlled replacement failure")) })
    })
}

/// A replacement factory whose fresh session stores a tool call its replay cannot read.
fn unreplayable() -> CreateAgentSessionRuntimeFactory {
    Rc::new(|options| {
        let replacement = super::session::session(&options.cwd, None, |inner| inner);
        std::mem::swap(
            &mut *replacement.session.session_manager().borrow_mut(),
            &mut *options.session_manager.borrow_mut(),
        );
        replacement
            .session
            .session_manager()
            .borrow_mut()
            .append_message(super::transcript::assistant(
                &serde_json::json!([{"type": "toolCall", "name": "read", "arguments": {}}]),
                "toolUse",
                None,
            ))
            .unwrap();
        Box::pin(async { Ok(replacement) })
    })
}

/// Runs one in-process new-session request and checks what it left.
async fn new_session_case(directory: &Path, input: &NewInput, expected: &NewExpected) {
    let guard = Guard::default();
    guard.veto.set(matches!(input.outcome, Outcome::Veto));
    let world = commanding(directory, None, &guard, None).await;
    seed_old_state(&world);
    if input.loader {
        world.mode.show_working_loader();
    }
    let old = world.host.session();
    let before = world.mode.chat_container.render(80);
    submit(&world.mode, "/new");
    assert!(settled(&world).await.is_empty());
    assert_eq!(*guard.requests.borrow(), [(true, true)]);
    assert!(world.mode.working.loading_animation.borrow().is_none());
    assert!(world.mode.status_container.children().borrow().is_empty());
    assert_left(&world, &old, &before, expected);
}

/// Checks the transcript, session and queued input a settled request left behind.
fn assert_left(
    world: &World,
    old: &Rc<maestro_app::AgentSession>,
    before: &[String],
    expected: &NewExpected,
) {
    let current = world.mode.runtime_host.session();
    match expected {
        NewExpected::Notice { chat } => {
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(world.mode.chat_container.render(80), painted(&theme, chat));
            assert!(!Rc::ptr_eq(old, &current));
            assert!(Rc::ptr_eq(&current, &world.host.session()));
            assert!(world.mode.compaction_queued_messages.borrow().is_empty());
        }
        NewExpected::Kept => {
            assert_eq!(world.mode.chat_container.render(80), before);
            assert!(Rc::ptr_eq(old, &current));
            assert_eq!(world.mode.compaction_queued_messages.borrow().len(), 1);
        }
        NewExpected::Fatal { .. } => unreachable!("fatal cases run in their own child"),
    }
}

/// Runs one fatal new-session request in this child, printing each terminal write.
async fn fatal_case(directory: &Path, input: &NewInput) {
    let guard = Guard::default();
    let create = match input.outcome {
        Outcome::Reject => rejecting(),
        _ => unreplayable(),
    };
    let world = commanding(directory, None, &guard, Some(create)).await;
    if input.loader {
        world.mode.show_working_loader();
    }
    world
        .terminal
        .set_tap(|write| println!("\nWRITE {:?}", strip_ansi(write).unwrap()));
    submit(&world.mode, "/new");
    drop(settled(&world).await);
    println!("\nPROBE survived");
}

#[test]
fn new_session_stops_progress_honors_veto_and_uses_runtime_failure_route() {
    const TEST: &str = "new_session_stops_progress_honors_veto_and_uses_runtime_failure_route";
    let recorded = cases::<NewInput, NewExpected>(FIXTURES, TEST);
    isolated(TEST, async |directory| {
        for (input, expected) in &recorded {
            if !matches!(expected, NewExpected::Fatal { .. }) {
                new_session_case(directory, input, expected).await;
            }
        }
    });
    for (index, (input, expected)) in recorded.iter().enumerate() {
        let NewExpected::Fatal { line } = expected else {
            continue;
        };
        let case = format!("fatal{index}");
        let body = async |directory: &Path| fatal_case(directory, input).await;
        let Some(output) = isolated_output(TEST, &case, &[], "trap '' XFSZ", body) else {
            continue;
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(output.status.code(), Some(1), "{case}: {stdout}");
        assert!(
            stdout.contains("PROBE released=true empty=true"),
            "{stdout}"
        );
        assert!(!stdout.contains("PROBE survived"), "{stdout}");
        let last = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("WRITE "))
            .rfind(|write| write.contains("Error: "));
        assert!(
            last.is_some_and(|write| write.contains(line.as_str())),
            "{stdout}"
        );
    }
}

#[test]
fn new_session_action_uses_live_bindings() {
    const TEST: &str = "new_session_action_uses_live_bindings";
    isolated(TEST, async |directory| {
        std::fs::write(directory.join("keybindings.json"), BOUND).unwrap();
        let guard = Guard::default();
        let world = commanding(directory, None, &guard, None).await;
        let requested = |key: &str| {
            let before = guard.requests.borrow().len();
            world.mode.editor.set_text("draft");
            world.terminal.send_input(key);
            guard.requests.borrow().len() > before || world.runtime.futures() > 0
        };
        guard.veto.set(true);
        assert!(
            requested("\x1bn"),
            "the constructor binding runs the action"
        );
        assert!(settled(&world).await.is_empty());
        assert_eq!(world.mode.editor.get_text(), "draft");
        assert_eq!(guard.requests.borrow().len(), 1);
        assert!(!requested("\x0e"), "a wrong modifier does nothing");
        maestro_tui::get_keybindings().set_user_bindings(vec![(
            "app.session.new".to_owned(),
            Some(KeybindingKeys::Single("alt+m".to_owned())),
        )]);
        assert!(!requested("\x1bn"), "the replaced key does nothing");
        let old = world.host.session();
        guard.veto.set(false);
        assert!(requested("\x1bm"), "the live binding runs the action");
        assert!(settled(&world).await.is_empty());
        assert_eq!(world.mode.editor.get_text(), "draft");
        assert!(!Rc::ptr_eq(&old, &world.host.session()));
        assert!(shown(
            &world.mode.chat_container.render(80),
            "✓ New session started"
        ));
    });
}

#[test]
fn new_session_shows_later_responses_and_compaction() {
    const TEST: &str = "new_session_shows_later_responses_and_compaction";
    isolated(TEST, async |directory| {
        let guard = Guard::default();
        let world = commanding(directory, None, &guard, None).await;
        submit(&world.mode, "/new");
        assert!(settled(&world).await.is_empty());
        let session = world.host.session();
        session
            .settings_manager()
            .borrow_mut()
            .set_compaction_enabled(false);
        let reply = json!([{"type": "text", "text": "fresh answer"}]);
        super::events::prompted(&world, move |_, stream| {
            stream.end(Some(super::events::shared(&reply, "stop", None)));
        })
        .await;
        let rows = world.mode.chat_container.render(80);
        assert!(shown(&rows, "fresh answer"), "the response shows: {rows:?}");
        submit(&world.mode, "/compact");
        assert!(settled(&world).await.is_empty());
        let rows = world.mode.chat_container.render(80);
        let failed = "Error: Compaction failed: No API provider registered for api: unknown";
        assert!(
            shown(&rows, failed),
            "the compaction ends in view: {rows:?}"
        );
    });
}

/// Shows `rows` as pending shell rows while a response streams, then resets through a new
/// session that `veto` decides, and checks that only a kept session keeps them.
async fn pending_reset_case(directory: &Path, rows: &[&str], veto: bool) {
    let guard = Guard::default();
    let script = scripted(directory, "echoing", "printf 'printed %s\\n' \"$2\"");
    let world = commanding(directory, Some(script), &guard, None).await;
    with_held_response(&world, async |_| {
        for row in rows {
            submit(&world.mode, row);
            assert!(settled(&world).await.is_empty());
        }
    })
    .await;
    let before = pending_shells(&world);
    assert_eq!(before.len(), rows.len());
    guard.veto.set(veto);
    submit(&world.mode, "/new");
    assert!(settled(&world).await.is_empty());
    if veto {
        assert_eq!(pending_shells(&world), before, "a veto keeps the rows");
        return;
    }
    let retired = pending_shells(&world).is_empty();
    with_held_response(&world, async |_| {
        submit(&world.mode, "steer");
        // The replacement session runs commands in the default shell.
        submit(&world.mode, "!printf 'printed %s\\n' fresh");
        assert!(settled(&world).await.is_empty());
        let pending = world.mode.pending_messages_container.render(80);
        assert!(!shown(&pending, "printed mark"), "{pending:?}");
        assert!(shown(&pending, "printed fresh"), "{pending:?}");
    })
    .await;
    submit(&world.mode, "after");
    let chat = world.mode.chat_container.render(80);
    assert!(!shown(&chat, "printed mark"), "{chat:?}");
    assert!(shown(&chat, "printed fresh"), "the new row moves: {chat:?}");
    assert!(retired, "the reset retires every row");
}

#[test]
fn pending_shell_rows_do_not_cross_session_reset() {
    isolated(
        "pending_shell_rows_do_not_cross_session_reset",
        async |directory| {
            for rows in [&[][..], &["!mark"], &["!mark", "!mark", "!mark2"]] {
                for veto in [true, false] {
                    pending_reset_case(directory, rows, veto).await;
                }
            }
        },
    );
}

#[test]
fn command_callbacks_release_mode_after_completion() {
    const TEST: &str = "command_callbacks_release_mode_after_completion";
    isolated_case(TEST, "child", &[], async |directory| {
        std::fs::write(directory.join("keybindings.json"), BOUND).unwrap();
        let guard = Guard::default();
        guard.veto.set(true);
        let World {
            mode,
            runtime,
            terminal,
            host,
            ..
        } = commanding(directory, None, &guard, None).await;
        let editor = Rc::clone(&mode.editor);
        let on_submit = editor.on_submit().unwrap();
        on_submit("/new");
        assert_eq!(runtime.futures(), 1);
        let weak = settle_dropped(mode, &runtime).await;
        assert!(weak.upgrade().is_none(), "settled work releases the mode");
        on_submit("/new");
        maestro_tui::tui::InputHandler::handle_input(editor.as_ref(), "\x1bn");
        assert_eq!(runtime.futures(), 0, "callbacks do not resurrect the mode");
        assert_eq!(guard.requests.borrow().len(), 1);
        drop((terminal, host));
        let second = Guard::default();
        let world = commanding(directory, None, &second, None).await;
        submit(&world.mode, "/new");
        assert!(settled(&world).await.is_empty());
        assert_eq!(second.requests.borrow().len(), 1);
        assert!(shown(
            &world.mode.chat_container.render(80),
            "✓ New session started"
        ));
        // An admitted compaction keeps the mode and its instructions through its refusal.
        let third = Guard::default();
        let world = commanding(directory, None, &third, None).await;
        compactable(&world);
        submit(&world.mode, "/compact keep");
        let World { mode, runtime, .. } = world;
        let weak = settle_dropped(mode, &runtime).await;
        drop(runtime.drain_futures());
        assert_eq!(*third.instructions.borrow(), [Some("keep".to_owned())]);
        assert!(weak.upgrade().is_none(), "settled work releases the mode");
    });
}

/// Drops `mode`, checks that the work it handed `runtime` still keeps it, then awaits that
/// work and returns the mode's weak handle.
async fn settle_dropped(
    mode: Rc<InteractiveMode>,
    runtime: &ManualRuntime,
) -> Weak<InteractiveMode> {
    let weak = Rc::downgrade(&mode);
    drop(mode);
    assert!(weak.upgrade().is_some(), "admitted work keeps the mode");
    let errors = admitted(runtime).await;
    assert!(errors.is_empty(), "{errors:?}");
    weak
}

/// One recorded manual compaction request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompactInput {
    /// The stored entries, each shown by its recorded `type` field only.
    entries: Vec<Value>,
    /// The instructions passed to the command, absent when none are.
    #[serde(default)]
    instructions: Option<String>,
    /// Whether the compaction is refused.
    reject: bool,
    /// Whether working progress is shown when the command starts.
    loader: bool,
}

/// What a recorded manual compaction request does.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompactExpected {
    /// Whether the session compacts.
    compacts: bool,
}

/// Completes each recorded entry into a record of its type, chained to the one before.
fn completed(entries: &[Value]) -> Vec<Value> {
    let mut parent = Value::Null;
    let mut records = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let mut record = entry.as_object().unwrap().clone();
        let id = format!("e{index:07}");
        let filled = match entry.get("type").and_then(Value::as_str) {
            Some("message") => json!({"message": super::transcript::user_record(&id)}),
            Some("custom_message") => {
                json!({"customType": "note", "content": "c", "display": true})
            }
            Some("label") => json!({"targetId": "e0000000", "label": "kept"}),
            _ => json!({}),
        };
        record.extend(filled.as_object().unwrap().clone());
        record.extend(
            json!({"id": id, "parentId": parent, "timestamp": "2026-01-02T03:04:05.006Z"})
                .as_object()
                .unwrap()
                .clone(),
        );
        parent = Value::String(id);
        records.push(Value::Object(record));
    }
    records
}

/// Replaces the current session's history with the completed `entries`.
fn store(world: &World, directory: &Path, entries: &[Value]) {
    let header = json!({"type": "session", "version": 3, "id": "stored",
        "timestamp": "2026-01-02T03:04:05.006Z", "cwd": directory.to_str().unwrap()});
    let lines: Vec<String> = std::iter::once(header)
        .chain(completed(entries))
        .map(|record| record.to_string())
        .collect();
    let path = directory.join("stored.jsonl");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let opened = SessionManager::open(path.to_str().unwrap(), None, None).unwrap();
    *world.host.session().session_manager().borrow_mut() = opened;
}

/// One flag per compaction start: whether the working row was already released.
type Starts = Rc<RefCell<Vec<bool>>>;

/// Counts the compaction starts of the current session, recording at each whether the
/// working row was already released.
fn starts(world: &World) -> (Starts, Box<dyn Fn()>) {
    let seen: Starts = Rc::default();
    let mode = Rc::downgrade(&world.mode);
    let recorded = Rc::clone(&seen);
    let unsubscribe = world.host.session().subscribe(Rc::new(move |event| {
        if let (AgentSessionEvent::CompactionStart { .. }, Some(mode)) = (event, mode.upgrade()) {
            let released = mode.working.loading_animation.borrow().is_none();
            recorded.borrow_mut().push(released);
        }
    }));
    (seen, Box::new(unsubscribe))
}

/// Runs one recorded manual compaction request through the command and checks it.
async fn compaction_case(directory: &Path, input: CompactInput, expected: &CompactExpected) {
    let guard = Guard::default();
    guard.compaction.set(if input.reject {
        Compaction::Cancel
    } else {
        Compaction::Supply
    });
    let world = commanding(directory, None, &guard, None).await;
    compactable(&world);
    store(&world, directory, &input.entries);
    if input.loader {
        world.mode.show_working_loader();
    }
    let status = world.mode.status_container.render(80);
    let (seen, unsubscribe) = starts(&world);
    let chat = world.mode.chat_container.children().borrow().len();
    let instructions = input.instructions.clone();
    let outcome = Rc::clone(&world.mode)
        .handle_compact_command(input.instructions)
        .await;
    assert!(outcome.is_ok(), "the command reports no outcome itself");
    assert!(settled(&world).await.is_empty());
    unsubscribe();
    if expected.compacts {
        assert_eq!(*seen.borrow(), [true], "one compaction after the release");
        assert_eq!(*guard.instructions.borrow(), [instructions]);
        return;
    }
    assert!(seen.borrow().is_empty() && guard.instructions.borrow().is_empty());
    assert_eq!(world.mode.status_container.render(80), status);
    assert_eq!(
        world.mode.working.loading_animation.borrow().is_some(),
        input.loader
    );
    warned(&world, chat);
}

/// Checks that the transcript gained only the spaced compaction warning past its first
/// `chat` children.
fn warned(world: &World, chat: usize) {
    let rows = world.mode.chat_container.render(80);
    let theme = world.themes.theme().get().unwrap();
    let warning = painted(
        &theme,
        &[" @warning@Warning: Nothing to compact (no messages yet)\u{1b}[39m".to_owned()],
    );
    assert_eq!(
        world.mode.chat_container.children().borrow().len(),
        chat + 2
    );
    assert!(
        rows.iter().any(|row| row.starts_with(&warning[0])),
        "{rows:?}"
    );
}

#[test]
fn manual_compaction_counts_only_messages_and_leaves_outcomes_to_events() {
    const TEST: &str = "manual_compaction_counts_only_messages_and_leaves_outcomes_to_events";
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<CompactInput, CompactExpected>(FIXTURES, TEST) {
            compaction_case(directory, input, &expected).await;
        }
        // A message outside the selected branch still counts.
        let guard = Guard::default();
        let world = commanding(directory, None, &guard, None).await;
        let message = json!({"type": "message"});
        store(&world, directory, &[message.clone(), message]);
        let session = world.host.session();
        session
            .session_manager()
            .borrow_mut()
            .branch("e0000000")
            .unwrap();
        let (seen, unsubscribe) = starts(&world);
        Rc::clone(&world.mode)
            .handle_compact_command(None)
            .await
            .unwrap();
        unsubscribe();
        assert_eq!(*seen.borrow(), [true], "two stored messages compact");
    });
}

/// One compaction typed in the editor while input is queued during it: how the extension
/// answers, the error its end event carries and the notice it shows instead of a summary.
async fn subscriber_case(
    directory: &Path,
    compaction: Compaction,
    error: Option<&str>,
    notice: Option<&str>,
) {
    let guard = Guard::default();
    guard.compaction.set(compaction);
    let world = commanding(directory, None, &guard, None).await;
    compactable(&world);
    let session = world.host.session();
    super::progress::answer_session(&session);
    session
        .settings_manager()
        .borrow_mut()
        .set_compaction_enabled(false);
    let events: Rc<RefCell<Vec<String>>> = Rc::default();
    let mode = Rc::downgrade(&world.mode);
    let recorded = Rc::clone(&events);
    let unsubscribe = session.subscribe(Rc::new(move |event| match event {
        AgentSessionEvent::CompactionStart { reason } => {
            recorded.borrow_mut().push(format!("start {reason:?}"));
            submit(&mode.upgrade().unwrap(), "queued during compaction");
        }
        AgentSessionEvent::CompactionEnd {
            will_retry,
            error_message,
            ..
        } => {
            assert!(!will_retry);
            recorded.borrow_mut().push(format!("end {error_message:?}"));
        }
        _ => {}
    }));
    submit(&world.mode, "/compact");
    assert!(settled(&world).await.is_empty());
    unsubscribe();
    let ended = format!("end {error:?}");
    assert_eq!(*events.borrow(), ["start Manual".to_owned(), ended]);
    assert!(world.mode.compaction_queued_messages.borrow().is_empty());
    let rows = world.mode.chat_container.render(80);
    match notice {
        Some(text) => {
            let shown = rows
                .iter()
                .filter(|row| strip_ansi(row).unwrap().contains(text));
            assert_eq!(shown.count(), 1, "one event-owned notice: {rows:?}");
        }
        None => assert!(shown(&rows, "Compacted from 123 tokens"), "{rows:?}"),
    }
    let status = world.mode.status_container.render(80);
    assert!(
        !shown(&status, "ompact"),
        "the compaction row is gone: {status:?}"
    );
    assert_eq!(
        delivered(&session, "queued during compaction"),
        1,
        "the queued input reaches the current session once"
    );
}

/// Counts the user messages holding `text` that `session` stored.
fn delivered(session: &AgentSession, text: &str) -> usize {
    session
        .session_manager()
        .borrow()
        .get_entries()
        .iter()
        .filter(|entry| entry["message"]["role"] == "user")
        .filter(|entry| entry.to_string().contains(text))
        .count()
}

#[test]
fn manual_compaction_flows_through_one_progress_subscriber() {
    isolated(
        "manual_compaction_flows_through_one_progress_subscriber",
        async |directory| {
            subscriber_case(directory, Compaction::Supply, None, None).await;
            let cancelled = Some("Error: Compaction cancelled");
            subscriber_case(directory, Compaction::Cancel, None, cancelled).await;
            let failed = "Compaction failed: No API provider registered for api: unknown";
            let notice = format!("Error: {failed}");
            subscriber_case(directory, Compaction::Continue, Some(failed), Some(&notice)).await;
        },
    );
}

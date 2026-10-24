//! Queued steering and follow-up input: snapshots, pending rows, restoration, status lines
//! and the queue key actions over the mode's real session and editor.
use super::super::queues::{CompactionQueuedMessage, RestoreQueuedMessagesOptions};
use super::typed::recalled;
use super::{Setup, World, complete, isolated, painted, started};
use crate::InteractiveMode;
use maestro_app::messages::UserMessageDelivery;
use maestro_app::{
    AgentSessionEvent, AssistantMessage, AssistantMessageEventStream, Cancellation,
    DiagnosticErrorInfo, ExtensionFactory, InputEvent, InputEventResult, Local, PromptOptions,
    RegisteredCommand, StopReason, Usage, is_whitespace_scalar,
};
use maestro_tui::editor_component::TextCallback;
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, EditorComponent, KeybindingKeys, Spacer, Text};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// What a world heard, in order: queue operations, inputs the controlled extension saw,
/// runs of its command and submissions.
type Heard = Rc<RefCell<Vec<Value>>>;

/// Every recorded queue case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_queues.json");

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    super::cases(FIXTURES, test)
}

/// A recorded delivery.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Mode {
    /// Steering.
    Steer,
    /// Follow-up.
    FollowUp,
}

impl Mode {
    /// The session delivery this mode records.
    fn delivery(self) -> UserMessageDelivery {
        match self {
            Self::Steer => UserMessageDelivery::Steer,
            Self::FollowUp => UserMessageDelivery::FollowUp,
        }
    }
}

/// A recorded message kept during compaction.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Kept {
    /// The kept text.
    text: String,
    /// Its delivery.
    mode: Mode,
}

/// The queues a case starts with.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Queues {
    /// Session steering texts.
    #[serde(default)]
    steering: Vec<String>,
    /// Session follow-up texts.
    #[serde(default)]
    follow_up: Vec<String>,
    /// Messages kept during compaction.
    #[serde(default)]
    local: Vec<Kept>,
}

/// Queued texts grouped by delivery.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    /// Steering texts.
    steering: Vec<String>,
    /// Follow-up texts.
    follow_up: Vec<String>,
}

/// What a queue action left behind.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Observed {
    /// The session queue operations the action performed, in order.
    calls: Vec<Value>,
    /// The messages still kept during compaction.
    local: Vec<Kept>,
    /// Every queued text.
    session: Snapshot,
    /// The editor text.
    text: String,
    /// The editor history, oldest first.
    history: Vec<String>,
    /// The error notices the transcript shows.
    errors: Vec<String>,
    /// How many times the running turn was stopped.
    aborts: usize,
    /// The pending rows at width 80.
    pending: Vec<String>,
    /// The transcript rows at width 80.
    chat: Vec<String>,
    /// The failures handed to the terminal host, which no recorded case observes beside a
    /// rejected prompt's own failure.
    #[serde(skip)]
    failures: Vec<String>,
}

impl Observed {
    /// The recorded observation `expected` as the mode paints and stores it: role markers
    /// become the published theme's escapes, and the history keeps each entry the way the
    /// editor's history does, trimmed, nonempty and without an immediate repeat.
    fn expected(world: &World, expected: Value) -> Self {
        let mut expected: Self = serde_json::from_value(expected).unwrap();
        let theme = world.themes.theme().get().unwrap();
        expected.pending = painted(&theme, &expected.pending);
        expected.chat = painted(&theme, &expected.chat);
        let mut history: Vec<String> = Vec::new();
        for entry in expected.history {
            let entry = entry.trim_matches(is_whitespace_scalar);
            if !entry.is_empty() && history.last().map(String::as_str) != Some(entry) {
                history.push(entry.to_owned());
            }
        }
        expected.history = history;
        expected
    }
}

/// Splits the recorded `count` from the rest of an observation.
fn counted(mut expected: Value) -> (Option<usize>, Value) {
    let count = expected
        .as_object_mut()
        .unwrap()
        .shift_remove("count")
        .map(|count| usize::try_from(count.as_u64().unwrap()).unwrap());
    (count, expected)
}

/// The session queue operations a world performs from now on: an emptied queue as
/// `["clear"]`, any other change with its contents.
fn queue_operations(world: &World) -> Heard {
    let calls = Heard::default();
    record_queue(world, &calls);
    calls
}

/// Appends each session queue operation of `world` to `calls` from now on.
fn record_queue(world: &World, calls: &Heard) {
    let recorded = Rc::clone(calls);
    drop(
        world
            .mode
            .runtime_host
            .session()
            .subscribe(Rc::new(move |event| {
                if let AgentSessionEvent::QueueUpdate {
                    steering,
                    follow_up,
                } = event
                {
                    recorded
                        .borrow_mut()
                        .push(if steering.is_empty() && follow_up.is_empty() {
                            json!(["clear"])
                        } else {
                            json!(["update", steering, follow_up])
                        });
                }
            })),
    );
}

/// Queues the case's starting texts in the session and keeps its compaction messages.
fn seed(world: &World, queues: Queues) {
    let session = world.mode.runtime_host.session();
    for text in &queues.steering {
        session.steer(text, &[]).unwrap();
    }
    for text in &queues.follow_up {
        session.follow_up(text, &[]).unwrap();
    }
    world
        .mode
        .compaction_queued_messages
        .borrow_mut()
        .extend(
            queues
                .local
                .into_iter()
                .map(|local| CompactionQueuedMessage {
                    text: local.text,
                    mode: local.mode.delivery(),
                }),
        );
}

/// What `world` holds now, given the queue operations since seeding and the stopped turns.
fn observe(world: &World, calls: &Heard, aborts: usize) -> Observed {
    let mode = &world.mode;
    let local = mode
        .compaction_queued_messages
        .borrow()
        .iter()
        .map(|message| Kept {
            text: message.text.clone(),
            mode: match message.mode {
                UserMessageDelivery::Steer => Mode::Steer,
                UserMessageDelivery::FollowUp => Mode::FollowUp,
            },
        })
        .collect();
    let queued = mode.get_all_queued_messages();
    let text = mode.editor.get_text();
    let mut history = recalled(&mode.editor);
    history.reverse();
    mode.editor.set_text(&text);
    let failures = settled(world);
    let chat = mode.chat_container.render(80);
    let errors = chat
        .iter()
        .filter_map(|row| {
            row.split_once("Error: ")
                .map(|(_, notice)| notice.trim_end().to_owned())
        })
        .collect();
    Observed {
        calls: calls.take(),
        local,
        session: Snapshot {
            steering: queued.steering,
            follow_up: queued.follow_up,
        },
        text,
        history,
        errors,
        aborts,
        pending: mode.pending_messages_container.render(80),
        chat,
        failures,
    }
}

/// Completes the work the mode handed to the terminal host and returns the failures it
/// reported; every handed piece finishes in one step here, as no prompt waits.
pub(super) fn settled(world: &World) -> Vec<String> {
    world
        .runtime
        .take_futures()
        .into_iter()
        .filter_map(|mut future| {
            let polled = std::pin::Pin::new(&mut future)
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
            match polled {
                std::task::Poll::Ready(Err(error)) => Some(
                    error
                        .downcast_ref::<DiagnosticErrorInfo>()
                        .map_or_else(|| error.to_string(), |error| error.message.clone()),
                ),
                std::task::Poll::Ready(Ok(())) => None,
                std::task::Poll::Pending => panic!("handed work is still waiting"),
            }
        })
        .collect()
}

/// The response the controlled provider holds open, and the cancellation of its turn.
struct Held {
    /// The open response.
    stream: AssistantMessageEventStream,
    /// The turn's cancellation.
    signal: Cancellation,
}

impl Held {
    /// How many times the turn was stopped.
    fn aborts(&self) -> usize {
        usize::from(self.signal.is_aborted())
    }
}

/// A finished answer of the controlled provider.
fn answer() -> Arc<RwLock<AssistantMessage>> {
    Arc::new(RwLock::new(AssistantMessage {
        content: Vec::new(),
        api: "unknown".into(),
        provider: "unknown".into(),
        model: "unknown".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0.0,
    }))
}

/// Runs `body` on the started `world` while its session streams a response the controlled
/// provider holds open; then ends that response and waits for the prompt to settle.
fn streaming<T>(world: &World, body: impl FnOnce(&World, &Held) -> T) -> T {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let tasks = tokio::task::LocalSet::new();
    tasks.block_on(&runtime, async {
        let session = world.mode.runtime_host.session();
        world
            .host
            .services()
            .auth_storage
            .set_runtime_api_key("unknown", "key");
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let sender = RefCell::new(Some(sender));
        *session.agent().stream_fn.write().unwrap() = Some(Rc::new(move |_, _, options, _| {
            let stream = AssistantMessageEventStream::new();
            match sender.borrow_mut().take() {
                Some(sender) => drop(sender.send(Held {
                    stream: stream.clone(),
                    signal: options.common.signal.unwrap(),
                })),
                None => stream.end(Some(answer())),
            }
            Box::pin(std::future::ready(Ok(stream)))
        }));
        let prompted = Rc::clone(&session);
        let run = tokio::task::spawn_local(async move {
            prompted.prompt("run", PromptOptions::default()).await
        });
        let held = receiver.await.unwrap();
        assert!(session.is_streaming());
        let result = body(world, &held);
        held.stream.end(Some(answer()));
        run.await.unwrap().unwrap();
        result
    })
}

/// Queues `s, s`, `f` and the compaction messages `lf`, `ls`.
fn mixed() -> Queues {
    Queues {
        steering: vec!["s".into(), "s".into()],
        follow_up: vec!["f".into()],
        local: vec![
            Kept {
                text: "lf".into(),
                mode: Mode::FollowUp,
            },
            Kept {
                text: "ls".into(),
                mode: Mode::Steer,
            },
        ],
    }
}

/// The recorded snapshot operations.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum SnapshotOp {
    /// Read, then clear, every queue.
    MergeClear {
        /// Session steering texts.
        steering: Vec<String>,
        /// Session follow-up texts.
        follow_up: Vec<String>,
        /// Messages kept during compaction.
        local: Vec<Kept>,
    },
}

/// What reading and clearing returned.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Merged {
    /// The snapshot before clearing.
    before: Snapshot,
    /// What clearing returned.
    cleared: Snapshot,
    /// What the clearing left behind.
    after: Value,
}

#[test]
fn queued_texts_merge_by_mode_without_reordering_or_deduplication() {
    const TEST: &str = "queued_texts_merge_by_mode_without_reordering_or_deduplication";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<SnapshotOp, Merged>(TEST) {
            let SnapshotOp::MergeClear {
                steering,
                follow_up,
                local,
            } = input;
            let world = started(directory);
            seed(
                &world,
                Queues {
                    steering,
                    follow_up,
                    local,
                },
            );
            let calls = queue_operations(&world);
            let before = world.mode.get_all_queued_messages();
            assert!(calls.borrow().is_empty(), "reading consumes nothing");
            assert_eq!(world.mode.get_all_queued_messages(), before);
            let cleared = world.mode.clear_all_queues();
            assert_eq!(
                Snapshot {
                    steering: before.steering,
                    follow_up: before.follow_up,
                },
                expected.before
            );
            assert_eq!(
                Snapshot {
                    steering: cleared.steering,
                    follow_up: cleared.follow_up,
                },
                expected.cleared
            );
            assert_eq!(
                observe(&world, &calls, 0),
                Observed::expected(&world, expected.after)
            );
        }
    });
}

/// The recorded pending-row operations.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum PendingOp {
    /// Rows for the starting queues at one width.
    #[serde(rename = "display")]
    Queues {
        /// Session steering texts.
        #[serde(default)]
        steering: Vec<String>,
        /// Session follow-up texts.
        #[serde(default)]
        follow_up: Vec<String>,
        /// Messages kept during compaction.
        #[serde(default)]
        local: Vec<Kept>,
        /// The render width.
        width: usize,
    },
    /// The hint under replaced dequeue bindings, over a tab, styled spaces and a newline.
    Hint {
        /// The replaced bindings.
        bindings: std::collections::BTreeMap<String, Vec<String>>,
    },
    /// Rows before and after the queues are cleared.
    #[serde(rename = "display-clear-sequence")]
    ClearSequence,
}

/// The recorded pending rows.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Rows {
    /// The rows at the case's width.
    Painted {
        /// The rows.
        painted: Vec<String>,
    },
    /// The rows before and after clearing.
    Sequence {
        /// Before clearing.
        before: Vec<String>,
        /// After clearing.
        after: Vec<String>,
    },
}

/// Rebuilds the pending rows of `mode` and renders them at `width`.
fn pending(mode: &InteractiveMode, width: usize) -> Vec<String> {
    mode.update_pending_messages_display().unwrap();
    mode.pending_messages_container.render(width)
}

#[test]
fn pending_rows_keep_labels_live_hint_styles_and_clear_empty() {
    const TEST: &str = "pending_rows_keep_labels_live_hint_styles_and_clear_empty";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<PendingOp, Rows>(TEST) {
            let world = started(directory);
            let theme = world.themes.theme().get().unwrap();
            match (input, expected) {
                (
                    PendingOp::Queues {
                        steering,
                        follow_up,
                        local,
                        width,
                    },
                    Rows::Painted { painted: rows },
                ) => {
                    let queues = Queues {
                        steering,
                        follow_up,
                        local,
                    };
                    seed(&world, queues);
                    assert_eq!(pending(&world.mode, width), painted(&theme, &rows));
                }
                (PendingOp::Hint { bindings }, Rows::Painted { painted: rows }) => {
                    let bindings = bindings
                        .into_iter()
                        .map(|(action, keys)| (action, Some(KeybindingKeys::Multiple(keys))))
                        .collect();
                    maestro_tui::get_keybindings().set_user_bindings(bindings);
                    let queues = Queues {
                        steering: vec!["a\t\x1b[7m \x1b[27m\nb".into()],
                        ..Queues::default()
                    };
                    seed(&world, queues);
                    assert_eq!(pending(&world.mode, 80), painted(&theme, &rows));
                }
                (PendingOp::ClearSequence, Rows::Sequence { before, after }) => {
                    let queues = Queues {
                        steering: vec!["before".into()],
                        ..Queues::default()
                    };
                    seed(&world, queues);
                    assert_eq!(pending(&world.mode, 80), painted(&theme, &before));
                    world.mode.clear_all_queues();
                    assert_eq!(pending(&world.mode, 80), painted(&theme, &after));
                }
                _ => panic!("a display case and its observation disagree"),
            }
        }
    });
}

/// The recorded restorations.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum RestoreOp {
    /// Every queue restored over a draft, stopping the turn or not.
    #[serde(rename = "restore")]
    Grouped {
        /// Session steering texts.
        steering: Vec<String>,
        /// Session follow-up texts.
        follow_up: Vec<String>,
        /// Messages kept during compaction.
        local: Vec<Kept>,
        /// The draft.
        text: String,
        /// Whether the turn stops.
        abort: bool,
    },
    /// One steering message restored over `existing` with a supplied draft.
    #[serde(rename = "restore-override")]
    Override {
        /// The supplied draft, absent when the editor's text is used.
        current_text: Option<String>,
    },
    /// Blank queued texts restored over `draft`.
    #[serde(rename = "restore-blank-sdk-text")]
    BlankSdkText {
        /// The queued steering texts.
        steering: Vec<String>,
    },
}

/// Restores the case's queues over its draft in a streaming `world` and checks what is left.
fn restore_case(world: &World, held: &Held, input: RestoreOp, expected: Value) {
    let (count, expected) = counted(expected);
    let (queues, draft, abort, current_text) = match input {
        RestoreOp::Grouped {
            steering,
            follow_up,
            local,
            text,
            abort,
        } => {
            let queues = Queues {
                steering,
                follow_up,
                local,
            };
            (queues, text, abort, None)
        }
        RestoreOp::Override { current_text } => {
            let queues = Queues {
                steering: vec!["q".into()],
                ..Queues::default()
            };
            (queues, "existing".to_owned(), false, current_text)
        }
        RestoreOp::BlankSdkText { steering } => {
            let queues = Queues {
                steering,
                ..Queues::default()
            };
            (queues, "draft".to_owned(), false, None)
        }
    };
    seed(world, queues);
    world.mode.editor.set_text(&draft);
    let calls = queue_operations(world);
    let options = RestoreQueuedMessagesOptions {
        abort,
        current_text: current_text.as_deref(),
    };
    let restored = world
        .mode
        .restore_queued_messages_to_editor(options)
        .unwrap();
    if let Some(count) = count {
        assert_eq!(restored, count);
    }
    assert_eq!(
        observe(world, &calls, held.aborts()),
        Observed::expected(world, expected)
    );
}

#[test]
fn dequeue_restores_grouped_text_and_preserves_nonblank_draft() {
    const TEST: &str = "dequeue_restores_grouped_text_and_preserves_nonblank_draft";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<RestoreOp, Value>(TEST) {
            streaming(&started(directory), |world, held| {
                restore_case(world, held, input, expected);
            });
        }
    });
}

/// The recorded restorations of empty queues.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum EmptyOp {
    /// No queued message, with a supplied draft, stopping the turn or not.
    RestoreEmpty {
        /// Whether the turn stops.
        abort: bool,
    },
}

#[test]
fn empty_restore_preserves_editor_and_stream_escape_still_aborts() {
    const TEST: &str = "empty_restore_preserves_editor_and_stream_escape_still_aborts";
    isolated(TEST, |directory| {
        for (EmptyOp::RestoreEmpty { abort }, expected) in cases::<EmptyOp, Value>(TEST) {
            let (count, expected) = counted(expected);
            streaming(&started(directory), |world, held| {
                world.mode.editor.set_text("kept");
                world
                    .mode
                    .pending_messages_container
                    .add_child(Rc::new(Spacer::new(1)));
                let calls = queue_operations(world);
                let options = RestoreQueuedMessagesOptions {
                    abort,
                    current_text: Some("override"),
                };
                let restored = world
                    .mode
                    .restore_queued_messages_to_editor(options)
                    .unwrap();
                assert_eq!(Some(restored), count);
                assert_eq!(
                    observe(world, &calls, held.aborts()),
                    Observed::expected(world, expected)
                );
            });
        }
        streaming(&started(directory), |world, held| {
            world.mode.editor.set_text("kept");
            world.mode.editor.handle_input("\x1b");
            assert_eq!(world.mode.editor.get_text(), "kept");
            assert_eq!(
                held.aborts(),
                1,
                "the interrupt key stops an empty-queue turn"
            );
        });
    });
}

/// The recorded dequeue actions.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum DequeueOp {
    /// The dequeue action over compaction messages.
    Dequeue {
        /// The kept messages.
        local: Vec<Kept>,
    },
}

#[test]
fn dequeue_status_uses_exact_zero_singular_and_plural_lines() {
    const TEST: &str = "dequeue_status_uses_exact_zero_singular_and_plural_lines";
    isolated(TEST, |directory| {
        for (DequeueOp::Dequeue { local }, expected) in cases::<DequeueOp, Value>(TEST) {
            let world = started(directory);
            seed(
                &world,
                Queues {
                    local,
                    ..Queues::default()
                },
            );
            let calls = queue_operations(&world);
            let frames = world.terminal.writes().len();
            world.mode.editor.handle_input("\x1b[1;3A");
            world.runtime.settle().unwrap();
            assert!(world.terminal.writes().len() > frames, "a frame is drawn");
            assert_eq!(
                observe(&world, &calls, 0),
                Observed::expected(&world, expected)
            );
        }
    });
}

/// The recorded status sequences.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum StatusOp {
    /// Two statuses, other transcript content, then a third status.
    #[serde(rename = "status-interleaved")]
    Interleaved,
    /// One status, a change to the transcript's end, then a second status.
    #[serde(rename = "status-identity")]
    Identity {
        /// The change.
        change: Change,
    },
}

/// A change to the end of the transcript between two statuses.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Change {
    /// The status spacer replaced by an equal-looking spacer.
    ReplaceSpacer,
    /// The status text removed.
    RemoveLast,
    /// The transcript cleared.
    Clear,
}

/// The recorded transcript.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Transcript {
    /// The transcript rows at width 80.
    painted: Vec<String>,
    /// The transcript's children.
    children: usize,
}

/// Shows the case's statuses around its transcript change.
fn status_sequence(mode: &InteractiveMode, input: StatusOp) {
    let children = || mode.chat_container.children();
    mode.show_status("first").unwrap();
    let change = match input {
        StatusOp::Interleaved => {
            let first = Rc::clone(&children().borrow()[1]);
            mode.show_status("second").unwrap();
            assert_eq!(children().borrow().len(), 2);
            assert!(Rc::ptr_eq(&children().borrow()[1], &first));
            let separator = Text::new("separator".into(), 0, 0, None);
            mode.chat_container.add_child(Rc::new(separator));
            mode.show_status("third").unwrap();
            return;
        }
        StatusOp::Identity { change } => change,
    };
    match change {
        Change::ReplaceSpacer => children().borrow_mut()[0] = Rc::new(Spacer::new(1)),
        Change::RemoveLast => drop(children().borrow_mut().pop()),
        Change::Clear => mode.chat_container.clear(),
    }
    let before = children().borrow().len();
    mode.show_status("second").unwrap();
    assert_eq!(children().borrow().len(), before + 2);
}

#[test]
fn status_replaces_only_the_adjacent_retained_pair() {
    const TEST: &str = "status_replaces_only_the_adjacent_retained_pair";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<StatusOp, Transcript>(TEST) {
            let world = started(directory);
            status_sequence(&world.mode, input);
            let theme = world.themes.theme().get().unwrap();
            let children = world.mode.chat_container.children().borrow().len();
            assert_eq!(children, expected.children);
            assert_eq!(
                world.mode.chat_container.render(80),
                painted(&theme, &expected.painted)
            );
        }
    });
}

/// The recorded key names.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum KeyName {
    /// One key list to capitalize.
    CapitalizeKey {
        /// The keys as bound.
        key: String,
    },
}

/// A recorded displayed key name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Displayed {
    /// The displayed keys.
    result: String,
}

#[test]
fn key_display_capitalizes_each_live_binding_without_reordering() {
    const TEST: &str = "key_display_capitalizes_each_live_binding_without_reordering";
    for (KeyName::CapitalizeKey { key }, expected) in cases::<KeyName, Displayed>(TEST) {
        assert_eq!(InteractiveMode::capitalize_key(&key), expected.result);
    }
    isolated(TEST, |directory| {
        // The mode publishes the application's key registry, which the display reads live.
        let _world = started(directory);
        let keys = maestro_tui::get_keybindings();
        let bind = |keys_bound: Option<&[&str]>| {
            keys.set_user_bindings(vec![(
                "app.message.dequeue".to_owned(),
                keys_bound.map(|bound| {
                    KeybindingKeys::Multiple(bound.iter().map(|key| (*key).to_owned()).collect())
                }),
            )]);
            InteractiveMode::get_app_key_display("app.message.dequeue")
        };
        assert_eq!(bind(None), "Alt+Up");
        assert_eq!(bind(Some(&["shift+x", "ctrl+q"])), "Shift+X/Ctrl+Q");
        assert_eq!(bind(Some(&["ctrl+q", "shift+x"])), "Ctrl+Q/Shift+X");
        assert_eq!(bind(None), "Alt+Up");
    });
}

/// The recorded interrupt key presses.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum EscapeOp {
    /// The interrupt key over every queue and a draft.
    Escape {
        /// Whether a response streams.
        streaming: bool,
    },
}

/// Presses the interrupt key over every queue and the draft `draft`, recording whether the
/// turn was already stopped each time the editor text changed.
fn escape(world: &World, held: Option<&Held>) -> (Heard, Vec<bool>) {
    seed(world, mixed());
    world.mode.editor.set_text("draft");
    let calls = queue_operations(world);
    let stopped = Rc::new(RefCell::new(Vec::new()));
    if let Some(held) = held {
        let (stopped, signal) = (Rc::clone(&stopped), held.signal.clone());
        world.mode.editor.set_on_change(Some(Rc::new(move |_| {
            stopped.borrow_mut().push(signal.is_aborted());
        })));
    }
    world.mode.editor.handle_input("\x1b");
    world.mode.editor.set_on_change(None);
    let stopped = stopped.take();
    (calls, stopped)
}

/// Presses the interrupt key in a streaming or idle world and checks what is left.
fn escape_case(directory: &Path, streams: bool, expected: Value) {
    if !streams {
        let world = started(directory);
        let (calls, _) = escape(&world, None);
        let observed = observe(&world, &calls, 0);
        assert_eq!(observed, Observed::expected(&world, expected));
        return;
    }
    streaming(&started(directory), |world, held| {
        let (calls, stopped) = escape(world, Some(held));
        assert_eq!(
            stopped,
            [false],
            "the text is restored before the turn stops"
        );
        let observed = observe(world, &calls, held.aborts());
        assert_eq!(observed, Observed::expected(world, expected));
    });
}

#[test]
fn stream_escape_restores_before_aborting() {
    const TEST: &str = "stream_escape_restores_before_aborting";
    isolated(TEST, |directory| {
        for (EscapeOp::Escape { streaming }, expected) in cases::<EscapeOp, Value>(TEST) {
            escape_case(directory, streaming, expected);
        }
    });
}

/// A started mode whose session loads the controlled extension and the command `cmd`: the
/// extension's input handler records `["input", text]` and, when `refuse` is set, fails
/// on `selected`; the command records `["command", arguments]`. Queue operations are
/// recorded in the same log.
fn controlled(directory: &Path, refuse: bool) -> (World, Heard) {
    let heard = Heard::default();
    let inputs = Rc::clone(&heard);
    let factory: ExtensionFactory<Local> = Rc::new(move |api| {
        let inputs = Rc::clone(&inputs);
        let registered = api.on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                inputs.borrow_mut().push(json!(["input", event.text]));
                let result = if refuse && event.text == "selected" {
                    Err("refused input".into())
                } else {
                    Ok(None)
                };
                Box::pin(std::future::ready(result))
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    });
    let world = Setup {
        extension: Some(factory),
        ..Setup::default()
    }
    .build(directory);
    complete(world.mode.init()).unwrap();
    world.runtime.settle().unwrap();
    let session = world.mode.runtime_host.session();
    let extension = Rc::clone(&session.resource_loader().get_extensions().extensions[0]);
    let source_info = Arc::clone(&extension.read().unwrap().source_info);
    let runs = Rc::clone(&heard);
    let command = RegisteredCommand::<Local> {
        name: "cmd".into(),
        source_info,
        description: None,
        get_argument_completions: None,
        handler: Rc::new(move |arguments, _| {
            runs.borrow_mut().push(json!(["command", arguments]));
            Box::pin(std::future::ready(Ok(())))
        }),
    };
    let command = Rc::new(RwLock::new(command));
    extension
        .write()
        .unwrap()
        .commands
        .insert("cmd".into(), command);
    if refuse {
        let rejection = DiagnosticErrorInfo::new(None, "controlled rejection");
        drop(
            session
                .extension_runner()
                .on_error(Rc::new(move |_| Err(Box::new(rejection.clone())))),
        );
    }
    record_queue(&world, &heard);
    (world, heard)
}

/// Runs `action` from the start notification of a compaction of `world`'s session, while
/// the session compacts, then lets the compaction end.
fn during_compaction(world: &World, action: impl Fn(&Rc<InteractiveMode>) + 'static) {
    let session = world.mode.runtime_host.session();
    let mode = Rc::downgrade(&world.mode);
    let compacting = Rc::clone(&session);
    let unsubscribe = session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::CompactionStart { .. } = event {
            assert!(compacting.is_compacting());
            action(&mode.upgrade().unwrap());
        }
    }));
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("unknown", "key");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let tasks = tokio::task::LocalSet::new();
    let ended = tasks.block_on(&runtime, session.compact(None));
    assert_eq!(
        ended.unwrap_err().message,
        "Nothing to compact (session too small)"
    );
    unsubscribe();
}

/// Types `plain` and pastes `expanded` so the editor's text differs from its expanded
/// text: a paste of more than ten lines is shown as a marker.
fn paste(world: &World, expanded: &str) {
    let editor = &world.mode.editor;
    editor.set_text("");
    let pasted = format!("{expanded}{}", "\n".repeat(11));
    editor.handle_input(&format!("\x1b[200~{pasted}\x1b[201~"));
    assert_ne!(
        editor.get_text(),
        EditorComponent::get_expanded_text(&**editor).unwrap()
    );
}

/// Replaces the editor's submission with one that records `["submit", text]`, or removes it.
fn record_submission(world: &World, heard: &Heard, installed: bool) {
    let submitted = Rc::clone(heard);
    let submission: Option<TextCallback> = installed.then(|| -> TextCallback {
        Rc::new(move |text: &str| submitted.borrow_mut().push(json!(["submit", text])))
    });
    world.mode.editor.set_on_submit(submission);
}

/// The pending rows recorded for a single queued message, by its delivery and text.
type SingleRows = HashMap<(Mode, String), Vec<String>>;

/// The pending rows every recorded case of `test` shows for exactly one queued message.
fn single_rows(test: &str) -> SingleRows {
    cases::<Value, Value>(test)
        .into_iter()
        .filter_map(|(_, expected)| {
            let snapshot: Snapshot =
                serde_json::from_value(expected.get("session")?.clone()).ok()?;
            let pending: Vec<String> = serde_json::from_value(expected["pending"].clone()).ok()?;
            let single = match (snapshot.steering.as_slice(), snapshot.follow_up.as_slice()) {
                ([queued], []) => (Mode::Steer, queued.clone()),
                ([], [queued]) => (Mode::FollowUp, queued.clone()),
                _ => return None,
            };
            Some((single, pending))
        })
        .collect()
}

/// The recorded observation with each recorded prompt replaced by what the session does
/// with it: a registered command runs with its arguments; other text reaches the input
/// handler and, unless refused, joins the queue the prompt names, which the pending rows
/// then show as the recorded rows of that single message.
fn session_effects(mut expected: Value, refused: bool, rows: &SingleRows) -> Value {
    let mut calls = Vec::new();
    for call in expected["calls"].as_array().unwrap().clone() {
        let [name, text, options] = call.as_array().unwrap().as_slice() else {
            calls.push(call);
            continue;
        };
        assert_eq!(name, "prompt");
        let text = text.as_str().unwrap();
        let (command, arguments) = text
            .strip_prefix('/')
            .map_or(("", ""), |rest| rest.split_once(' ').unwrap_or((rest, "")));
        if command == "cmd" {
            calls.push(json!(["command", arguments]));
            continue;
        }
        calls.push(json!(["input", text]));
        if refused {
            continue;
        }
        let mode: Mode = serde_json::from_value(options["streamingBehavior"].clone()).unwrap();
        let queue = match mode {
            Mode::Steer => "steering",
            Mode::FollowUp => "followUp",
        };
        expected["session"][queue]
            .as_array_mut()
            .unwrap()
            .push(json!(text));
        let session = &expected["session"];
        calls.push(json!(["update", session["steering"], session["followUp"]]));
        expected["pending"] = json!(rows[&(mode, text.to_owned())]);
    }
    expected["calls"] = json!(calls);
    expected
}

/// The recorded enqueues.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum EnqueueOp {
    /// One message kept during compaction over the draft `draft`.
    Enqueue {
        /// Its delivery.
        mode: Mode,
    },
}

#[test]
fn compaction_enqueue_keeps_text_mode_history_and_status() {
    const TEST: &str = "compaction_enqueue_keeps_text_mode_history_and_status";
    isolated(TEST, |directory| {
        for (EnqueueOp::Enqueue { mode }, expected) in cases::<EnqueueOp, Value>(TEST) {
            let world = started(directory);
            world.mode.editor.set_text("draft");
            let calls = queue_operations(&world);
            let queued = world
                .mode
                .queue_compaction_message(" queued ".into(), mode.delivery());
            queued.unwrap();
            let observed = observe(&world, &calls, 0);
            assert_eq!(observed, Observed::expected(&world, expected));
        }
    });
}

/// The recorded command checks.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum DetectionOp {
    /// Whether one text names a registered command.
    ExtensionDetection {
        /// The text.
        text: String,
    },
}

/// A recorded check result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Detected {
    /// Whether the text names a registered command.
    result: bool,
}

#[test]
fn extension_command_detection_uses_slash_and_first_ascii_space() {
    const TEST: &str = "extension_command_detection_uses_slash_and_first_ascii_space";
    isolated(TEST, |directory| {
        let (world, _) = controlled(directory, false);
        for (DetectionOp::ExtensionDetection { text }, expected) in
            cases::<DetectionOp, Detected>(TEST)
        {
            assert_eq!(
                world.mode.is_extension_command(&text),
                expected.result,
                "{text:?}"
            );
        }
    });
}

/// Where a recorded input arrives.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum State {
    /// No response streams and nothing compacts.
    Idle,
    /// A response streams.
    Stream,
    /// The session compacts.
    Compact,
}

/// The recorded follow-up inputs.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum FollowOp {
    /// The follow-up key over one text, in one state.
    Follow {
        /// Where it arrives.
        state: State,
        /// The editor text.
        text: String,
        /// The expanded editor text, the same text here.
        expanded: String,
    },
    /// The follow-up key with no response streaming, over a text whose expansion may
    /// differ, with or without a submission installed.
    FollowFallback {
        /// The editor text.
        text: String,
        /// The expanded editor text, when it differs.
        expanded: Option<String>,
        /// Whether no submission is installed.
        #[serde(default)]
        no_submit: bool,
    },
}

/// Presses the follow-up key in a world the case's state selects and checks what is left.
fn follow_case(directory: &Path, input: FollowOp, expected: Value, rows: &SingleRows) {
    let (world, heard) = controlled(directory, false);
    let expected = session_effects(expected, false, rows);
    let (state, text, installed) = match input {
        FollowOp::Follow {
            state,
            text,
            expanded,
        } => {
            assert_eq!(text, expanded, "the editor expands nothing in these cases");
            (state, text, true)
        }
        FollowOp::FollowFallback {
            text,
            expanded,
            no_submit,
        } => {
            let shown = expanded.unwrap_or_else(|| text.clone());
            (State::Idle, shown, !no_submit)
        }
    };
    record_submission(&world, &heard, installed);
    let press = move |mode: &Rc<InteractiveMode>| {
        mode.editor.set_text(&text);
        mode.handle_follow_up().unwrap();
    };
    // A streaming case is observed while the response still streams, before its end
    // delivers the queued message.
    let observed = match state {
        State::Idle => {
            press(&world.mode);
            observe(&world, &heard, 0)
        }
        State::Stream => streaming(&world, |world, _| {
            heard.borrow_mut().clear();
            press(&world.mode);
            observe(world, &heard, 0)
        }),
        State::Compact => {
            during_compaction(&world, press);
            observe(&world, &heard, 0)
        }
    };
    assert_eq!(observed, Observed::expected(&world, expected));
}

#[test]
fn follow_up_routes_expanded_input_through_idle_stream_and_compaction() {
    const TEST: &str = "follow_up_routes_expanded_input_through_idle_stream_and_compaction";
    isolated(TEST, |directory| {
        let rows = single_rows(TEST);
        for (input, expected) in cases::<FollowOp, Value>(TEST) {
            follow_case(directory, input, expected, &rows);
        }
        let (world, heard) = controlled(directory, false);
        record_submission(&world, &heard, true);
        paste(&world, " pasted ");
        world.mode.handle_follow_up().unwrap();
        assert_eq!(*heard.borrow(), [json!(["submit", "pasted"])]);
    });
}

/// The recorded Enter presses.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum SubmitOp {
    /// Enter over one text while a response streams, during a compaction or not.
    Submit {
        /// Whether the session compacts.
        compacting: bool,
        /// Whether a response streams.
        streaming: bool,
        /// The submitted text.
        text: String,
    },
}

/// Submits `text` through the editor's installed submission while a response streams or
/// the session compacts, and checks what is left.
fn submit_case(directory: &Path, input: SubmitOp, expected: Value, rows: &SingleRows) {
    let SubmitOp::Submit {
        compacting,
        streaming: streams,
        text,
    } = input;
    assert!(
        streams,
        "every recorded Enter arrives while a response streams"
    );
    let (world, heard) = controlled(directory, false);
    let expected = session_effects(expected, false, rows);
    let waiter = world.mode.get_user_input();
    let mut waiter = std::pin::pin!(waiter);
    assert_eq!(super::typed::poll(&mut waiter), None);
    let press = move |mode: &Rc<InteractiveMode>| {
        mode.editor.on_submit().unwrap()(&text);
    };
    let observed = if compacting {
        // A compaction first stops the response, so the compaction branch runs alone.
        during_compaction(&world, press);
        observe(&world, &heard, 0)
    } else {
        // Observed while the response still streams, before its end delivers the message.
        streaming(&world, |world, _| {
            heard.borrow_mut().clear();
            press(&world.mode);
            observe(world, &heard, 0)
        })
    };
    let still_waiting = super::typed::poll(&mut waiter).is_none();
    assert!(still_waiting, "the typed input still waits");
    assert_eq!(observed, Observed::expected(&world, expected));
}

#[test]
fn enter_steers_during_streaming_and_prioritizes_compaction() {
    const TEST: &str = "enter_steers_during_streaming_and_prioritizes_compaction";
    isolated(TEST, |directory| {
        let rows = single_rows(TEST);
        for (input, expected) in cases::<SubmitOp, Value>(TEST) {
            submit_case(directory, input, expected, &rows);
        }
    });
}

/// The recorded prompt rejections.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum RejectionOp {
    /// A streaming prompt the session refuses, sent by the follow-up key or Enter.
    PromptRejection {
        /// The action sending it.
        operation: Operation,
    },
}

/// The action sending a refused prompt.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    /// The follow-up key over a pasted ` selected `.
    Follow,
    /// Enter over `selected`.
    Submit,
}

/// Sends `selected` while a response streams and the session refuses it, then checks
/// that the failure reaches the terminal host with the history and editor already
/// updated and no pending rows refreshed.
fn rejection_case(directory: &Path, operation: Operation, expected: Value) {
    let (world, heard) = controlled(directory, true);
    let mut expected = expected;
    let error = expected
        .as_object_mut()
        .unwrap()
        .shift_remove("error")
        .unwrap();
    let mut expected = Observed::expected(&world, session_effects(expected, true, &HashMap::new()));
    expected.failures = vec![error.as_str().unwrap().to_owned()];
    streaming(&world, |world, _| {
        heard.borrow_mut().clear();
        match operation {
            Operation::Follow => {
                paste(world, " selected ");
                world.mode.handle_follow_up().unwrap();
            }
            Operation::Submit => {
                world.mode.editor.set_text("raw");
                world.mode.editor.on_submit().unwrap()("selected");
            }
        }
    });
    assert_eq!(observe(&world, &heard, 0), expected);
}

/// Types `/cmd arg` and an ordinary text with the follow-up key from a compaction's start
/// notification, checking before the notification returns that the command already ran
/// and the text is already kept.
fn compaction_admission(directory: &Path) {
    let (world, heard) = controlled(directory, false);
    let log = Rc::clone(&heard);
    during_compaction(&world, move |mode| {
        mode.editor.set_text("/cmd arg");
        mode.handle_follow_up().unwrap();
        assert_eq!(*log.borrow(), [json!(["command", "arg"])]);
        assert_eq!(mode.editor.get_text(), "");
        mode.editor.set_text(" kept ");
        mode.handle_follow_up().unwrap();
        let kept = mode.compaction_queued_messages.borrow();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].text, "kept");
        assert!(matches!(kept[0].mode, UserMessageDelivery::FollowUp));
    });
    assert_eq!(settled(&world), Vec::<String>::new());
}

#[test]
fn queue_actions_use_real_session_compaction_and_prompt_routes() {
    const TEST: &str = "queue_actions_use_real_session_compaction_and_prompt_routes";
    isolated(TEST, |directory| {
        for (RejectionOp::PromptRejection { operation }, expected) in
            cases::<RejectionOp, Value>(TEST)
        {
            rejection_case(directory, operation, expected);
        }
        compaction_admission(directory);
    });
}

/// Binds `action` to `keys`, or back to its default.
fn bind(action: &str, keys: Option<&str>) {
    let keys = keys.map(|key| KeybindingKeys::Single(key.to_owned()));
    maestro_tui::get_keybindings().set_user_bindings(vec![(action.to_owned(), keys)]);
}

/// The dequeue key moved to Ctrl+Q and back: only the bound key restores.
fn dequeue_binding(directory: &Path) {
    let world = started(directory);
    let restore = |key: &str, restored: bool| {
        seed(
            &world,
            Queues {
                steering: vec!["queued".into()],
                ..Queues::default()
            },
        );
        world.mode.editor.set_text("");
        world.terminal.send_input(key);
        assert_eq!(
            world.mode.editor.get_text() == "queued",
            restored,
            "{key:?}"
        );
        world.mode.clear_all_queues();
    };
    bind("app.message.dequeue", Some("ctrl+q"));
    restore("\x1b[1;3A", false);
    restore("\x1bq", false);
    restore("\x11", true);
    bind("app.message.dequeue", None);
    restore("\x11", false);
    restore("\x1b[1;3A", true);
}

/// The follow-up key moved to Alt+X and back, with no response streaming: only the bound
/// key forwards the text to the submission.
fn follow_up_binding(directory: &Path) {
    let (world, heard) = controlled(directory, false);
    record_submission(&world, &heard, true);
    let forward = |key: &str, forwarded: bool| {
        heard.borrow_mut().clear();
        world.mode.editor.set_text("typed");
        world.terminal.send_input(key);
        let expected = if forwarded {
            vec![json!(["submit", "typed"])]
        } else {
            Vec::new()
        };
        assert_eq!(*heard.borrow(), expected, "{key:?}");
    };
    bind("app.message.followUp", Some("alt+x"));
    forward("\x1b\r", false);
    forward("\x18", false);
    forward("\x1bx", true);
    bind("app.message.followUp", None);
    forward("\x1bx", false);
    forward("\x1b\r", true);
}

/// The interrupt key moved to Alt+Z while a response streams with nothing queued: only
/// the bound key stops the response.
fn interrupt_binding(directory: &Path) {
    let world = started(directory);
    streaming(&world, |world, held| {
        bind("app.interrupt", Some("alt+z"));
        world.mode.editor.handle_input("\x1b");
        world.mode.editor.handle_input("\x1a");
        assert_eq!(held.aborts(), 0);
        world.mode.editor.handle_input("\x1bz");
        assert_eq!(held.aborts(), 1);
        bind("app.interrupt", None);
    });
}

#[test]
fn queued_actions_follow_live_bindings() {
    isolated("queued_actions_follow_live_bindings", |directory| {
        dequeue_binding(directory);
        follow_up_binding(directory);
        interrupt_binding(directory);
    });
}

#[test]
fn queue_callbacks_release_the_mode_after_active_work_settles() {
    isolated(
        "queue_callbacks_release_the_mode_after_active_work_settles",
        |directory| {
            let world = started(directory);
            let (weak, handed) = streaming(&world, |world, _| {
                world.mode.editor.set_text("later");
                world.terminal.send_input("\x1b\r");
                (Rc::downgrade(&world.mode), world.runtime.take_futures())
            });
            let World { mode, .. } = world;
            drop(mode);
            assert!(weak.upgrade().is_some(), "the handed prompt keeps the mode");
            for mut future in handed {
                let polled = std::pin::Pin::new(&mut future)
                    .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
                assert!(matches!(polled, std::task::Poll::Ready(Ok(()))));
            }
            assert!(weak.upgrade().is_none(), "settled work releases the mode");
        },
    );
}

//! Queued steering and follow-up input: snapshots, pending rows, restoration, status lines
//! and the queue key actions over the mode's real session and editor.
use super::super::queues::{CompactionQueuedMessage, RestoreQueuedMessagesOptions};
use super::typed::recalled;
use super::{World, isolated, painted, recorded, started};
use crate::InteractiveMode;
use maestro_app::messages::UserMessageDelivery;
use maestro_app::{
    AgentSessionEvent, AssistantMessage, AssistantMessageEventStream, Cancellation, PromptOptions,
    StopReason, Usage, is_whitespace_scalar,
};
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, EditorComponent, KeybindingKeys, Spacer, Text};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Every recorded queue case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_queues.json");

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    recorded(FIXTURES, test)
}

/// A recorded delivery.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
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
struct Local {
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
    local: Vec<Local>,
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
    local: Vec<Local>,
    /// Every queued text.
    session: Snapshot,
    /// The editor text.
    text: String,
    /// The editor history, oldest first.
    history: Vec<String>,
    /// Failures handed to the terminal host.
    errors: Vec<String>,
    /// How many times the running turn was stopped.
    aborts: usize,
    /// The pending rows at width 80.
    pending: Vec<String>,
    /// The transcript rows at width 80.
    chat: Vec<String>,
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
fn queue_operations(world: &World) -> Rc<RefCell<Vec<Value>>> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&calls);
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
    calls
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
fn observe(world: &World, calls: &Rc<RefCell<Vec<Value>>>, aborts: usize) -> Observed {
    let mode = &world.mode;
    let local = mode
        .compaction_queued_messages
        .borrow()
        .iter()
        .map(|message| Local {
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
    let errors = world
        .runtime
        .take_futures()
        .into_iter()
        .filter_map(|mut future| {
            let polled = std::pin::Pin::new(&mut future)
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
            match polled {
                std::task::Poll::Ready(Err(error)) => Some(error.to_string()),
                std::task::Poll::Ready(Ok(())) => None,
                std::task::Poll::Pending => panic!("a key action left work running"),
            }
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
        chat: mode.chat_container.render(80),
    }
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

/// Runs `body` on a started mode whose session streams a response the controlled provider
/// holds open; then ends that response and waits for the prompt to settle.
fn streaming<T>(directory: &Path, body: impl FnOnce(&World, &Held) -> T) -> T {
    let world = started(directory);
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
        let result = body(&world, &held);
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
            Local {
                text: "lf".into(),
                mode: Mode::FollowUp,
            },
            Local {
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
        local: Vec<Local>,
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
        local: Vec<Local>,
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
        local: Vec<Local>,
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
            streaming(directory, |world, held| {
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
            streaming(directory, |world, held| {
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
        streaming(directory, |world, held| {
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
        local: Vec<Local>,
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
fn escape(world: &World, held: Option<&Held>) -> (Rc<RefCell<Vec<Value>>>, Vec<bool>) {
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
    streaming(directory, |world, held| {
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

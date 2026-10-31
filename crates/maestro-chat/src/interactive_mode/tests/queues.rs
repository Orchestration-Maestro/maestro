//! Queued steering and follow-up input: snapshots, pending rows, restoration, status lines
//! and the queue key actions over the mode's real session and editor.
use super::super::queues::{CompactionQueuedMessage, RestoreQueuedMessagesOptions};
use super::bash::shelling;
use super::typed::{poll, recalled, submit};
use super::{
    Held, Setup, World, answer, border_row, driving, echoing_shell, holding, isolated, painted,
    rendered_border, scripted, select_level, start_commands, start_shells, started, streaming,
    transcript_positions,
};
use crate::BashExecutionComponent;
use crate::InteractiveMode;
use crate::interactive_mode::bash::component_address;
use maestro_app::messages::UserMessageDelivery;
use maestro_app::{
    AgentSessionEvent, AssistantMessageEventStream, Cancellation, Context, DiagnosticErrorInfo,
    Extension, ExtensionFactory, InputEvent, InputEventResult, Local, Message, PromptTemplate,
    RegisteredCommand, SourceInfo, SourceOrigin, SourceScope, ThinkingLevel, UserBlock,
    UserContent, is_whitespace_scalar,
};
use maestro_theme::ThemeColor;
use maestro_tui::editor_component::TextCallback;
use maestro_tui::tui::{InputHandler, LocalFuture};
use maestro_tui::{Component, EditorComponent, KeybindingKeys, Spacer, Text};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    path::Path,
    pin::Pin,
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
pub(super) enum Mode {
    /// Steering.
    Steer,
    /// Follow-up.
    FollowUp,
}

impl Mode {
    /// The session delivery this mode records.
    pub(super) fn delivery(self) -> UserMessageDelivery {
        match self {
            Self::Steer => UserMessageDelivery::Steer,
            Self::FollowUp => UserMessageDelivery::FollowUp,
        }
    }
}

/// A recorded message kept during compaction.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Kept {
    /// The kept text.
    pub(super) text: String,
    /// Its delivery.
    pub(super) mode: Mode,
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
    fn expected(world: &World, mut expected: Value) -> Self {
        // The pending-row sample a delivered compaction case keeps is not an observation.
        if let Some(object) = expected.as_object_mut() {
            object.shift_remove("pendingSample");
        }
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
    // The recording seeds the queues without showing them; the session's queue updates
    // drew the rows here, so the case starts from the recorded empty rows.
    world.mode.pending_messages_container.clear();
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
fn settled(world: &World) -> Vec<String> {
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
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<SnapshotOp, Merged>(TEST) {
            let SnapshotOp::MergeClear {
                steering,
                follow_up,
                local,
            } = input;
            let world = started(directory).await;
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
            // The session's queue update drew the compaction rows still kept while clearing;
            // the only caller refreshes the rows right after, as the recording shows.
            world.mode.update_pending_messages_display().unwrap();
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
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<PendingOp, Rows>(TEST) {
            let world = started(directory).await;
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
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<RestoreOp, Value>(TEST) {
            streaming(&started(directory).await, async |world, held| {
                restore_case(world, held, input, expected);
            })
            .await;
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
    isolated(TEST, async |directory| {
        for (EmptyOp::RestoreEmpty { abort }, expected) in cases::<EmptyOp, Value>(TEST) {
            let (count, expected) = counted(expected);
            streaming(&started(directory).await, async |world, held| {
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
            })
            .await;
        }
        streaming(&started(directory).await, async |world, held| {
            world.mode.editor.set_text("kept");
            world.mode.editor.handle_input("\x1b");
            assert_eq!(world.mode.editor.get_text(), "kept");
            assert_eq!(
                held.aborts(),
                1,
                "the interrupt key stops an empty-queue turn"
            );
        })
        .await;
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
    isolated(TEST, async |directory| {
        for (DequeueOp::Dequeue { local }, expected) in cases::<DequeueOp, Value>(TEST) {
            let world = started(directory).await;
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
    isolated(TEST, async |directory| {
        for (input, expected) in cases::<StatusOp, Transcript>(TEST) {
            let world = started(directory).await;
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
        assert_eq!(
            maestro_app::format_key_text(&key, true),
            super::platform_keys::on_this_platform(&expected.result)
        );
    }
    isolated(TEST, async |directory| {
        // The mode publishes the application's key registry, which the display reads live.
        let _world = started(directory).await;
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
        assert_eq!(bind(None), super::platform_keys::on_this_platform("Alt+Up"));
        assert_eq!(bind(Some(&["shift+x", "ctrl+q"])), "Shift+X/Ctrl+Q");
        assert_eq!(bind(Some(&["ctrl+q", "shift+x"])), "Ctrl+Q/Shift+X");
        assert_eq!(bind(None), super::platform_keys::on_this_platform("Alt+Up"));
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
async fn escape_case(directory: &Path, streams: bool, expected: Value) {
    if !streams {
        let world = started(directory).await;
        let (calls, _) = escape(&world, None);
        let observed = observe(&world, &calls, 0);
        assert_eq!(observed, Observed::expected(&world, expected));
        return;
    }
    streaming(&started(directory).await, async |world, held| {
        let (calls, stopped) = escape(world, Some(held));
        assert_eq!(
            stopped,
            [false],
            "the text is restored before the turn stops"
        );
        let observed = observe(world, &calls, held.aborts());
        assert_eq!(observed, Observed::expected(world, expected));
    })
    .await;
}

#[test]
fn stream_escape_restores_before_aborting() {
    const TEST: &str = "stream_escape_restores_before_aborting";
    isolated(TEST, async |directory| {
        for (EscapeOp::Escape { streaming }, expected) in cases::<EscapeOp, Value>(TEST) {
            escape_case(directory, streaming, expected).await;
        }
    });
}

/// The controlled extension: its input handler records `["input", text]` and, when
/// `refuse` is set, fails on `selected`; its shell handler records
/// `["execute", command, excluded]` and answers the command without running one.
fn controlled_factory(heard: &Heard, refuse: bool) -> ExtensionFactory<Local> {
    let inputs = Rc::clone(heard);
    let executed = Rc::clone(heard);
    Rc::new(move |api| {
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
        let shells = Rc::clone(&executed);
        let answered = api.on::<maestro_app::UserBashEvent, _>(
            "user_bash".to_owned(),
            Rc::new(move |event: &mut maestro_app::UserBashEvent, _| {
                shells.borrow_mut().push(json!([
                    "execute",
                    event.command,
                    event.exclude_from_context
                ]));
                let result = maestro_app::BashResult {
                    output: String::new(),
                    exit_code: Some(0.0),
                    cancelled: false,
                    truncated: false,
                    full_output_path: None,
                };
                let answer: maestro_app::UserBashEventResult<
                    super::ShellOperations,
                    maestro_app::BashResult,
                > = maestro_app::UserBashEventResult {
                    operations: None,
                    result: maestro_app::Presence::Present(result),
                };
                Box::pin(std::future::ready(Ok(Some(answer))))
            }),
        );
        Box::pin(std::future::ready(registered.and(answered).map(drop)))
    })
}

/// A started mode whose session loads the controlled extension and the command `cmd`: the
/// extension's input handler records `["input", text]` and, when `refuse` is set, fails
/// on `selected`; its shell handler records `["execute", command, excluded]` and answers
/// the command without running one; the command `cmd` records `["command", arguments]` and
/// the command `setter` sets the editor's text. Queue operations are recorded in the same
/// log.
async fn controlled(directory: &Path, refuse: bool) -> (World, Heard) {
    controlled_in(directory, refuse, Setup::default()).await
}

/// The prompt template the expansion witnesses type: `/greet world` expands to `Hello world`.
fn greeting() -> PromptTemplate {
    PromptTemplate {
        name: "greet".into(),
        description: "Greet someone".into(),
        argument_hint: None,
        content: "Hello $1".into(),
        source_info: SourceInfo {
            path: "greet.md".into(),
            source: "local".into(),
            scope: SourceScope::Project,
            origin: SourceOrigin::TopLevel,
            base_dir: None,
        },
        file_path: "greet.md".into(),
    }
}

/// Registers on `extension` the command `cmd`, which records its arguments in `heard`, and
/// the command `setter`, which sets the editor's text to its arguments.
fn register_commands(extension: &Rc<RwLock<Extension<Local>>>, heard: &Heard) {
    let source_info = Arc::clone(&extension.read().unwrap().source_info);
    let runs = Rc::clone(heard);
    let recording = RegisteredCommand::<Local> {
        name: "cmd".into(),
        source_info: Arc::clone(&source_info),
        description: None,
        get_argument_completions: None,
        handler: Rc::new(move |arguments, _| {
            runs.borrow_mut().push(json!(["command", arguments]));
            Box::pin(std::future::ready(Ok(())))
        }),
    };
    let setting = RegisteredCommand::<Local> {
        name: "setter".into(),
        source_info,
        description: None,
        get_argument_completions: None,
        handler: Rc::new(|arguments, context| {
            Box::pin(async move {
                context.ui()?.set_editor_text(arguments)?;
                Ok(())
            })
        }),
    };
    for command in [recording, setting] {
        let name = command.name.clone();
        extension
            .write()
            .unwrap()
            .commands
            .insert(name, Rc::new(RwLock::new(command)));
    }
}

/// [`controlled`] over a session that also loads the prompt template [`greeting`].
async fn templated(directory: &Path) -> (World, Heard) {
    let setup = Setup {
        prompts: vec![vec![greeting()]],
        ..Setup::default()
    };
    controlled_in(directory, false, setup).await
}

/// [`controlled`] over the session `setup` describes, whose extension is the controlled one.
async fn controlled_in(directory: &Path, refuse: bool, setup: Setup) -> (World, Heard) {
    let heard = Heard::default();
    let factory = controlled_factory(&heard, refuse);
    let world = Setup {
        extension: Some(factory),
        ..setup
    }
    .build(directory);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    // The recorded transcripts start after the startup resource listing.
    world.mode.chat_container.clear();
    let session = world.mode.runtime_host.session();
    register_commands(
        &session.resource_loader().get_extensions().extensions[0],
        &heard,
    );
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

/// The failure the harness's own compaction always ends with.
const COMPACTION_FAILURE: &str = "Compaction failed: Nothing to compact (session too small)";

/// The row the mode shows for the error `text`, painted and padded.
fn error_row(world: &World, text: &str) -> String {
    let painted_row = painted(
        &world.themes.theme().get().unwrap(),
        &[format!(" @error@{text}\x1b[39m")],
    )
    .remove(0);
    let pad = 80usize.saturating_sub(text.chars().count() + 1);
    format!("{painted_row}{}", " ".repeat(pad))
}

/// Appends a notice the mode shows for a compaction that failed, painted and padded.
fn with_notice(mut observed: Observed, world: &World, text: &str) -> Observed {
    observed
        .chat
        .extend([String::new(), error_row(world, text)]);
    observed
}

/// Appends the notice the harness's failing compaction ends with, which the mode shows for a
/// manual compaction.
fn with_compaction_notice(observed: Observed, world: &World) -> Observed {
    let mut observed = observed;
    let delivered = observed
        .chat
        .iter()
        .position(|row| row.starts_with("\x1b]133;A"))
        .map(|start| observed.chat.split_off(start - 1))
        .unwrap_or_default();
    let named = format!("Error: {COMPACTION_FAILURE}");
    let mut observed = with_notice(observed, world, &named);
    let prompted = !delivered.is_empty();
    observed.chat.extend(delivered);
    observed
        .errors
        .push(format!("{COMPACTION_FAILURE}\x1b[39m"));
    if prompted {
        // The delivered prompt then fails to start a response, and the mode shows that failure.
        observed.chat.extend([
            "\x1b]133;A\x07".to_owned(),
            format!(
                "\x1b]133;B\x07\x1b]133;C\x07{}",
                error_row(world, &format!("Error: {PROVIDER_FAILURE}"))
            ),
        ]);
        observed.errors.push(format!("{PROVIDER_FAILURE}\x1b[39m"));
    }
    observed
}

/// The failure of the delivered input when no provider is registered.
const PROVIDER_FAILURE: &str = "No API provider registered for api: unknown";

/// The notice the automatic compaction attempt ends with when no provider is registered.
const AUTO_COMPACTION_FAILURE: &str =
    "Auto-compaction failed: No API provider registered for api: unknown";

/// Runs `action` from the start notification of a compaction of `world`'s session, while
/// the session compacts, then lets the compaction end.
pub(super) async fn during_compaction(
    world: &World,
    action: impl Fn(&Rc<InteractiveMode>) + 'static,
) {
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
    async {
        let ended = session.compact(None).await;
        assert_eq!(
            ended.unwrap_err().message,
            "Nothing to compact (session too small)"
        );
        // The compaction's end delivers the input it captured, on the terminal host.
        super::progress::settle_host(world).await;
    }
    .await;
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
            if let Some(sample) = expected.get("pendingSample") {
                let mode: Mode = serde_json::from_value(sample["mode"].clone()).ok()?;
                let queued = sample["text"].as_str()?.to_owned();
                let rows: Vec<String> = serde_json::from_value(sample["rows"].clone()).ok()?;
                return Some(((mode, queued), rows));
            }
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
    isolated(TEST, async |directory| {
        for (EnqueueOp::Enqueue { mode }, expected) in cases::<EnqueueOp, Value>(TEST) {
            let world = started(directory).await;
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
    isolated(TEST, async |directory| {
        let (world, _) = controlled(directory, false).await;
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
async fn follow_case(directory: &Path, input: FollowOp, expected: Value, rows: &SingleRows) {
    let (world, heard) = controlled(directory, false).await;
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
        State::Stream => {
            streaming(&world, async |world, _| {
                heard.borrow_mut().clear();
                press(&world.mode);
                observe(world, &heard, 0)
            })
            .await
        }
        State::Compact => {
            during_compaction(&world, press).await;
            observe(&world, &heard, 0)
        }
    };
    let expected = Observed::expected(&world, expected);
    let expected = if matches!(state, State::Compact) {
        with_compaction_notice(expected, &world)
    } else {
        expected
    };
    assert_eq!(observed, expected);
}

#[test]
fn follow_up_routes_expanded_input_through_idle_stream_and_compaction() {
    const TEST: &str = "follow_up_routes_expanded_input_through_idle_stream_and_compaction";
    isolated(TEST, async |directory| {
        let rows = single_rows(TEST);
        for (input, expected) in cases::<FollowOp, Value>(TEST) {
            follow_case(directory, input, expected, &rows).await;
        }
        let (world, heard) = controlled(directory, false).await;
        record_submission(&world, &heard, true);
        paste(&world, " pasted ");
        world.mode.handle_follow_up().unwrap();
        assert_eq!(*heard.borrow(), [json!(["submit", "pasted"])]);
    });
}

/// The user texts of every request the session's provider received, one list per request.
type Requests = Rc<RefCell<Vec<Vec<String>>>>;

/// The text of a user message: its text, or its text blocks in order.
fn text_of(content: &UserContent) -> String {
    match content {
        UserContent::Text(text) => text.clone(),
        UserContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                UserBlock::Text(text) => Some(text.text.as_str()),
                UserBlock::Image(_) => None,
            })
            .collect(),
    }
}

/// The user texts of the request `context` carries.
fn user_texts(context: &Context) -> Vec<String> {
    context
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User(user) => Some(text_of(&user.content)),
            _ => None,
        })
        .collect()
}

/// The user texts `session` holds, in order: what its next request is built from.
fn conversation_texts(session: &maestro_app::AgentSession<Local>) -> Vec<String> {
    let messages = session.messages();
    let messages = messages.read().unwrap();
    messages
        .iter()
        .filter_map(|message| match message {
            maestro_app::AgentMessage::User(user) => Some(text_of(&user.read().unwrap().content)),
            _ => None,
        })
        .collect()
}

/// Points the session's provider at an answer that comes at once and records the user texts
/// of each request it receives from now on. The zero-usage answers would otherwise start an
/// automatic compaction of their own, so the session compacts only on request.
fn answering(world: &World) -> Requests {
    world
        .mode
        .runtime_host
        .session()
        .set_auto_compaction_enabled(false);
    world
        .host
        .services()
        .auth_storage
        .set_runtime_api_key("unknown", "key");
    let requests = Requests::default();
    let seen = Rc::clone(&requests);
    *world
        .mode
        .runtime_host
        .session()
        .agent()
        .stream_fn
        .write()
        .unwrap() = Some(Rc::new(move |_, context, _, _| {
        seen.borrow_mut().push(user_texts(&context));
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(answer()));
        Box::pin(std::future::ready(Ok(stream)))
    }));
    requests
}

#[test]
fn follow_up_sends_a_template_to_the_provider_expanded() {
    const TEST: &str = "follow_up_sends_a_template_to_the_provider_expanded";
    isolated(TEST, async |directory| {
        let (world, _) = templated(directory).await;
        streaming(&world, async |world, _| {
            world.mode.editor.set_text("/greet world");
            world.mode.handle_follow_up().unwrap();
            assert_eq!(settled(world), Vec::<String>::new());
            assert_eq!(
                world.mode.get_all_queued_messages().follow_up,
                ["Hello world"],
                "the session queues the expansion, not what was typed"
            );
        })
        .await;
        assert_eq!(
            conversation_texts(&world.mode.runtime_host.session()),
            ["run", "Hello world"],
            "the follow-up turn carries the expansion"
        );
    });
}

#[test]
fn compaction_keeps_a_template_as_typed_and_sends_it_expanded() {
    const TEST: &str = "compaction_keeps_a_template_as_typed_and_sends_it_expanded";
    isolated(TEST, async |directory| {
        let (world, _) = templated(directory).await;
        let requests = answering(&world);
        let kept = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&kept);
        during_compaction(&world, move |mode| {
            mode.editor.set_text("/greet world");
            mode.handle_follow_up().unwrap();
            let queued = mode.compaction_queued_messages.borrow();
            seen.borrow_mut()
                .extend(queued.iter().map(|message| message.text.clone()));
        })
        .await;
        assert_eq!(
            *kept.borrow(),
            ["/greet world"],
            "the compaction keeps the typed text"
        );
        assert_eq!(
            *requests.borrow(),
            [["Hello world"]],
            "the delivery after the compaction sends the expansion"
        );
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
async fn submit_case(directory: &Path, input: SubmitOp, expected: Value, rows: &SingleRows) {
    let SubmitOp::Submit {
        compacting,
        streaming: streams,
        text,
    } = input;
    assert!(
        streams,
        "every recorded Enter arrives while a response streams"
    );
    let (world, heard) = controlled(directory, false).await;
    let expected = session_effects(expected, false, rows);
    let waiter = world.mode.get_user_input();
    let mut waiter = std::pin::pin!(waiter);
    assert_eq!(super::typed::poll(&mut waiter), None);
    let press = move |mode: &Rc<InteractiveMode>| {
        mode.editor.on_submit().unwrap()(&text);
    };
    let observed = if compacting {
        // A compaction first stops the response, so the compaction branch runs alone.
        during_compaction(&world, press).await;
        observe(&world, &heard, 0)
    } else {
        // Observed while the response still streams, before its end delivers the message.
        streaming(&world, async |world, _| {
            heard.borrow_mut().clear();
            press(&world.mode);
            observe(world, &heard, 0)
        })
        .await
    };
    let still_waiting = super::typed::poll(&mut waiter).is_none();
    assert!(still_waiting, "the typed input still waits");
    let expected = Observed::expected(&world, expected);
    let expected = if compacting {
        with_compaction_notice(expected, &world)
    } else {
        expected
    };
    assert_eq!(observed, expected);
}

#[test]
fn enter_steers_during_streaming_and_prioritizes_compaction() {
    const TEST: &str = "enter_steers_during_streaming_and_prioritizes_compaction";
    isolated(TEST, async |directory| {
        let rows = single_rows(TEST);
        for (input, expected) in cases::<SubmitOp, Value>(TEST) {
            submit_case(directory, input, expected, &rows).await;
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
async fn rejection_case(directory: &Path, operation: Operation, expected: Value) {
    let (world, heard) = controlled(directory, true).await;
    let mut expected = expected;
    let error = expected
        .as_object_mut()
        .unwrap()
        .shift_remove("error")
        .unwrap();
    let mut expected = Observed::expected(&world, session_effects(expected, true, &HashMap::new()));
    expected.failures = vec![error.as_str().unwrap().to_owned()];
    streaming(&world, async |world, _| {
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
    })
    .await;
    // The bound error listener shows the input handler's failure first, attributed to its
    // extension.
    let heading = "Extension \"<inline>\" error: refused input";
    let shown = painted(
        &world.themes.theme().get().unwrap(),
        &[format!(" @error@{heading}\x1b[39m")],
    )
    .remove(0);
    expected.chat.insert(
        0,
        format!("{shown}{}", " ".repeat(80 - heading.chars().count() - 1)),
    );
    let expected = with_notice(expected, &world, AUTO_COMPACTION_FAILURE);
    assert_eq!(observe(&world, &heard, 0), expected);
}

/// Types `/cmd arg`, `/setter` and an ordinary text with the follow-up key from a
/// compaction's start notification, checking before the notification returns that each
/// command already ran, that the editor holds what `setter` put there and that the text is
/// already kept.
async fn compaction_admission(directory: &Path) {
    let (world, heard) = controlled(directory, false).await;
    let log = Rc::clone(&heard);
    during_compaction(&world, move |mode| {
        mode.editor.set_text("/cmd arg");
        mode.handle_follow_up().unwrap();
        assert_eq!(*log.borrow(), [json!(["command", "arg"])]);
        assert_eq!(mode.editor.get_text(), "");
        mode.editor.set_text("/setter set by the command");
        mode.handle_follow_up().unwrap();
        assert_eq!(
            mode.editor.get_text(),
            "set by the command",
            "the editor is cleared before the command runs, not after"
        );
        mode.editor.set_text(" kept ");
        mode.handle_follow_up().unwrap();
        let kept = mode.compaction_queued_messages.borrow();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].text, "kept");
        assert!(matches!(kept[0].mode, UserMessageDelivery::FollowUp));
    })
    .await;
    assert_eq!(settled(&world), Vec::<String>::new());
}

#[test]
fn queue_actions_use_real_session_compaction_and_prompt_routes() {
    const TEST: &str = "queue_actions_use_real_session_compaction_and_prompt_routes";
    isolated(TEST, async |directory| {
        for (RejectionOp::PromptRejection { operation }, expected) in
            cases::<RejectionOp, Value>(TEST)
        {
            rejection_case(directory, operation, expected).await;
        }
        compaction_admission(directory).await;
    });
}

/// Binds `action` to `keys`, or back to its default.
fn bind(action: &str, keys: Option<&str>) {
    let keys = keys.map(|key| KeybindingKeys::Single(key.to_owned()));
    maestro_tui::get_keybindings().set_user_bindings(vec![(action.to_owned(), keys)]);
}

/// The dequeue key moved to Ctrl+Q and back: only the bound key restores.
async fn dequeue_binding(directory: &Path) {
    let world = started(directory).await;
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
async fn follow_up_binding(directory: &Path) {
    let (world, heard) = controlled(directory, false).await;
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
async fn interrupt_binding(directory: &Path) {
    let world = started(directory).await;
    streaming(&world, async |world, held| {
        bind("app.interrupt", Some("alt+z"));
        world.mode.editor.handle_input("\x1b");
        world.mode.editor.handle_input("z");
        assert_eq!(held.aborts(), 0);
        world.mode.editor.handle_input("\x1bz");
        assert_eq!(held.aborts(), 1);
        bind("app.interrupt", None);
    })
    .await;
    // Back on its default, Escape stops a later response again and Alt+Z no longer does.
    streaming(&world, async |world, held| {
        world.mode.editor.handle_input("\x1bz");
        assert_eq!(held.aborts(), 0);
        world.mode.editor.handle_input("\x1b");
        assert_eq!(held.aborts(), 1);
    })
    .await;
}

#[test]
fn queued_actions_follow_live_bindings() {
    isolated("queued_actions_follow_live_bindings", async |directory| {
        dequeue_binding(directory).await;
        follow_up_binding(directory).await;
        interrupt_binding(directory).await;
    });
}

#[test]
fn queue_callbacks_release_the_mode_after_active_work_settles() {
    isolated(
        "queue_callbacks_release_the_mode_after_active_work_settles",
        async |directory| {
            let world = started(directory).await;
            let (weak, handed) = streaming(&world, async |world, _| {
                world.mode.editor.set_text("later");
                world.terminal.send_input("\x1b\r");
                (Rc::downgrade(&world.mode), world.runtime.take_futures())
            })
            .await;
            let World { mode, runtime, .. } = world;
            let editor = Rc::clone(&mode.editor);
            drop(mode);
            assert!(weak.upgrade().is_some(), "the handed prompt keeps the mode");
            for mut future in handed {
                let polled = std::pin::Pin::new(&mut future)
                    .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()));
                assert!(matches!(polled, std::task::Poll::Ready(Ok(()))));
            }
            assert!(weak.upgrade().is_none(), "settled work releases the mode");
            // The editor outlives the mode, and the queue keys it still answers hand the
            // runtime nothing: interrupt, follow-up and dequeue.
            editor.set_text("later");
            for key in ["\x1b", "\x1b\r", DEQUEUE_KEY] {
                editor.handle_input(key);
                assert_eq!(runtime.futures(), 0, "{key:?} hands work to the runtime");
                assert!(weak.upgrade().is_none(), "{key:?} revives the mode");
            }
        },
    );
}

/// The draft an interrupt witness holds in the editor.
const DRAFT: &str = "draft";

/// The draft the interrupt key restores a queued message above.
const QUEUED_DRAFT: &str = "queued\n\ndraft";

/// The draft that selects shell mode, which the interrupt key can clear.
const SHELL_DRAFT: &str = "!draft";

/// The shell command that keeps running until its release file appears.
const HELD: &str = "!hold";

/// The interrupt key's selection: whether a response streams, whether a shell runs and
/// whether the editor holds a shell command.
#[derive(Clone, Copy, Debug)]
struct Interrupt {
    /// Whether the session streams a held response.
    streaming: bool,
    /// Whether a shell command runs.
    running: bool,
    /// Whether the editor holds a shell command.
    mode: bool,
}

impl Interrupt {
    /// The editor text this selection starts with.
    fn draft(self) -> &'static str {
        if self.mode { SHELL_DRAFT } else { DRAFT }
    }

    /// The queued text this selection starts with: one steering message where the restore
    /// is observable and none where it would replace the shell draft.
    fn queued(self) -> Queues {
        if !self.streaming || self.mode {
            return Queues::default();
        }
        Queues {
            steering: vec!["queued".into()],
            ..Queues::default()
        }
    }

    /// The editor text the interrupt key leaves behind.
    fn text(self) -> &'static str {
        match (self.streaming, self.running, self.mode) {
            // A streaming response restores the queued text before the draft.
            (true, _, false) => QUEUED_DRAFT,
            (true, _, true) | (false, true, _) => self.draft(),
            (false, false, true) => "",
            (false, false, false) => DRAFT,
        }
    }
}

/// The file whose creation lets the held shell end.
const HELD_RELEASE: &str = "held-release";

/// A started mode whose shell waits for the release file.
pub(super) async fn held_shell(directory: &Path) -> World {
    let release = directory.join(HELD_RELEASE);
    let _ = std::fs::remove_file(&release);
    let script = scripted(
        directory,
        "shell",
        &format!(
            "while [ ! -e {} ]; do :; done\nprintf 'held\\n'",
            release.display()
        ),
    );
    shelling(directory, &script).await
}

/// Submits the held shell command and returns the command it admitted once the session
/// runs it.
pub(super) fn start_held(world: &World) -> LocalFuture {
    submit(&world.mode, HELD);
    let mut admitted = world.runtime.take_futures();
    assert_eq!(admitted.len(), 1, "the submission admits one command");
    let mut command = admitted.pop().unwrap();
    assert!(
        poll(&mut command).is_none(),
        "the held shell waits for its release"
    );
    assert!(
        world.mode.runtime_host.session().is_bash_running(),
        "the held shell runs"
    );
    command
}

/// Waits for the held shell's own command to finish, which its release or its cancellation
/// settles.
pub(super) async fn until_settled(command: LocalFuture) {
    command.await.expect("the held shell settles");
}

/// Lets the held shell's script end so its command can settle.
fn release_held(directory: &Path) {
    std::fs::write(directory.join(HELD_RELEASE), "").unwrap();
}

/// Presses the interrupt key, recording whether the response was already stopped at each
/// editor change it made.
fn press_interrupt(world: &World, signal: Option<&Cancellation>) -> Vec<bool> {
    let stopped = Rc::new(RefCell::new(Vec::new()));
    if let Some(signal) = signal {
        let (seen, signal) = (Rc::clone(&stopped), signal.clone());
        world.mode.editor.set_on_change(Some(Rc::new(move |_| {
            seen.borrow_mut().push(signal.is_aborted());
        })));
    }
    world.mode.editor.handle_input("\x1b");
    world.mode.editor.set_on_change(None);
    stopped.borrow().clone()
}

/// Checks what the interrupt key left after running `selection`.
fn assert_interrupt(
    world: &World,
    selection: Interrupt,
    signal: Option<&Cancellation>,
    stopped: &[bool],
) {
    let session = world.mode.runtime_host.session();
    // The interrupt key clears shell mode when nothing outranks it; a cancelled shell also
    // clears it once its submission settles, as the reference's own post-await assignment
    // does, so only a streaming response leaves the shell draft in shell mode.
    let shell_mode = selection.mode && selection.streaming;
    assert_eq!(
        world.mode.is_bash_mode.get(),
        shell_mode,
        "shell mode after {selection:?}"
    );
    assert_eq!(
        world.mode.editor.get_text().as_str(),
        selection.text(),
        "editor text after {selection:?}"
    );
    assert_eq!(
        session.is_bash_running(),
        selection.running && selection.streaming,
        "shell running after {selection:?}"
    );
    if selection.streaming {
        assert!(
            signal.unwrap().is_aborted(),
            "the response is stopped: {selection:?}"
        );
        assert!(
            stopped.iter().all(|stopped| !stopped),
            "the queued text is restored before the turn stops: {stopped:?}"
        );
    }
}

/// Checks the editor border: clearing shell mode restores the thinking border, and a shell
/// branch that leaves the shell draft keeps the shell border.
fn assert_border(world: &World, selection: Interrupt) {
    let expected = match (selection.streaming, selection.running, selection.mode) {
        (true, _, true) => Some("@bashMode@━\x1b[39m"),
        (false, false, true) => Some("@thinkingHigh@━\x1b[39m"),
        _ => None,
    };
    if let Some(expected) = expected {
        assert_eq!(
            rendered_border(world),
            border_row(world, expected),
            "{selection:?}"
        );
    }
}

/// Runs the interrupt selection in `world`, returning the mode's work failures.
async fn interrupt_in(
    world: &World,
    directory: &Path,
    selection: Interrupt,
    signal: Option<Cancellation>,
) -> Vec<Box<dyn std::error::Error>> {
    let ((), errors) = driving(world, async {
        // A queued text is seeded once the response streams, where the restore can take it
        // back, and before any shell command so both branches see the same queues.
        seed(world, selection.queued());
        let mut command = selection.running.then(|| start_held(world));
        let stopped = press_interrupt(world, signal.as_ref());
        // A streaming response outranks the shell, so its release settles the command later;
        // every other interrupt cancels the command itself, which settles before the checks.
        if selection.running && !selection.streaming {
            until_settled(command.take().expect("the running shell holds its command")).await;
        }
        assert_interrupt(world, selection, signal.as_ref(), &stopped);
        assert_border(world, selection);
        if let Some(command) = command {
            release_held(directory);
            until_settled(command).await;
        }
    })
    .await;
    errors
}

/// Runs one interrupt selection over a fresh mode and checks its outcome.
async fn interrupt_case(directory: &Path, selection: Interrupt) {
    let world = held_shell(directory).await;
    select_level(&world, ThinkingLevel::High);
    world.mode.editor.set_text(selection.draft());
    let errors = if selection.streaming {
        holding(&world, |signal| {
            interrupt_in(&world, directory, selection, Some(signal))
        })
        .await
    } else {
        interrupt_in(&world, directory, selection, None).await
    };
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn shell_escape_keeps_streaming_priority_and_live_mode_border() {
    isolated(
        "shell_escape_keeps_streaming_priority_and_live_mode_border",
        async |directory| {
            for (streaming, running, mode) in [
                (false, false, false),
                (false, false, true),
                (false, true, false),
                (false, true, true),
                (true, false, false),
                (true, false, true),
                (true, true, false),
                (true, true, true),
            ] {
                interrupt_case(
                    directory,
                    Interrupt {
                        streaming,
                        running,
                        mode,
                    },
                )
                .await;
            }
        },
    );
}

/// The queued texts a refresh witness crosses with the pending shell rows.
#[derive(Clone, Copy, Debug)]
enum QueueClass {
    /// Nothing queued.
    Empty,
    /// One steering message.
    Steering,
    /// One follow-up message.
    FollowUp,
    /// Both queues.
    Both,
}

impl QueueClass {
    /// The queued texts this class starts with.
    fn queues(self) -> Queues {
        let (steering, follow_up) = match self {
            Self::Empty => (vec![], vec![]),
            Self::Steering => (vec!["s".into()], vec![]),
            Self::FollowUp => (vec![], vec!["f".into()]),
            Self::Both => (vec!["s".into()], vec!["f".into()]),
        };
        Queues {
            steering,
            follow_up,
            local: Vec::new(),
        }
    }

    /// The text rows the display owner paints, before the hint.
    fn labels(self) -> Vec<&'static str> {
        match self {
            Self::Empty => vec![],
            Self::Steering => vec!["Steering: s"],
            Self::FollowUp => vec!["Follow-up: f"],
            Self::Both => vec!["Steering: s", "Follow-up: f"],
        }
    }
}

/// The handles the pending area holds as shell rows, in its own order.
fn pending_shell_rows(world: &World) -> Vec<Rc<BashExecutionComponent>> {
    let retained = world.mode.pending_shells.borrow().clone();
    let children = world.mode.pending_messages_container.children();
    children
        .borrow()
        .iter()
        .filter_map(|child| {
            let address = component_address(child);
            retained
                .iter()
                .find(|shell| component_address(*shell) == address)
                .map(Rc::clone)
        })
        .collect()
}

/// Checks that every shell saved before the refresh, `original`, is still the retained and
/// shown row at its position, that it paints its output-only marker once, and that
/// `class`'s text rows follow them.
fn assert_pending_rows(world: &World, class: QueueClass, original: &[Rc<BashExecutionComponent>]) {
    let retained = world.mode.pending_shells.borrow();
    assert_eq!(retained.len(), original.len(), "retained shell rows");
    for (index, (retained, saved)) in retained.iter().zip(original.iter()).enumerate() {
        assert!(
            Rc::ptr_eq(retained, saved),
            "row {index} stays retained through the refresh"
        );
    }
    let shown = pending_shell_rows(world);
    assert_eq!(shown.len(), original.len(), "shown shell rows");
    for (index, (shown, saved)) in shown.iter().zip(original.iter()).enumerate() {
        assert!(
            Rc::ptr_eq(shown, saved),
            "row {index} keeps its identity through the refresh"
        );
    }
    assert_painted_output(world, original.len());
    let labels = class.labels();
    let text_rows = pending_text_rows(world, original.len());
    let expected = if labels.is_empty() {
        0
    } else {
        labels.len() + 1
    };
    assert_eq!(text_rows.len(), expected, "text rows for {class:?}");
    for (row, label) in text_rows.iter().zip(labels.iter()) {
        assert!(row.contains(label), "{row:?} carries {label:?}");
    }
    if !labels.is_empty() {
        let hint = text_rows.last().unwrap();
        assert!(hint.contains("to edit all queued messages"), "{hint:?}");
    }
    let theme = world.themes.theme().get().unwrap();
    let dim = theme.get_fg_ansi(&ThemeColor::Dim).unwrap();
    for row in &text_rows {
        assert!(row.contains(dim), "{row:?} is painted dim");
    }
}

/// Checks that each of `shells` pending rows paints its output-only marker once: the marker
/// is absent from the command text, so the command header cannot supply it.
fn assert_painted_output(world: &World, shells: usize) {
    let rows = world.mode.pending_messages_container.render(80);
    let painted = rows.join("\n");
    for index in 0..shells {
        assert_eq!(
            painted.matches(&format!("printed mark{index}")).count(),
            1,
            "row {index} paints its output once: {painted}"
        );
    }
}

/// The rendered rows of the text children that follow `shells` shell rows.
fn pending_text_rows(world: &World, shells: usize) -> Vec<String> {
    let children = world.mode.pending_messages_container.children();
    let children = children.borrow();
    let text: Vec<String> = children
        .iter()
        .skip(shells)
        .flat_map(|child| child.render(80))
        .filter(|row| !row.is_empty())
        .collect();
    text
}

/// Refreshes the pending rows once and checks the shells saved before the first refresh.
fn refresh(world: &World, class: QueueClass, original: &[Rc<BashExecutionComponent>]) {
    pending(&world.mode, 80);
    assert_pending_rows(world, class, original);
}

/// The default keys of the dequeue action, Alt+Up.
const DEQUEUE_KEY: &str = "\x1b[1;3A";

/// One refresh witness: `shells` pending rows under `class`'s queues, refreshed twice, then
/// dequeued through the registered key, refreshed again and once more after the
/// ordinary-input flush.
async fn refresh_case(directory: &Path, shells: usize, class: QueueClass) {
    let world = echoing_shell(directory).await;
    let errors = async {
        holding(&world, |_| async {
            let ((), errors) = driving(&world, async {
                start_shells(&world, shells).await;
                let original: Vec<Rc<BashExecutionComponent>> =
                    world.mode.pending_shells.borrow().clone();
                assert_painted_output(&world, shells);
                seed(&world, class.queues());
                refresh(&world, class, &original);
                refresh(&world, class, &original);
                seed(
                    &world,
                    Queues {
                        steering: vec!["t".into()],
                        ..Queues::default()
                    },
                );
                pending(&world.mode, 80);
                let retained = world.mode.pending_shells.borrow().len();
                assert_eq!(retained, shells, "a later enqueue keeps the shells");
                // The dequeue action empties the queues and refreshes the rows itself.
                world.terminal.send_input(DEQUEUE_KEY);
                assert_pending_rows(&world, QueueClass::Empty, &original);
                refresh(&world, QueueClass::Empty, &original);
                world.mode.flush_pending_bash_components();
                refresh(&world, QueueClass::Empty, &[]);
                assert!(
                    world.mode.pending_shells.borrow().is_empty(),
                    "a flushed shell is not pending"
                );
            })
            .await;
            errors
        })
        .await
    }
    .await;
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn pending_shell_rows_survive_queue_refresh() {
    isolated(
        "pending_shell_rows_survive_queue_refresh",
        async |directory| {
            for shells in [0, 1, 3] {
                for class in [
                    QueueClass::Empty,
                    QueueClass::Steering,
                    QueueClass::FollowUp,
                    QueueClass::Both,
                ] {
                    refresh_case(directory, shells, class).await;
                }
            }
        },
    );
}

/// Submits `text` through the installed submission and returns what the waiting input
/// received, with the pending shell rows reported at the moment it arrived.
fn deliver(
    world: &World,
    waiter: &mut Pin<Box<dyn Future<Output = String>>>,
) -> Option<(String, usize)> {
    world.mode.editor.on_submit().unwrap()("ordinary");
    poll(waiter).map(|text| (text, world.mode.pending_shells.borrow().len()))
}

/// How the shell commands of a movement case are worded.
#[derive(Clone, Copy, Debug)]
enum Commands {
    /// Each command has its own text.
    Distinct,
    /// Every command has the same text, so only a row's handle tells it from another.
    Shared,
}

/// Produces `count` pending shell rows while a response streams, and returns the world with
/// the retained rows and the pending area's children.
async fn produce_pending(
    directory: &Path,
    count: usize,
    commands: Commands,
) -> (
    World,
    Vec<Rc<BashExecutionComponent>>,
    Vec<Rc<dyn Component>>,
) {
    let world = echoing_shell(directory).await;
    let retained = async {
        holding(&world, |_| async {
            let (rows, errors) = driving(&world, async {
                match commands {
                    Commands::Distinct => start_shells(&world, count).await,
                    Commands::Shared => start_commands(&world, count, |_| "!same".to_owned()).await,
                }
                world.mode.pending_shells.borrow().clone()
            })
            .await;
            assert!(errors.is_empty(), "{errors:?}");
            rows
        })
        .await
    }
    .await;
    seed(
        &world,
        Queues {
            steering: vec!["s".into()],
            follow_up: vec!["f".into()],
            local: Vec::new(),
        },
    );
    pending(&world.mode, 80);
    let neighbours = world
        .mode
        .pending_messages_container
        .children()
        .borrow()
        .clone();
    assert_eq!(retained.len(), count, "the response produced the rows");
    (world, retained, neighbours)
}

/// Ends the response, delivers an ordinary submission and checks where every row went.
async fn movement_case(directory: &Path, count: usize, commands: Commands) {
    let (world, retained, neighbours) = produce_pending(directory, count, commands).await;
    let mut waiter = super::typed::waiting(&world.mode);
    assert_eq!(
        world.mode.pending_shells.borrow().len(),
        count,
        "ending the response keeps the rows pending"
    );
    assert!(
        transcript_positions(&world, &retained).is_empty(),
        "ending the response moves nothing"
    );
    let (text, pending) = deliver(&world, &mut waiter).expect("the ordinary input arrives");
    assert_eq!(text, "ordinary");
    assert_eq!(pending, 0, "the rows moved before the input arrived");
    let positions = transcript_positions(&world, &retained);
    assert_eq!(positions.len(), count, "every row moved once");
    assert!(
        positions.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "the rows keep their insertion order: {positions:?}"
    );
    let kept = world.mode.pending_messages_container.children();
    let kept = kept.borrow().clone();
    assert_eq!(
        kept.len(),
        neighbours.len() - count,
        "the text rows stay behind"
    );
    for (kept, neighbour) in kept.iter().zip(neighbours.iter().skip(count)) {
        assert!(
            Rc::ptr_eq(kept, neighbour),
            "the text rows keep their identity and order"
        );
    }
    let transcript = world.mode.chat_container.children().borrow().len();
    world.mode.flush_pending_bash_components();
    assert_eq!(
        world.mode.chat_container.children().borrow().len(),
        transcript,
        "a second flush moves nothing"
    );
    assert!(recalled(&world.mode.editor).contains(&"ordinary".to_owned()));
}

#[test]
fn pending_shell_rows_move_once_in_insertion_order_before_input() {
    isolated(
        "pending_shell_rows_move_once_in_insertion_order_before_input",
        async |directory| {
            for commands in [Commands::Distinct, Commands::Shared] {
                for count in [0, 1, 3] {
                    movement_case(directory, count, commands).await;
                }
            }
        },
    );
}

/// What a submission did with its text.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Route {
    /// A shell command ran, its output included or excluded from the context.
    Shell(bool),
    /// A registered extension command ran.
    Command,
    /// The text was kept until the compaction ends.
    Compacted,
    /// The text went to the session as a steering message.
    Steered,
    /// The text reached the waiting input.
    Delivered,
}

/// The session states a submission starts in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Producer {
    /// No response streams and the session does not compact.
    Idle,
    /// A held response streams.
    Streaming,
    /// The session compacts.
    Compacting,
}

impl Producer {
    /// The route this state gives `text`, given the shell exclusion it selects: a
    /// registered command runs through a prompt in both busy states.
    fn route(self, text: &str, exclusion: Option<bool>) -> Route {
        match (self, exclusion) {
            (_, Some(exclude)) => Route::Shell(exclude),
            (Self::Compacting | Self::Streaming, None) if text.starts_with('/') => Route::Command,
            (Self::Compacting, None) => Route::Compacted,
            (Self::Streaming, None) => Route::Steered,
            (Self::Idle, None) => Route::Delivered,
        }
    }
}

/// The texts an ordinary submission offers, with the shell exclusion each selects.
const TEXTS: [(&str, Option<bool>); 6] = [
    ("!echo accepted", Some(false)),
    ("!!echo excluded", Some(true)),
    ("!", None),
    ("!!", None),
    ("ordinary", None),
    ("/cmd", None),
];

/// The waiting input a case observes.
type Waiting = Pin<Box<dyn Future<Output = String>>>;

/// Awaits the work `route`'s submission admitted: each piece completes only once its part
/// reached the session, and text that changes no queue admits nothing.
async fn settle(world: &World, route: Route) {
    if matches!(route, Route::Compacted | Route::Delivered) {
        return;
    }
    loop {
        let admitted = world.runtime.take_futures();
        if admitted.is_empty() {
            return;
        }
        for piece in admitted {
            if let Err(error) = piece.await {
                panic!("the submission fails: {error}");
            }
        }
    }
}

/// What one admission case expects to find.
struct Expected {
    /// The route the submission takes.
    route: Route,
    /// The submitted text.
    text: &'static str,
    /// The transcript and pending child counts before the submission.
    baseline: (usize, usize),
}

/// What a submission left behind, read once so each route checks its own part.
struct Observation {
    /// The route entries the extension recorded.
    routes: Vec<serde_json::Value>,
    /// Whether the waiting input received the text.
    arrived: bool,
    /// Messages kept until the compaction ends.
    kept: usize,
    /// Session steering messages.
    steering: Vec<String>,
    /// New transcript children.
    chat: usize,
    /// New pending-area children.
    pending: usize,
    /// Retained pending shell rows.
    shells: usize,
    /// The editor history.
    history: Vec<String>,
}

impl Observation {
    /// Reads what `world` holds after a submission.
    fn of(world: &World, heard: &Heard, waiter: &mut Waiting, baseline: (usize, usize)) -> Self {
        let routes = heard
            .borrow()
            .iter()
            .filter(|entry| matches!(entry[0].as_str(), Some("execute" | "input" | "command")))
            .cloned()
            .collect();
        let session = world.mode.runtime_host.session();
        Self {
            routes,
            arrived: poll(waiter).is_some(),
            kept: world.mode.compaction_queued_messages.borrow().len(),
            steering: session.get_steering_messages(),
            chat: world.mode.chat_container.children().borrow().len() - baseline.0,
            pending: world
                .mode
                .pending_messages_container
                .children()
                .borrow()
                .len()
                - baseline.1,
            shells: world.mode.pending_shells.borrow().len(),
            history: recalled(&world.mode.editor),
        }
    }

    /// Checks that `expected` is the only route entry the extension recorded.
    fn only(&self, expected: serde_json::Value, route: &str) {
        assert_eq!(self.routes, vec![expected], "{route}");
    }

    /// Checks the single history entry a submission records.
    fn one_history(&self, text: &str, route: &str) {
        assert_eq!(
            self.history,
            vec![text.to_owned()],
            "one history entry for {text:?} through {route}"
        );
    }

    /// Checks a shell command's route, placement and history entry.
    fn shell(&self, text: &str, exclusion: bool) {
        let command = text.trim_start_matches('!');
        self.only(json!(["execute", command, exclusion]), "shell");
        assert_eq!(self.kept, 0, "a shell command keeps no message");
        assert!(self.steering.is_empty(), "a shell command steers nothing");
        assert_eq!(self.chat + self.pending, 1, "one retained shell row");
        assert_eq!(
            self.shells, self.pending,
            "the pending rows are the retained ones"
        );
        self.one_history(text, "shell");
    }

    /// Checks a steering message's route, queue and history entry.
    fn steered(&self, text: &str) {
        self.only(json!(["input", text]), "steer");
        assert_eq!(
            self.steering,
            vec![text.to_owned()],
            "the message {text:?} is queued"
        );
        assert_eq!(self.kept, 0, "a steering message is kept in the session");
        assert_eq!(self.shells, 0, "no shell row after {text:?}");
        self.one_history(text, "steer");
    }

    /// Checks a message kept until the compaction ends.
    fn compacted(&self, text: &str) {
        assert!(self.routes.is_empty(), "{:?}", self.routes);
        assert_eq!(self.kept, 1, "one message waits for the compaction");
        assert!(self.steering.is_empty());
        assert_eq!(self.shells, 0, "no shell row after {text:?}");
        self.one_history(text, "compaction");
    }

    /// Checks a registered command's route and history entry.
    fn commanded(&self, text: &str) {
        self.only(json!(["command", ""]), "command");
        assert_eq!(self.kept, 0);
        assert!(self.steering.is_empty());
        assert_eq!(self.shells, 0, "no shell row after {text:?}");
        self.one_history(text, "command");
    }

    /// Checks a text that reached the waiting input and changed no queue.
    fn plain(&self, text: &str) {
        assert!(self.routes.is_empty(), "{:?}", self.routes);
        assert_eq!(self.kept, 0);
        assert!(self.steering.is_empty());
        assert_eq!(self.shells, 0, "no shell row after {text:?}");
        self.one_history(text, "input");
    }
}

/// Checks what `expected.route` left.
fn assert_route(world: &World, heard: &Heard, waiter: &mut Waiting, expected: &Expected) {
    let observed = Observation::of(world, heard, waiter, expected.baseline);
    let route = expected.route;
    assert_eq!(observed.arrived, route == Route::Delivered, "{route:?}");
    match route {
        Route::Shell(exclusion) => observed.shell(expected.text, exclusion),
        Route::Steered => observed.steered(expected.text),
        Route::Compacted => observed.compacted(expected.text),
        Route::Command => observed.commanded(expected.text),
        Route::Delivered => observed.plain(expected.text),
    }
}

/// The retained transcript and pending child counts at the observed admission state.
fn child_counts(world: &World) -> (usize, usize) {
    (
        world.mode.chat_container.children().borrow().len(),
        world
            .mode
            .pending_messages_container
            .children()
            .borrow()
            .len(),
    )
}

/// Checks an ordinary input's admission before the compaction's end captures it.
async fn observe_compaction_admission(
    world: World,
    heard: Heard,
    waiter: Waiting,
    text: &'static str,
    baseline: (usize, usize),
) {
    let world = Rc::new(world);
    let observed_world = Rc::clone(&world);
    let waiter = RefCell::new(waiter);
    during_compaction(&world, move |mode| {
        mode.editor.on_submit().unwrap()(text);
        let expected = Expected {
            route: Route::Compacted,
            text,
            baseline,
        };
        assert_route(&observed_world, &heard, &mut waiter.borrow_mut(), &expected);
    })
    .await;
}

/// Submits input while driving its admitted work to the selected route.
async fn submit_route(
    world: &World,
    heard: &Heard,
    text: &'static str,
    route: Route,
) -> Vec<Box<dyn std::error::Error>> {
    driving(world, async {
        heard.borrow_mut().clear();
        world.mode.editor.on_submit().unwrap()(text);
        settle(world, route).await;
    })
    .await
    .1
}

/// Submits `text` in `producer`'s state and checks the route it took.
async fn admission_case(
    directory: &Path,
    producer: Producer,
    (text, exclusion): (&'static str, Option<bool>),
) {
    let (world, heard) = controlled(directory, false).await;
    let mut waiter: Waiting = super::typed::waiting(&world.mode);
    let baseline = child_counts(&world);
    let route = producer.route(text, exclusion);
    if producer == Producer::Compacting && route == Route::Compacted {
        return observe_compaction_admission(world, heard, waiter, text, baseline).await;
    }
    let press = move |mode: &Rc<InteractiveMode>| {
        mode.editor.on_submit().unwrap()(text);
    };
    let mut expected = Expected {
        route,
        text,
        baseline,
    };
    // A streaming response consumes what it steered when it ends, so its route is checked
    // while the response still holds the session.
    let errors = match producer {
        Producer::Streaming => {
            holding(&world, |_| async {
                let expected = Expected {
                    baseline: child_counts(&world),
                    ..expected
                };
                let errors = submit_route(&world, &heard, text, route).await;
                assert_route(&world, &heard, &mut waiter, &expected);
                errors
            })
            .await
        }
        Producer::Compacting => {
            let notice = with_compaction_notice(observe(&world, &Heard::default(), 0), &world);
            heard.borrow_mut().clear();
            during_compaction(&world, press).await;
            let chat = world.mode.chat_container.render(80);
            assert!(
                chat.windows(notice.chat.len())
                    .any(|rows| rows == notice.chat)
            );
            expected.baseline.0 += notice.chat.len();
            (driving(&world, settle(&world, route))).await.1
        }
        Producer::Idle => submit_route(&world, &heard, text, route).await,
    };
    assert!(errors.is_empty(), "{errors:?}");
    if producer != Producer::Streaming {
        assert_route(&world, &heard, &mut waiter, &expected);
    }
}

#[test]
fn shell_dispatch_precedes_streaming_and_compaction_admission() {
    isolated(
        "shell_dispatch_precedes_streaming_and_compaction_admission",
        async |directory| {
            for producer in [Producer::Idle, Producer::Streaming, Producer::Compacting] {
                for text in TEXTS {
                    admission_case(directory, producer, text).await;
                }
            }
        },
    );
}

#[test]
fn extension_abort_restores_queued_input_exactly_as_typed() {
    const TEST: &str = "extension_abort_restores_queued_input_exactly_as_typed";
    isolated(TEST, async |directory| {
        streaming(&started(directory).await, async |world, held| {
            let session = world.mode.runtime_host.session();
            session.steer("  queued  steer ", &[]).unwrap();
            world.mode.editor.set_text("");
            session.extension_runner().create_context().abort().unwrap();
            assert_eq!(world.mode.editor.get_text(), "  queued  steer ");
            assert_eq!(session.pending_message_count(), 0);
            assert_eq!(held.aborts(), 1);
        })
        .await;
    });
}

#[test]
fn extension_abort_during_a_confirmation_restores_queued_input() {
    const TEST: &str = "extension_abort_during_a_confirmation_restores_queued_input";
    isolated(TEST, async |directory| {
        streaming(&started(directory).await, async |world, held| {
            let session = world.mode.runtime_host.session();
            session.steer("queued steer", &[]).unwrap();
            world.mode.editor.set_text("");
            let ui = maestro_app::ExtensionUIContext::new(world.mode.clone());
            let mut confirmation =
                std::pin::pin!(ui.confirm("Run the tool?".into(), "bash".into(), None));
            assert!(
                poll(&mut confirmation).is_none(),
                "the confirmation stays open"
            );
            session.extension_runner().create_context().abort().unwrap();
            assert_eq!(world.mode.editor.get_text(), "queued steer");
            assert_eq!(held.aborts(), 1);
        })
        .await;
    });
}

//! Typed input through the editor's installed submission and the single waiting input.
use super::{World, cases as recorded_cases, debug::only_log, isolated, started};
use crate::{CustomEditor, InteractiveMode};
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, KeybindingKeys};
use serde::Deserialize;
use std::{
    cell::RefCell,
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

/// Every recorded input case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_input.json");

/// Polls `future` once with a waker that observes nothing.
pub(super) fn poll<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

/// An input future of `mode` that has been polled once and is waiting.
pub(super) fn waiting(mode: &InteractiveMode) -> Pin<Box<dyn Future<Output = String>>> {
    let mut waiter: Pin<Box<dyn Future<Output = String>>> = Box::pin(mode.get_user_input());
    assert_eq!(poll(&mut waiter), None);
    waiter
}

/// The editor's history, newest first, recalled with Up from an empty editor, which is
/// left empty again.
pub(super) fn recalled(editor: &CustomEditor) -> Vec<String> {
    editor.set_text("");
    let mut history: Vec<String> = Vec::new();
    loop {
        editor.handle_input("\x1b[A");
        let text = editor.get_text();
        if text.is_empty() || history.last() == Some(&text) {
            break;
        }
        history.push(text);
    }
    editor.set_text("");
    history
}

/// Submits `text` through the callback the mode installed on its editor.
pub(super) fn submit(mode: &InteractiveMode, text: &str) {
    let callback = mode.editor.on_submit().unwrap();
    callback(text);
}

/// One submitted value, with or without a waiting input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    /// The submitted value.
    value: String,
    /// Whether an input waits when it is submitted.
    waiting: bool,
}

/// What one submission delivered and recorded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    /// The values the waiting input received.
    delivered: Vec<String>,
    /// The history, newest first.
    history: Vec<String>,
    /// Whether an input still waits.
    waiting: bool,
}

#[test]
fn typed_submission_trims_ecmascript_whitespace_and_records_history() {
    const TEST: &str = "typed_submission_trims_ecmascript_whitespace_and_records_history";
    isolated(TEST, async |directory| {
        for (input, expected) in recorded_cases::<Selection, Selected>(FIXTURES, TEST) {
            let world = started(directory).await;
            let mut waiter = input.waiting.then(|| waiting(&world.mode));
            submit(&world.mode, &input.value);
            let delivered: Vec<String> = waiter.as_mut().and_then(poll).into_iter().collect();
            assert_eq!(delivered, expected.delivered, "{:?}", input.value);
            assert_eq!(
                world.mode.on_input_callback.borrow().is_some(),
                expected.waiting
            );
            assert_eq!(recalled(&world.mode.editor), expected.history);
        }
    });
}

/// A recorded sequence of submissions and waits.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sequence {
    /// The sequence's name.
    op: String,
}

/// What a sequence of submissions and waits observed.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Waited {
    /// Early, empty, accepted and later submissions around two waits.
    OneShot {
        /// What the first waiter received.
        delivered: Vec<String>,
        /// What the second waiter received.
        second: String,
        /// The history, newest first.
        history: Vec<String>,
        /// Whether the slot is empty at the end.
        cleared: bool,
    },
    /// A second waiter replacing the first.
    #[serde(rename_all = "camelCase")]
    Replace {
        /// Whether the displaced waiter completed.
        old_done: bool,
        /// What the replacing waiter received.
        new_value: String,
        /// The history, newest first.
        history: Vec<String>,
        /// Whether the slot is empty at the end.
        cleared: bool,
    },
}

/// Early, empty, accepted, later and fresh-waiter submissions.
fn one_shot(world: &World) -> (Vec<String>, String) {
    submit(&world.mode, " early ");
    let mut first = waiting(&world.mode);
    submit(&world.mode, "   ");
    assert!(world.mode.on_input_callback.borrow().is_some());
    assert_eq!(poll(&mut first), None);
    submit(&world.mode, " one ");
    let delivered = vec![poll(&mut first).unwrap()];
    assert!(world.mode.on_input_callback.borrow().is_none());
    submit(&world.mode, " after ");
    let mut second = waiting(&world.mode);
    submit(&world.mode, " two ");
    (delivered, poll(&mut second).unwrap())
}

/// A second waiter replacing a first, which stays pending through later submissions.
fn replace(world: &World) -> (bool, String, Vec<String>) {
    let mut old = waiting(&world.mode);
    let mut latest = waiting(&world.mode);
    assert_eq!(poll(&mut old), None);
    submit(&world.mode, " replacement ");
    let old_done = poll(&mut old).is_some();
    let new_value = poll(&mut latest).unwrap();
    let history = recalled(&world.mode.editor);
    submit(&world.mode, " later ");
    assert_eq!(poll(&mut old), None);
    (old_done, new_value, history)
}

#[test]
fn typed_waiter_is_one_shot_without_buffering_early_submissions() {
    const TEST: &str = "typed_waiter_is_one_shot_without_buffering_early_submissions";
    isolated(TEST, async |directory| {
        for (input, expected) in recorded_cases::<Sequence, Waited>(FIXTURES, TEST) {
            let world = started(directory).await;
            let cleared = |world: &World| world.mode.on_input_callback.borrow().is_none();
            match (input.op.as_str(), expected) {
                (
                    "early-empty-one-shot-next",
                    Waited::OneShot {
                        delivered,
                        second,
                        history,
                        cleared: empty,
                    },
                ) => {
                    assert_eq!(one_shot(&world), (delivered, second));
                    assert_eq!(recalled(&world.mode.editor), history);
                    assert_eq!(cleared(&world), empty);
                }
                (
                    "replace",
                    Waited::Replace {
                        old_done,
                        new_value,
                        history,
                        cleared: empty,
                    },
                ) => {
                    assert_eq!(replace(&world), (old_done, new_value, history));
                    assert_eq!(cleared(&world), empty);
                }
                (op, _) => panic!("unknown sequence {op}"),
            }
        }
        let world = started(directory).await;
        let mut unpolled = Box::pin(world.mode.get_user_input());
        submit(&world.mode, " unclaimed ");
        assert!(world.mode.on_input_callback.borrow().is_none());
        assert_eq!(poll(&mut unpolled), None);
        submit(&world.mode, " claimed ");
        assert_eq!(poll(&mut unpolled).as_deref(), Some("claimed"));
        drop(waiting(&world.mode));
        submit(&world.mode, " abandoned ");
        assert!(world.mode.on_input_callback.borrow().is_none());
        assert_eq!(
            recalled(&world.mode.editor),
            ["abandoned", "claimed", "unclaimed"]
        );
    });
}

#[test]
fn typed_callback_does_not_retain_the_mode() {
    isolated(
        "typed_callback_does_not_retain_the_mode",
        async |directory| {
            let World {
                mode,
                terminal,
                runtime,
                themes,
                host,
                loader,
            } = started(directory).await;
            let editor = Rc::clone(&mode.editor);
            let callback = editor.on_submit().unwrap();
            let mut unpolled = Box::pin(mode.get_user_input());
            let mut waiter = waiting(&mode);
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_none());
            callback(" after release ");
            assert!(weak.upgrade().is_none());
            assert_eq!(poll(&mut waiter), None);
            assert_eq!(poll(&mut unpolled), None);
            assert!(recalled(&editor).is_empty());
            drop((terminal, runtime, themes, host, loader));
        },
    );
}

thread_local! {
    /// The action this thread's next observed wake runs; it is taken out before it runs.
    static ON_WAKE: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::default();
}

/// A waker that runs the action installed for this thread's next wake.
struct Observer;

impl Wake for Observer {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(action) = ON_WAKE.with_borrow_mut(Option::take) {
            action();
        }
    }
}

/// What the waiting input saw when it was woken.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Observation {
    /// The text it received.
    text: String,
    /// Whether the slot was already empty.
    cleared: bool,
    /// The history, newest first.
    history: Vec<String>,
}

/// What delivery observed and what the reentrant waiter received.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reentered {
    /// The observations made at the wake.
    observations: Vec<Observation>,
    /// What the first waiter received.
    first: String,
    /// What the waiter registered at the wake received.
    second: String,
    /// The final history, newest first.
    history: Vec<String>,
}

#[test]
fn typed_delivery_precedes_history_and_accepts_reentrant_registration() {
    const TEST: &str = "typed_delivery_precedes_history_and_accepts_reentrant_registration";
    isolated(TEST, async |directory| {
        let [(input, expected)] =
            <[(Sequence, Reentered); 1]>::try_from(recorded_cases(FIXTURES, TEST))
                .ok()
                .unwrap();
        assert_eq!(input.op, "callback-reentry");
        let world = started(directory).await;
        world.mode.editor.add_to_history("older");
        let first = Rc::new(RefCell::new(Box::pin(world.mode.get_user_input())));
        let waker = Waker::from(Arc::new(Observer));
        let polled = first
            .borrow_mut()
            .as_mut()
            .poll(&mut Context::from_waker(&waker));
        assert!(polled.is_pending());
        let observations = Rc::new(RefCell::new(Vec::new()));
        let second = Rc::new(RefCell::new(None));
        let (mode, woken, observed, next) = (
            Rc::clone(&world.mode),
            Rc::clone(&first),
            Rc::clone(&observations),
            Rc::clone(&second),
        );
        ON_WAKE.set(Some(Box::new(move || {
            let text = poll(&mut *woken.borrow_mut()).unwrap();
            let cleared = mode.on_input_callback.borrow().is_none();
            let history = recalled(&mode.editor);
            observed.borrow_mut().push(Observation {
                text,
                cleared,
                history,
            });
            *next.borrow_mut() = Some(waiting(&mode));
            mode.editor.set_text("alternate editor text");
        })));
        submit(&world.mode, " selected ");
        assert!(ON_WAKE.with_borrow(Option::is_none));
        assert_eq!(*observations.borrow(), expected.observations);
        assert_eq!(observations.borrow()[0].text, expected.first);
        assert_eq!(world.mode.editor.get_text(), "alternate editor text");
        assert_eq!(recalled(&world.mode.editor)[0], expected.first);
        submit(&world.mode, " next ");
        let mut waiter = second.borrow_mut().take().unwrap();
        assert_eq!(poll(&mut waiter), Some(expected.second));
        assert_eq!(recalled(&world.mode.editor), expected.history);
    });
}

/// The submit binding a case configures, with the keys it sends.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Binding {
    /// One binding loaded at construction and the keys sent under it.
    Keys {
        /// The configured submit key.
        binding: String,
        /// The keys sent, in order.
        keys: Vec<String>,
    },
    /// The binding changed after initialization, once in each direction.
    Live {
        /// The case's name.
        binding: String,
    },
}

/// What one key sent through the terminal changed.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct KeyStep {
    /// The key sent.
    key: String,
    /// What the waiting input has received so far.
    received: Option<String>,
    /// The editor text after the key.
    text: String,
}

/// One live binding and its rejected and accepted keys.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveStep {
    /// The submit key set after initialization.
    binding: String,
    /// The previous submit key, which must not submit.
    wrong: String,
    /// The current submit key.
    right: String,
    /// The editor text, unchanged by the previous key.
    before: String,
    /// What the waiting input received.
    delivered: String,
    /// The editor text after submission.
    after: String,
}

/// What the keys sent through the terminal observed.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Routed {
    /// Each key's effect and the text Up then recalls.
    #[serde(rename_all = "camelCase")]
    Keys {
        /// Each key's effect.
        steps: Vec<KeyStep>,
        /// The editor text after Up.
        history_recall: String,
    },
    /// Each live binding's effect.
    Live {
        /// Each binding's effect.
        steps: Vec<LiveStep>,
    },
}

/// A started mode whose key binding file binds submission to `binding`.
async fn bound(directory: &Path, binding: &str) -> World {
    let config = serde_json::json!({ "tui.input.submit": binding });
    std::fs::write(directory.join("keybindings.json"), config.to_string()).unwrap();
    started(directory).await
}

/// Sends `keys` under the binding loaded at construction, then recalls with Up.
async fn constructor_binding(
    directory: &Path,
    binding: &str,
    keys: &[String],
) -> (Vec<KeyStep>, String) {
    let world = bound(directory, binding).await;
    let mut waiter = waiting(&world.mode);
    world.mode.editor.set_text(" through terminal ");
    let mut received = None;
    let steps = keys
        .iter()
        .map(|key| {
            world.terminal.send_input(key);
            if received.is_none() {
                received = poll(&mut waiter);
            }
            KeyStep {
                key: key.clone(),
                received: received.clone(),
                text: world.mode.editor.get_text(),
            }
        })
        .collect();
    world.terminal.send_input("\x1b[A");
    (steps, world.mode.editor.get_text())
}

/// Changes the live submit binding to each step's key and sends the previous key, then
/// the current one.
async fn live_binding(directory: &Path, steps: &[LiveStep]) {
    let world = bound(directory, "ctrl+x").await;
    for step in steps {
        maestro_tui::get_keybindings().set_user_bindings(vec![(
            "tui.input.submit".to_owned(),
            Some(KeybindingKeys::Single(step.binding.clone())),
        )]);
        let mut waiter = waiting(&world.mode);
        world.mode.editor.set_text(&step.before);
        world.terminal.send_input(&step.wrong);
        assert_eq!(poll(&mut waiter), None);
        assert_eq!(world.mode.editor.get_text(), step.before);
        world.terminal.send_input(&step.right);
        assert_eq!(poll(&mut waiter).as_ref(), Some(&step.delivered));
        assert_eq!(world.mode.editor.get_text(), step.after);
    }
}

#[test]
fn typed_input_obeys_the_live_submit_binding() {
    const TEST: &str = "typed_input_obeys_the_live_submit_binding";
    isolated(TEST, async |directory| {
        for (input, expected) in recorded_cases::<Binding, Routed>(FIXTURES, TEST) {
            match (input, expected) {
                (
                    Binding::Keys { binding, keys },
                    Routed::Keys {
                        steps,
                        history_recall,
                    },
                ) => {
                    assert_eq!(
                        constructor_binding(directory, &binding, &keys).await,
                        (steps, history_recall)
                    );
                }
                (Binding::Live { binding }, Routed::Live { steps }) => {
                    assert_eq!(binding, "live-change-both-directions");
                    live_binding(directory, &steps).await;
                }
                _ => panic!("a binding case and its observation disagree"),
            }
        }
    });
}

/// Types `text` into the editor through the terminal and submits it with Enter.
fn enter(world: &World, text: &str) {
    world.terminal.send_input(text);
    world.terminal.send_input("\r");
}

/// Types `text` with Enter and a waiting input, then checks that a command consumed it: the
/// input keeps waiting, the editor is empty and the history records nothing.
fn consumed(world: &World, text: &str) {
    let mut waiter = waiting(&world.mode);
    enter(world, text);
    assert_eq!(poll(&mut waiter), None);
    assert_eq!(world.mode.editor.get_text(), "");
    assert!(recalled(&world.mode.editor).is_empty());
}

/// Types `text` with Enter and a waiting input, then checks that it arrives as ordinary input.
fn ordinary(world: &World, text: &str) {
    let mut waiter = waiting(&world.mode);
    enter(world, text);
    assert_eq!(poll(&mut waiter).as_deref(), Some(text.trim()));
    assert_eq!(recalled(&world.mode.editor)[0], text.trim());
}

/// The transcript rendered wide enough to keep each notice on one row.
fn transcript(world: &World) -> String {
    world.mode.chat_container.render(200).join("\n")
}

#[test]
fn name_submission_consumes_command_before_prompt() {
    isolated(
        "name_submission_consumes_command_before_prompt",
        async |directory| {
            let world = started(directory).await;
            consumed(&world, "/name");
            assert!(transcript(&world).contains("Warning: Usage: /name <name>"));
            consumed(&world, "  /name Demo  ");
            assert!(transcript(&world).contains("Session name set: Demo"));
            let name = world.host.session().session_name().unwrap();
            assert_eq!(name.as_deref(), Some("Demo"));
            let notices = world.mode.chat_container.children().borrow().len();
            ordinary(&world, "/names");
            ordinary(&world, "rename /name it");
            assert_eq!(world.mode.chat_container.children().borrow().len(), notices);
        },
    );
}

#[test]
fn session_submission_reports_without_prompt() {
    isolated(
        "session_submission_reports_without_prompt",
        async |directory| {
            let world = started(directory).await;
            consumed(&world, "/session");
            assert!(transcript(&world).contains("Session Info"));
            let notices = world.mode.chat_container.children().borrow().len();
            ordinary(&world, "/session extra");
            assert_eq!(world.mode.chat_container.children().borrow().len(), notices);
        },
    );
}

#[test]
fn debug_submission_records_then_clears_editor() {
    isolated(
        "debug_submission_records_then_clears_editor",
        async |directory| {
            let world = started(directory).await;
            consumed(&world, "/debug");
            assert!(only_log(&world).starts_with("Debug output at "));
            assert!(transcript(&world).contains("Debug log written"));
            world
                .runtime
                .set_write_error(Some(std::io::ErrorKind::PermissionDenied));
            let notices = world.mode.chat_container.children().borrow().len();
            let mut waiter = waiting(&world.mode);
            enter(&world, "/debug");
            assert_eq!(poll(&mut waiter), None);
            assert_eq!(world.mode.editor.get_text(), "");
            assert_eq!(world.mode.chat_container.children().borrow().len(), notices);
            assert_eq!(world.runtime.drain_futures().len(), 1);
            ordinary(&world, "/debug now");
        },
    );
}

#[test]
fn hotkeys_submission_uses_effective_keys_without_prompt() {
    isolated(
        "hotkeys_submission_uses_effective_keys_without_prompt",
        async |directory| {
            let world = started(directory).await;
            for (text, key) in [("/hotkeys", "alt+x"), ("\u{feff}/hotkeys\u{feff}", "alt+y")] {
                maestro_tui::get_keybindings().set_user_bindings(vec![(
                    "app.interrupt".to_owned(),
                    Some(KeybindingKeys::Single(key.to_owned())),
                )]);
                let before = world.mode.chat_container.children().borrow().len();
                consumed(&world, text);
                let children = world.mode.chat_container.children();
                let children = children.borrow();
                assert_eq!(children.len(), before + 6);
                let panel = children[before..]
                    .iter()
                    .flat_map(|child| child.render(200))
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(panel.contains("Keyboard Shortcuts"));
                let label = super::platform_keys::on_this_platform(if key == "alt+x" {
                    "Alt+X"
                } else {
                    "Alt+Y"
                });
                let cancel = panel
                    .lines()
                    .find(|row| row.contains("Cancel autocomplete / abort streaming"))
                    .unwrap();
                assert!(cancel.contains(&label));
            }
            for text in ["/hotkey", "/HOTKEYS", "/hotkeys extra", "/hotkeys\u{85}"] {
                let before = world.mode.chat_container.children().borrow().len();
                let mut waiter = waiting(&world.mode);
                enter(&world, text);
                assert_eq!(poll(&mut waiter).as_deref(), Some(text));
                assert_eq!(world.mode.editor.get_text(), "");
                assert_eq!(recalled(&world.mode.editor)[0], text);
                assert_eq!(world.mode.chat_container.children().borrow().len(), before);
            }
        },
    );
}

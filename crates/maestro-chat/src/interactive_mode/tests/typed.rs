//! Typed input through the editor's installed submission and the single waiting input.
use super::{World, isolated, recorded, started};
use crate::{CustomEditor, InteractiveMode};
use maestro_tui::KeybindingKeys;
use maestro_tui::tui::InputHandler;
use serde::{Deserialize, de::DeserializeOwned};
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

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    recorded(FIXTURES, test)
}

/// Polls `future` once with a waker that observes nothing.
pub(super) fn poll<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

/// An input future of `mode` that has been polled once and is waiting.
fn waiting(mode: &InteractiveMode) -> Pin<Box<dyn Future<Output = String>>> {
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
pub(super) fn submit(world: &World, text: &str) {
    let callback = world.mode.editor.on_submit().unwrap();
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
    isolated(TEST, |directory| {
        for (input, expected) in cases::<Selection, Selected>(TEST) {
            let world = started(directory);
            let mut waiter = input.waiting.then(|| waiting(&world.mode));
            submit(&world, &input.value);
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
    submit(world, " early ");
    let mut first = waiting(&world.mode);
    submit(world, "   ");
    assert!(world.mode.on_input_callback.borrow().is_some());
    assert_eq!(poll(&mut first), None);
    submit(world, " one ");
    let delivered = vec![poll(&mut first).unwrap()];
    assert!(world.mode.on_input_callback.borrow().is_none());
    submit(world, " after ");
    let mut second = waiting(&world.mode);
    submit(world, " two ");
    (delivered, poll(&mut second).unwrap())
}

/// A second waiter replacing a first, which stays pending through later submissions.
fn replace(world: &World) -> (bool, String, Vec<String>) {
    let mut old = waiting(&world.mode);
    let mut latest = waiting(&world.mode);
    assert_eq!(poll(&mut old), None);
    submit(world, " replacement ");
    let old_done = poll(&mut old).is_some();
    let new_value = poll(&mut latest).unwrap();
    let history = recalled(&world.mode.editor);
    submit(world, " later ");
    assert_eq!(poll(&mut old), None);
    (old_done, new_value, history)
}

#[test]
fn typed_waiter_is_one_shot_without_buffering_early_submissions() {
    const TEST: &str = "typed_waiter_is_one_shot_without_buffering_early_submissions";
    isolated(TEST, |directory| {
        for (input, expected) in cases::<Sequence, Waited>(TEST) {
            let world = started(directory);
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
        let world = started(directory);
        let mut unpolled = Box::pin(world.mode.get_user_input());
        submit(&world, " unclaimed ");
        assert!(world.mode.on_input_callback.borrow().is_none());
        assert_eq!(poll(&mut unpolled), None);
        submit(&world, " claimed ");
        assert_eq!(poll(&mut unpolled).as_deref(), Some("claimed"));
        drop(waiting(&world.mode));
        submit(&world, " abandoned ");
        assert!(world.mode.on_input_callback.borrow().is_none());
        assert_eq!(
            recalled(&world.mode.editor),
            ["abandoned", "claimed", "unclaimed"]
        );
    });
}

#[test]
fn typed_callback_does_not_retain_the_mode() {
    isolated("typed_callback_does_not_retain_the_mode", |directory| {
        let World {
            mode,
            terminal,
            runtime,
            themes,
            host,
            loader,
        } = started(directory);
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
    });
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
    isolated(TEST, |directory| {
        let [(input, expected)] = <[(Sequence, Reentered); 1]>::try_from(cases(TEST))
            .ok()
            .unwrap();
        assert_eq!(input.op, "callback-reentry");
        let world = started(directory);
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
        submit(&world, " selected ");
        assert!(ON_WAKE.with_borrow(Option::is_none));
        assert_eq!(*observations.borrow(), expected.observations);
        assert_eq!(observations.borrow()[0].text, expected.first);
        assert_eq!(world.mode.editor.get_text(), "alternate editor text");
        assert_eq!(recalled(&world.mode.editor)[0], expected.first);
        submit(&world, " next ");
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
fn bound(directory: &Path, binding: &str) -> World {
    let config = serde_json::json!({ "tui.input.submit": binding });
    std::fs::write(directory.join("keybindings.json"), config.to_string()).unwrap();
    started(directory)
}

/// Sends `keys` under the binding loaded at construction, then recalls with Up.
fn constructor_binding(directory: &Path, binding: &str, keys: &[String]) -> (Vec<KeyStep>, String) {
    let world = bound(directory, binding);
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
fn live_binding(directory: &Path, steps: &[LiveStep]) {
    let world = bound(directory, "ctrl+x");
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
    isolated(TEST, |directory| {
        for (input, expected) in cases::<Binding, Routed>(TEST) {
            match (input, expected) {
                (
                    Binding::Keys { binding, keys },
                    Routed::Keys {
                        steps,
                        history_recall,
                    },
                ) => {
                    assert_eq!(
                        constructor_binding(directory, &binding, &keys),
                        (steps, history_recall)
                    );
                }
                (Binding::Live { binding }, Routed::Live { steps }) => {
                    assert_eq!(binding, "live-change-both-directions");
                    live_binding(directory, &steps);
                }
                _ => panic!("a binding case and its observation disagree"),
            }
        }
    });
}

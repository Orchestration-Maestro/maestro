//! Typed input through the editor's installed submission and the single waiting input.
use super::{World, isolated, started};
use crate::CustomEditor;
use maestro_tui::tui::InputHandler;
use serde::{Deserialize, de::DeserializeOwned};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// Every recorded input case.
const FIXTURES: &str = include_str!("../../../tests/fixtures/interactive_input.json");

/// One recorded case, read before its witness gives the input and observation their types.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The witness the case belongs to.
    test: String,
    /// The recorded operation.
    input: serde_json::Value,
    /// The recorded observation.
    expected: serde_json::Value,
}

/// The recorded cases of `test`, typed by that witness.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(test: &str) -> Vec<(I, E)> {
    let all: Vec<Case> = serde_json::from_str(FIXTURES).unwrap();
    let cases: Vec<_> = all
        .into_iter()
        .filter(|case| case.test == test)
        .map(|case| {
            (
                serde_json::from_value(case.input).unwrap(),
                serde_json::from_value(case.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty());
    cases
}

/// Polls `future` once with a waker that observes nothing.
fn poll<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

/// The editor's history, newest first, recalled with Up from an empty editor, which is
/// left empty again.
fn recalled(editor: &CustomEditor) -> Vec<String> {
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
fn submit(world: &World, text: &str) {
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
            let mut waiter = input.waiting.then(|| Box::pin(world.mode.get_user_input()));
            if let Some(waiter) = &mut waiter {
                assert_eq!(poll(waiter), None);
            }
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
    let mut first = Box::pin(world.mode.get_user_input());
    assert_eq!(poll(&mut first), None);
    submit(world, "   ");
    assert!(world.mode.on_input_callback.borrow().is_some());
    assert_eq!(poll(&mut first), None);
    submit(world, " one ");
    let delivered = vec![poll(&mut first).unwrap()];
    assert!(world.mode.on_input_callback.borrow().is_none());
    submit(world, " after ");
    let mut second = Box::pin(world.mode.get_user_input());
    assert_eq!(poll(&mut second), None);
    submit(world, " two ");
    (delivered, poll(&mut second).unwrap())
}

/// A second waiter replacing a first, which stays pending through later submissions.
fn replace(world: &World) -> (bool, String, Vec<String>) {
    let mut old = Box::pin(world.mode.get_user_input());
    assert_eq!(poll(&mut old), None);
    let mut latest = Box::pin(world.mode.get_user_input());
    assert_eq!(poll(&mut latest), None);
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
        let mut dropped = Box::pin(world.mode.get_user_input());
        assert_eq!(poll(&mut dropped), None);
        drop(dropped);
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
        } = started(directory);
        let editor = std::rc::Rc::clone(&mode.editor);
        let callback = editor.on_submit().unwrap();
        let mut unpolled = Box::pin(mode.get_user_input());
        let mut waiting = Box::pin(mode.get_user_input());
        assert_eq!(poll(&mut waiting), None);
        let weak = std::rc::Rc::downgrade(&mode);
        drop(mode);
        assert!(weak.upgrade().is_none());
        callback(" after release ");
        assert!(weak.upgrade().is_none());
        assert_eq!(poll(&mut waiting), None);
        assert_eq!(poll(&mut unpolled), None);
        assert!(recalled(&editor).is_empty());
        drop((terminal, runtime, themes, host));
    });
}

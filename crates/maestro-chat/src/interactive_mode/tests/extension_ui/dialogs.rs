//! Extension dialogs through the installed port: each replaces the editor until it is
//! answered, cancelled, timed out or aborted, then restores the retained editor.
use super::super::{World, isolated, painted, started};
use super::{cases, port};
use crate::components::session_selector::list::tests::session_picker::manual_runtime::ms;
use maestro_app::{
    Cancellation, ExtensionUIDialogOptions, ExtensionUIFuture, MissingSessionCwdError,
    SessionCwdIssue,
};
use maestro_tui::{Component, Focusable};
use serde::Deserialize;
use serde_json::{Value, json};
use std::rc::Rc;

/// The editor text every recorded case starts from.
const RETAINED: &str = "retained text";

/// A recorded dialog kind.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
enum Kind {
    /// The option selector.
    Selector,
    /// The single-line text input.
    Input,
    /// The Yes/No confirmation.
    Confirm,
    /// The multiline editor.
    Editor,
}

/// How a recorded dialog ends.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
enum Scenario {
    /// Answered through its keys.
    Accept,
    /// Cancelled through its key.
    Cancel,
    /// Opened with an already-aborted signal.
    Preabort,
    /// Aborted while shown.
    Abort,
    /// Left until its countdown expires.
    Timeout,
}

/// A recorded dialog operation.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
enum Input {
    /// Open a dialog and end it as the scenario says.
    Dialog {
        /// The dialog kind.
        kind: Kind,
        /// How the dialog ends.
        scenario: Scenario,
        /// The countdown in milliseconds.
        #[serde(default)]
        timeout: Option<f64>,
        /// The text typed into an input before submitting.
        #[serde(default)]
        text: Option<String>,
        /// The input placeholder.
        #[serde(default)]
        placeholder: Option<String>,
        /// The confirmation option chosen.
        #[serde(default)]
        answer: Option<String>,
        /// The multiline editor's initial text.
        #[serde(default)]
        prefill: Option<String>,
    },
    /// Ask about a missing session directory and answer.
    MissingCwd {
        /// The directory offered instead.
        #[serde(rename = "fallbackCwd")]
        fallback_cwd: String,
        /// `Yes`, `No` or `cancel`.
        answer: String,
    },
    /// Open a dialog with no positive countdown, cancel it, then abort its signal.
    DialogNoTimeout {
        /// The dialog kind.
        kind: Kind,
        /// The countdown in milliseconds, absent or not positive.
        timeout: Option<f64>,
    },
    /// Open a dialog, release its result future, then cancel it, or abort its signal.
    DialogObserverRelease {
        /// The dialog kind.
        kind: Kind,
        /// Whether the dialog carries a signal and a countdown, and the abort ends it.
        #[serde(default)]
        abort: bool,
    },
}

/// The recorded state of the editor area.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Area {
    /// The painted rows while a dialog is shown.
    #[serde(default)]
    lines: Option<Vec<String>>,
    /// Whether the editor is back in the container with focus.
    restored: bool,
    /// The retained editor's text.
    editor: String,
    /// The container's child count.
    children: usize,
    /// The live countdowns.
    active_timers: usize,
    /// Whether a frame was requested.
    render_requested: bool,
}

/// The recorded observations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Expected {
    /// The area while shown, the settled value and the area afterwards.
    Dialog {
        /// The area right after opening.
        first: Area,
        /// The settled value.
        value: Value,
        /// The area after settling.
        after: Area,
    },
    /// The prompt as shown, its value and whether the editor came back.
    MissingCwd {
        /// The area right after opening.
        first: Area,
        /// The returned directory.
        value: Option<String>,
        /// Whether the editor came back.
        restored: bool,
    },
    /// The countdowns, frames and the abort after completion.
    NoTimeout {
        /// The live countdowns while shown.
        #[serde(rename = "activeTimers")]
        active_timers: usize,
        /// Whether showing and settling requested frames.
        #[serde(rename = "renderRequested")]
        render_requested: bool,
        /// Whether the late abort requested a frame.
        #[serde(rename = "abortAfterCompletionRequestedExtraRender")]
        late_abort_frame: bool,
    },
    /// The area after cancelling a released dialog.
    ObserverRelease {
        /// Whether the editor came back.
        restored: bool,
        /// The live countdowns.
        #[serde(rename = "activeTimers")]
        active_timers: usize,
    },
}

/// The settled result of `future`, polled once, or `None` while it waits.
fn settled<T>(future: &mut ExtensionUIFuture<T>) -> Option<T> {
    super::super::typed::poll(future).map(Result::unwrap)
}

/// A started mode whose editor holds the retained text, with its writes cleared.
async fn retained(directory: &std::path::Path) -> World {
    let world = started(directory).await;
    world.mode.editor.set_text(RETAINED);
    world.runtime.settle().unwrap();
    world.terminal.clear_writes();
    world
}

/// Moves the controlled clock to `millis`, running every callback due by then.
fn advance(world: &World, millis: u64) {
    for result in world.runtime.advance_to(ms(millis)) {
        result.unwrap();
    }
}

/// The editor area now; a requested frame runs and the countdowns left are counted.
fn observe(world: &World, shown: bool) -> Area {
    let mode = &world.mode;
    let children = mode.editor_container.children().borrow().clone();
    let editor_shown = children.len() == 1
        && std::ptr::addr_eq(Rc::as_ptr(&children[0]), Rc::as_ptr(&mode.editor));
    let lines = shown.then(|| mode.editor_container.render(80));
    advance(world, world.runtime.millis() + 16);
    let render_requested = !world.terminal.writes().is_empty();
    world.terminal.clear_writes();
    Area {
        lines,
        restored: editor_shown && mode.editor.focus_flag().get(),
        editor: mode.editor.get_text(),
        children: children.len(),
        active_timers: world.runtime.pending(),
        render_requested,
    }
}

/// `area` as recorded, its role markers painted with the published theme.
fn recorded(world: &World, area: Area) -> Area {
    let theme = world.themes.theme().get().unwrap();
    Area {
        lines: area.lines.map(|lines| painted(&theme, &lines)),
        ..area
    }
}

/// The dialog `kind` opened through the port, its value as recorded.
fn open(
    world: &World,
    kind: Kind,
    input: &Input,
    opts: Option<ExtensionUIDialogOptions>,
) -> ExtensionUIFuture<Value> {
    let host = port(&world.mode);
    let Input::Dialog {
        placeholder,
        prefill,
        ..
    } = input
    else {
        unreachable!("a dialog case");
    };
    let titled = |future: ExtensionUIFuture<Option<String>>| -> ExtensionUIFuture<Value> {
        Box::pin(async move { future.await.map(|value| json!(value)) })
    };
    match kind {
        Kind::Selector => titled(host.select(
            "Choose".to_owned(),
            ["Alpha", "Beta", "Alpha"].map(str::to_owned).to_vec(),
            opts,
        )),
        Kind::Input => titled(host.input("Enter".to_owned(), placeholder.clone(), opts)),
        Kind::Confirm => {
            let future = host.confirm("Title".to_owned(), "Message".to_owned(), opts);
            Box::pin(async move { future.await.map(|value| json!(value)) })
        }
        Kind::Editor => titled(host.editor("Edit".to_owned(), prefill.clone())),
    }
}

/// Ends a shown dialog as the scenario says, starting from its opening time `start`; a timed
/// out dialog's `future` is polled one millisecond before the deadline and must still wait.
fn finish(
    world: &World,
    input: &Input,
    signal: &Cancellation,
    start: u64,
    future: &mut ExtensionUIFuture<Value>,
) {
    let Input::Dialog {
        kind,
        scenario,
        timeout,
        text,
        answer,
        ..
    } = input
    else {
        unreachable!("a dialog case");
    };
    match scenario {
        Scenario::Preabort => {}
        Scenario::Abort => {
            signal.abort();
            assert!(world.runtime.drain_futures().is_empty());
        }
        Scenario::Cancel => world.terminal.send_input("\x1b"),
        Scenario::Timeout => {
            // The countdown shows whole seconds rounded up and ticks once per second.
            let deadline = std::time::Duration::from_secs_f64(timeout.unwrap() / 1000.0);
            let ticks = deadline.as_secs() + u64::from(deadline.subsec_nanos() > 0);
            // The controlled clock jumps to each target, so every tick is reached in turn.
            for tick in 1..ticks {
                advance(world, start + tick * 1000);
            }
            advance(world, start + ticks * 1000 - 1);
            assert_eq!(world.runtime.pending(), 1, "the countdown is still live");
            assert!(
                settled(future).is_none(),
                "the dialog waits until its deadline"
            );
            advance(world, start + ticks * 1000);
        }
        Scenario::Accept => {
            if *kind == Kind::Selector || answer.as_deref() == Some("No") {
                world.terminal.send_input("j");
            }
            if let Some(text) = text {
                world.terminal.send_input(text);
            }
            world.terminal.send_input("\r");
        }
    }
}

/// Opens, observes, ends and observes again every recorded dialog case of `test`.
fn dialog_group(test: &str) {
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::Dialog {
                    kind,
                    scenario,
                    timeout,
                    ..
                },
                Expected::Dialog {
                    first,
                    value,
                    after,
                },
            ) = (&input, expected)
            else {
                panic!("a dialog case");
            };
            let world = retained(directory).await;
            let signal = Cancellation::new();
            if *scenario == Scenario::Preabort {
                signal.abort();
            }
            let opts = Some(ExtensionUIDialogOptions {
                signal: Some(signal.clone()),
                timeout: *timeout,
            });
            let start = world.runtime.millis();
            let mut future = open(&world, *kind, &input, opts);
            let shown = observe(&world, !first.restored);
            assert_shown(shown, recorded(&world, first), *kind);
            let early = settled(&mut future);
            assert_eq!(early.is_none(), *scenario != Scenario::Preabort);
            finish(&world, &input, &signal, start, &mut future);
            assert_eq!(early.or_else(|| settled(&mut future)), Some(value));
            assert_eq!(observe(&world, false), after);
            signal.abort();
            assert!(world.runtime.drain_futures().is_empty());
            assert_eq!(world.runtime.pending(), 0, "a late abort requests nothing");
        }
    });
}

/// Compares the shown area with the recording; the multiline editor's own rows come from
/// its text area, which the recording host could not paint, so only its chrome is compared.
fn assert_shown(shown: Area, first: Area, kind: Kind) {
    if kind != Kind::Editor {
        assert_eq!(shown, first);
        return;
    }
    let (rows, recorded) = (shown.lines.clone().unwrap(), first.lines.clone().unwrap());
    assert_eq!(rows[..4], recorded[..4]);
    assert_eq!(rows[rows.len() - 5..], recorded[recorded.len() - 5..]);
    assert_eq!(
        Area {
            lines: None,
            ..shown
        },
        Area {
            lines: None,
            ..first
        }
    );
}

#[test]
fn dialog_answer_restores_editor_before_completion() {
    dialog_group("dialog_answer_restores_editor_before_completion");
}

#[test]
fn dialog_cancel_restores_editor_and_removes_abort_work() {
    dialog_group("dialog_cancel_restores_editor_and_removes_abort_work");
}

#[test]
fn aborted_dialogs_do_not_replace_or_render() {
    dialog_group("aborted_dialogs_do_not_replace_or_render");
}

#[test]
fn dialog_abort_restores_editor_and_stops_countdown() {
    dialog_group("dialog_abort_restores_editor_and_stops_countdown");
}

#[test]
fn dialog_timeout_uses_the_existing_countdown() {
    dialog_group("dialog_timeout_uses_the_existing_countdown");
}

#[test]
fn confirmation_maps_only_yes_to_true() {
    dialog_group("confirmation_maps_only_yes_to_true");
}

#[test]
fn multiline_dialog_restores_the_retained_editor() {
    dialog_group("multiline_dialog_restores_the_retained_editor");
}

#[test]
fn missing_directory_confirmation_returns_only_the_offered_fallback() {
    let test = "missing_directory_confirmation_returns_only_the_offered_fallback";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::MissingCwd {
                    fallback_cwd,
                    answer,
                },
                Expected::MissingCwd {
                    first,
                    value,
                    restored,
                },
            ) = (input, expected)
            else {
                panic!("a missing directory case");
            };
            let world = retained(directory).await;
            let error = MissingSessionCwdError::new(SessionCwdIssue {
                session_file: Some("/sessions/a.jsonl".to_owned()),
                session_cwd: "/missing".to_owned(),
                fallback_cwd,
            });
            let mut future = world.mode.prompt_for_missing_session_cwd(&error);
            assert_eq!(observe(&world, true), recorded(&world, first));
            if answer == "No" {
                world.terminal.send_input("j");
            }
            world
                .terminal
                .send_input(if answer == "cancel" { "\x1b" } else { "\r" });
            assert_eq!(settled(&mut future), Some(value));
            assert_eq!(observe(&world, false).restored, restored);
        }
    });
}

#[test]
fn dialog_nonpositive_timeout_and_completed_abort_leave_no_work() {
    let test = "dialog_nonpositive_timeout_and_completed_abort_leave_no_work";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::DialogNoTimeout { kind, timeout },
                Expected::NoTimeout {
                    active_timers,
                    render_requested,
                    late_abort_frame,
                },
            ) = (input, expected)
            else {
                panic!("a nonpositive countdown case");
            };
            let world = retained(directory).await;
            let host = port(&world.mode);
            let signal = Cancellation::new();
            let opts = Some(ExtensionUIDialogOptions {
                signal: Some(signal.clone()),
                timeout,
            });
            let mut future = match kind {
                Kind::Selector => host.select("Pick".to_owned(), Vec::new(), opts),
                _ => host.input("Input".to_owned(), None, opts),
            };
            let shown = observe(&world, true);
            assert_eq!(shown.active_timers, active_timers);
            world.terminal.send_input("\x1b");
            assert_eq!(settled(&mut future), Some(None));
            let settled_area = observe(&world, false);
            assert!(settled_area.restored);
            assert_eq!(
                shown.render_requested && settled_area.render_requested,
                render_requested
            );
            signal.abort();
            assert!(world.runtime.drain_futures().is_empty());
            assert_eq!(world.runtime.pending() > 0, late_abort_frame);
            assert_eq!(world.runtime.futures(), 0, "the abort observer ended");
        }
    });
}

#[test]
fn dialog_observer_drop_does_not_cancel_host_owned_work() {
    let test = "dialog_observer_drop_does_not_cancel_host_owned_work";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Expected>(test) {
            let (
                Input::DialogObserverRelease { kind, abort },
                Expected::ObserverRelease {
                    restored,
                    active_timers,
                },
            ) = (input, expected)
            else {
                panic!("an observer release case");
            };
            let world = retained(directory).await;
            let host = port(&world.mode);
            let signal = Cancellation::new();
            let opts = abort.then(|| ExtensionUIDialogOptions {
                signal: Some(signal.clone()),
                timeout: Some(5000.0),
            });
            let future = match kind {
                Kind::Selector => host.select("Choose".to_owned(), vec!["answer".to_owned()], opts),
                Kind::Input => host.input("Input".to_owned(), None, opts),
                _ => host.editor("Editor".to_owned(), None),
            };
            let shown = observe(&world, false);
            assert!(!shown.restored, "the dialog is shown");
            assert_eq!(
                shown.active_timers,
                usize::from(abort),
                "the countdown runs"
            );
            drop(future);
            if abort {
                signal.abort();
                assert!(world.runtime.drain_futures().is_empty());
            } else {
                world.terminal.send_input("\x1b");
            }
            let area = observe(&world, false);
            assert_eq!(area.restored, restored);
            assert_eq!(area.active_timers, active_timers);
        }
    });
}

#[test]
fn ui_context_retains_the_mode_but_completed_dialogs_leave_no_cycle() {
    isolated(
        "ui_context_retains_the_mode_but_completed_dialogs_leave_no_cycle",
        async |directory| {
            let World {
                mode,
                terminal,
                runtime,
                themes,
                host,
                loader,
            } = started(directory).await;
            let context = port(&mode);
            let weak = Rc::downgrade(&mode);
            drop(mode);
            assert!(weak.upgrade().is_some(), "the context keeps the mode");
            let mut selected = context.select(
                "Choose".to_owned(),
                vec!["answer".to_owned()],
                Some(ExtensionUIDialogOptions {
                    signal: Some(Cancellation::new()),
                    timeout: Some(5000.0),
                }),
            );
            terminal.send_input("\r");
            assert_eq!(settled(&mut selected), Some(Some("answer".to_owned())));
            let mut edited = context.editor("Edit".to_owned(), Some("text".to_owned()));
            terminal.send_input("\x1b");
            assert_eq!(settled(&mut edited), Some(None));
            assert!(runtime.drain_futures().is_empty());
            drop((selected, edited, context));
            assert!(weak.upgrade().is_none(), "nothing else keeps the mode");
            drop((terminal, runtime, themes, host, loader));
        },
    );
}

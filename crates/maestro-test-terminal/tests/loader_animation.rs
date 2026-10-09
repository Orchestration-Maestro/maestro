//! Loader lifecycle and callback ordering through its public component interface.
use maestro_tui::{Component, Loader, LoaderIndicatorOptions, TUI, TerminalImage};
use std::rc::Rc;
#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod manual_runtime;
    pub mod recording_terminal;
}
use support::checks::succeeds;

/// Checks each completed controlled callback.
fn all_succeed(results: Vec<std::io::Result<()>>) {
    for result in results {
        succeeds(result);
    }
}
use support::manual_runtime::{ManualRuntime, ms};
use support::recording_terminal::RecordingTerminal;

/// A writer with controlled time and recorded output.
fn writer() -> (TUI, RecordingTerminal, ManualRuntime) {
    let runtime = ManualRuntime::new();
    let terminal = RecordingTerminal::new(16, 24);
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    (tui, terminal, runtime)
}
/// Identity-colored message M with optional custom frames.
fn loader(tui: TUI, indicator: Option<LoaderIndicatorOptions>) -> Loader {
    Loader::new(
        tui,
        Rc::new(str::to_owned),
        Rc::new(str::to_owned),
        Some("M".into()),
        indicator,
    )
}
/// Literal padded rows for a short message in a fixed viewport.
fn shown(loader: &Loader, content: &str) {
    assert_eq!(
        loader.render(16),
        [String::new(), format!(" {content:<14} ")]
    );
}
/// Owned indicator configuration.
fn options(frames: &[&str], interval_ms: f64) -> LoaderIndicatorOptions {
    LoaderIndicatorOptions {
        frames: Some(frames.iter().map(|frame| (*frame).into()).collect()),
        interval_ms: Some(interval_ms),
    }
}
#[test]
fn loader_cycles_frames_at_the_requested_interval() {
    let (tui, _, runtime) = writer();
    let widget = loader(tui, None);
    shown(&widget, "⠋ M");
    all_succeed(runtime.advance_to(ms(79)));
    shown(&widget, "⠋ M");
    for (index, frame) in ["⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "⠋"]
        .into_iter()
        .enumerate()
    {
        all_succeed(runtime.advance_to(ms(80 * (u64::try_from(index).unwrap_or_default() + 1))));
        shown(&widget, &format!("{frame} M"));
    }
    widget.stop();
    let (tui, _, runtime) = writer();
    let widget = loader(tui, Some(options(&["", "B", ""], 80.0)));
    shown(&widget, "M");
    for (time, text) in [(80, "B M"), (160, "M"), (240, "M")] {
        all_succeed(runtime.advance_to(ms(time)));
        shown(&widget, text);
    }
}
#[test]
fn loader_restart_stop_and_message_keep_the_current_frame() {
    let (tui, _, runtime) = writer();
    let widget = loader(tui, None);
    all_succeed(runtime.advance_to(ms(80)));
    all_succeed(runtime.advance_to(ms(120)));
    widget.start();
    shown(&widget, "⠙ M");
    all_succeed(runtime.advance_to(ms(160)));
    shown(&widget, "⠙ M");
    all_succeed(runtime.advance_to(ms(200)));
    shown(&widget, "⠹ M");
    widget.stop();
    widget.stop();
    widget.set_message("N".into());
    shown(&widget, "⠹ N");
    all_succeed(runtime.advance_to(ms(400)));
    shown(&widget, "⠹ N");
    widget.start();
    all_succeed(runtime.advance_to(ms(480)));
    shown(&widget, "⠸ N");
}
#[test]
fn loader_indicator_replacement_resets_and_restarts() {
    let (tui, _, runtime) = writer();
    let widget = Loader::new(
        tui,
        Rc::new(|frame| format!("S{frame}")),
        Rc::new(str::to_owned),
        Some("M".into()),
        None,
    );
    all_succeed(runtime.advance_to(ms(80)));
    shown(&widget, "S⠙ M");
    widget.set_indicator(Some(options(&["A", "B"], 15.0)));
    shown(&widget, "A M");
    all_succeed(runtime.advance_to(ms(94)));
    shown(&widget, "A M");
    all_succeed(runtime.advance_to(ms(95)));
    shown(&widget, "B M");
    widget.set_indicator(Some(options(&["A", "B"], 15.0)));
    shown(&widget, "A M");
    all_succeed(runtime.advance_to(ms(110)));
    shown(&widget, "B M");
    widget.set_indicator(Some(options(&["*"], 15.0)));
    all_succeed(runtime.advance_to(ms(200)));
    shown(&widget, "* M");
    assert_eq!(runtime.pending(), 0);
    widget.set_indicator(Some(options(&[], 15.0)));
    shown(&widget, "M");
    assert_eq!(runtime.pending(), 0);
    widget.set_indicator(Some(LoaderIndicatorOptions::default()));
    shown(&widget, "⠋ M");
    all_succeed(runtime.advance_to(ms(280)));
    shown(&widget, "⠙ M");
    widget.stop();
    widget.set_indicator(None);
    shown(&widget, "S⠋ M");
    all_succeed(runtime.advance_to(ms(360)));
    shown(&widget, "S⠙ M");
}
#[test]
fn loader_indicator_frames_are_an_owned_snapshot() {
    let (tui, _, runtime) = writer();
    let mut frames = vec!["A".to_owned(), "B".to_owned()];
    let widget = loader(
        tui,
        Some(LoaderIndicatorOptions {
            frames: Some(frames.clone()),
            interval_ms: Some(10.0),
        }),
    );
    frames[1] = "Z".into();
    all_succeed(runtime.advance_to(ms(10)));
    shown(&widget, "B M");
    widget.set_indicator(Some(LoaderIndicatorOptions {
        frames: Some(frames),
        interval_ms: Some(10.0),
    }));
    shown(&widget, "A M");
    all_succeed(runtime.advance_to(ms(20)));
    shown(&widget, "Z M");
}
#[test]
fn loader_interval_defaults_follow_option_normalization() {
    for interval_ms in [
        None,
        Some(0.0),
        Some(-0.0),
        Some(-1.0),
        Some(f64::NEG_INFINITY),
        Some(f64::NAN),
    ] {
        let (tui, _, runtime) = writer();
        let widget = Loader::new(
            tui,
            Rc::new(|_| "colored".into()),
            Rc::new(str::to_owned),
            Some("M".into()),
            Some(LoaderIndicatorOptions {
                frames: None,
                interval_ms,
            }),
        );
        shown(&widget, "⠋ M");
        all_succeed(runtime.advance_to(ms(79)));
        shown(&widget, "⠋ M");
        all_succeed(runtime.advance_to(ms(80)));
        shown(&widget, "⠙ M");
    }
}
#[test]
fn loader_positive_intervals_use_native_duration_limits() {
    use std::time::Duration;
    let cases = [
        (0.000_000_1, Some(Duration::from_nanos(1))),
        (0.000_001, Some(Duration::from_nanos(1))),
        (0.25, Some(Duration::from_micros(250))),
        (1.75, Some(Duration::from_micros(1750))),
        (80.0, Some(ms(80))),
        (2_147_483_647.0, Some(ms(2_147_483_647))),
        (2_147_483_648.0, Some(ms(2_147_483_648))),
        (1e22, Some(Duration::from_secs(10_000_000_000_000_000_000))),
        (2e22, None),
        (f64::MAX, None),
        (f64::INFINITY, None),
    ];
    for (interval, expected) in cases {
        let (tui, _, runtime) = writer();
        succeeds(tui.stop());
        let widget = loader(tui, Some(options(&["A", "B"], interval)));
        assert_eq!(runtime.next_deadline(), expected, "{interval}");
        shown(&widget, "A M");
        if let Some(delay) = expected.filter(|delay| *delay < ms(1000)) {
            all_succeed(runtime.advance_to(delay.saturating_sub(Duration::from_nanos(1))));
            shown(&widget, "A M");
            all_succeed(runtime.advance_to(delay));
            shown(&widget, "B M");
        }
        widget.stop();
    }
    for interval in [f64::INFINITY, 0.25] {
        for frames in [&[][..], &["*"][..]] {
            let (tui, _, runtime) = writer();
            succeeds(tui.stop());
            let widget = loader(tui, Some(options(frames, interval)));
            assert_eq!(runtime.pending(), 0);
            shown(&widget, if frames.is_empty() { "M" } else { "* M" });
        }
    }
}

/// Synchronous actions invoked once from a color callback.
#[derive(Clone, Copy, Debug)]
enum Action {
    /// Cancel the current continuation.
    Stop,
    /// Replace the message during composition.
    Message,
    /// Replace the frames and interval.
    Indicator,
    /// Refresh and restart.
    Start,
    /// Drop Text's cached rows.
    Invalidate,
}
/// Applies a callback's one-shot action.
fn act(widget: &Loader, action: Action) {
    match action {
        Action::Stop => widget.stop(),
        Action::Message => widget.set_message("new".into()),
        Action::Indicator => widget.set_indicator(Some(options(&["X", "Y"], 7.0))),
        Action::Start => widget.start(),
        Action::Invalidate => widget.invalidate(),
    }
}
/// Recorded color calls.
type Calls = std::cell::RefCell<Vec<(&'static str, String)>>;
/// Invokes a one-shot action without retaining a borrow of the callback's handle slot.
fn reentrant_color(
    name: &'static str,
    calls: &Calls,
    slot: &std::cell::RefCell<Option<Loader>>,
    action: &std::cell::Cell<Option<Action>>,
    text: &str,
) -> String {
    calls.borrow_mut().push((name, text.into()));
    if let Some(action) = action.take() {
        let widget = slot.borrow().clone();
        assert!(widget.is_some());
        if let Some(widget) = widget {
            act(&widget, action);
        }
    }
    text.into()
}
/// Exercises one callback location and one action against recorded source ordering.
fn reentry(from_spinner: bool, action: Action) {
    use std::cell::{Cell, RefCell};
    let (tui, _, runtime) = writer();
    succeeds(tui.stop());
    let slot = Rc::new(RefCell::new(None));
    let calls = Rc::new(Calls::default());
    let spinner_action = Rc::new(Cell::new(None));
    let message_action = Rc::new(Cell::new(None));
    let spinner = {
        let (calls, slot, action) = (
            Rc::clone(&calls),
            Rc::clone(&slot),
            Rc::clone(&spinner_action),
        );
        Rc::new(move |text: &str| reentrant_color("spinner", &calls, &slot, &action, text))
    };
    let message = {
        let (calls, slot, action) = (
            Rc::clone(&calls),
            Rc::clone(&slot),
            Rc::clone(&message_action),
        );
        Rc::new(move |text: &str| reentrant_color("message", &calls, &slot, &action, text))
    };
    let widget = Loader::new(tui, spinner, message, Some("old".into()), None);
    *slot.borrow_mut() = Some(widget.clone());
    calls.borrow_mut().clear();
    if from_spinner {
        spinner_action.set(Some(action));
    } else {
        message_action.set(Some(action));
    }
    all_succeed(runtime.advance_to(ms(80)));
    assert_calls(&calls, reentry_calls(from_spinner, action));
    shown(
        &widget,
        if from_spinner && matches!(action, Action::Message) {
            "⠙ new"
        } else {
            "⠙ old"
        },
    );
    assert_eq!(
        runtime.next_deadline(),
        match action {
            Action::Stop => None,
            Action::Indicator => Some(ms(87)),
            _ => Some(ms(160)),
        }
    );
    calls.borrow_mut().clear();
    all_succeed(runtime.advance_to(if matches!(action, Action::Indicator) {
        ms(87)
    } else {
        ms(160)
    }));
    following_reentry(&widget, &calls, action);
    slot.borrow_mut().take();
}
/// Compares complete callback order and raw arguments.
fn assert_calls(calls: &Calls, expected: &[(&str, &str)]) {
    let calls = calls.borrow();
    let observed: Vec<_> = calls
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    assert_eq!(observed, expected);
}
#[test]
fn loader_color_callbacks_allow_ordered_reentry() {
    use std::cell::{Cell, RefCell};
    for from_spinner in [true, false] {
        for action in [
            Action::Stop,
            Action::Message,
            Action::Indicator,
            Action::Start,
            Action::Invalidate,
        ] {
            reentry(from_spinner, action);
        }
    }
    let (tui, _, runtime) = writer();
    succeeds(tui.stop());
    let slot = Rc::new(RefCell::new(None));
    let calls = Rc::new(Calls::default());
    let action = Rc::new(Cell::new(None));
    let spinner = {
        let (slot, calls, action) = (Rc::clone(&slot), Rc::clone(&calls), Rc::clone(&action));
        Rc::new(move |text: &str| reentrant_color("spinner", &calls, &slot, &action, text))
    };
    let message = {
        let calls = Rc::clone(&calls);
        Rc::new(move |text: &str| {
            calls.borrow_mut().push(("message", text.into()));
            text.into()
        })
    };
    let widget = Loader::new(tui, spinner, message, Some("M".into()), None);
    *slot.borrow_mut() = Some(widget.clone());
    calls.borrow_mut().clear();
    action.set(Some(Action::Stop));
    widget.start();
    assert_calls(&calls, &[("spinner", "⠋"), ("message", "M")]);
    assert_eq!(runtime.next_deadline(), Some(ms(80)));
    calls.borrow_mut().clear();
    all_succeed(runtime.advance_to(ms(80)));
    shown(&widget, "⠙ M");
    assert_calls(&calls, &[("spinner", "⠙"), ("message", "M")]);
    slot.borrow_mut().take();
}
#[test]
fn loader_colors_refresh_on_updates_not_plain_renders() {
    use std::cell::Cell;
    let (tui, _, _) = writer();
    let style = Rc::new(Cell::new('A'));
    let spinner_style = Rc::clone(&style);
    let message_style = Rc::clone(&style);
    let widget = Loader::new(
        tui,
        Rc::new(move |frame| format!("{}:{frame}", spinner_style.get())),
        Rc::new(move |message| format!("{}:{message}", message_style.get())),
        Some("M".into()),
        None,
    );
    shown(&widget, "A:⠋ A:M");
    style.set('B');
    shown(&widget, "A:⠋ A:M");
    widget.invalidate();
    shown(&widget, "A:⠋ A:M");
    widget.start();
    shown(&widget, "B:⠋ B:M");
    widget.set_message("Still loading...".into());
    assert_eq!(
        widget.render(16),
        ["", " B:⠋ B:Still    ", " loading...     "]
    );
}
#[test]
fn loader_updates_use_the_existing_frame_writer() {
    let (tui, terminal, runtime) = writer();
    let widget = loader(tui.clone(), Some(options(&["A", "B"], 80.0)));
    tui.add_child(Rc::new(widget.clone()));
    all_succeed(runtime.advance_to(ms(16)));
    let mut screen = vt100::Parser::new(24, 16, 0);
    for write in terminal.writes() {
        screen.process(write.as_bytes());
    }
    assert_eq!(screen.screen().contents(), "\n A M            ");
    terminal.clear_writes();
    widget.set_message("N".into());
    widget.set_message("Updated".into());
    all_succeed(runtime.advance_to(ms(32)));
    for write in terminal.writes() {
        screen.process(write.as_bytes());
    }
    assert_eq!(screen.screen().contents(), "\n A Updated      ");
    terminal.clear_writes();
    all_succeed(runtime.advance_to(ms(80)));
    widget.stop();
    succeeds(runtime.settle());
    for write in terminal.writes() {
        screen.process(write.as_bytes());
    }
    assert_eq!(screen.screen().contents(), "\n B Updated      ");
}
#[test]
fn loader_shared_handles_release_animation_without_writer_cycles() {
    use maestro_tui::tui::ComponentHandle;
    let (tui, _, runtime) = writer();
    let widget = loader(tui.clone(), None);
    let clone = widget.clone();
    widget.set_message("N".into());
    shown(&clone, "⠋ N");
    drop(widget);
    all_succeed(runtime.advance_to(ms(80)));
    shown(&clone, "⠙ N");
    let child: ComponentHandle = Rc::new(clone);
    let weak_child = Rc::downgrade(&child);
    tui.add_child(Rc::clone(&child));
    tui.remove_child(&child);
    drop(child);
    assert!(weak_child.upgrade().is_none());
    succeeds(runtime.settle());
    assert_eq!(runtime.pending(), 0);

    let (tui, _, runtime) = writer();
    let weak_terminal = Rc::downgrade(tui.terminal());
    let widget = loader(tui.clone(), None);
    let child: ComponentHandle = Rc::new(widget.clone());
    let weak_child = Rc::downgrade(&child);
    tui.add_child(child);
    drop(tui);
    assert!(weak_terminal.upgrade().is_none());
    assert!(weak_child.upgrade().is_none());
    widget.set_message("Retained".into());
    all_succeed(runtime.advance_to(ms(80)));
    shown(&widget, "⠙ Retained");
    drop(widget);
    succeeds(runtime.settle());
    assert_eq!(runtime.pending(), 0);
}

/// Full nested callback sequences recorded for the first tick.
fn reentry_calls(from_spinner: bool, action: Action) -> &'static [(&'static str, &'static str)] {
    match (from_spinner, action) {
        (true, Action::Message) => &[
            ("spinner", "⠙"),
            ("spinner", "⠙"),
            ("message", "new"),
            ("message", "new"),
        ],
        (false, Action::Message) => &[
            ("spinner", "⠙"),
            ("message", "old"),
            ("spinner", "⠙"),
            ("message", "new"),
        ],
        (_, Action::Indicator) => &[("spinner", "⠙"), ("message", "old"), ("message", "old")],
        (true, Action::Start) => &[
            ("spinner", "⠙"),
            ("spinner", "⠙"),
            ("message", "old"),
            ("message", "old"),
        ],
        (false, Action::Start) => &[
            ("spinner", "⠙"),
            ("message", "old"),
            ("spinner", "⠙"),
            ("message", "old"),
        ],
        (_, Action::Stop | Action::Invalidate) => &[("spinner", "⠙"), ("message", "old")],
    }
}
/// Observations after advancing controlled time, including a cancelled continuation.
fn following_reentry(widget: &Loader, calls: &Calls, action: Action) {
    match action {
        Action::Stop => {
            shown(widget, "⠙ old");
            assert_calls(calls, &[]);
        }
        Action::Indicator => {
            shown(widget, "Y old");
            assert_calls(calls, &[("message", "old")]);
        }
        Action::Message => {
            shown(widget, "⠹ new");
            assert_calls(calls, &[("spinner", "⠹"), ("message", "new")]);
        }
        Action::Start | Action::Invalidate => {
            shown(widget, "⠹ old");
            assert_calls(calls, &[("spinner", "⠹"), ("message", "old")]);
        }
    }
}

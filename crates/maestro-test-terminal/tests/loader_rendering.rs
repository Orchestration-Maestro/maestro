//! Exact loader rows and callback arguments from controlled rendering cases.
use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::{Component, Loader, LoaderIndicatorOptions, TUI, TerminalImage};
use serde::Deserialize;

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod manual_runtime;
    pub mod recording_terminal;
}
use support::checks::{parse, succeeds};
use support::manual_runtime::ManualRuntime;
use support::recording_terminal::RecordingTerminal;

/// One distinct rendering query and its complete observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Behavior that owns this query.
    test: String,
    /// Arguments to the public widget.
    input: Input,
    /// Complete rows and callback arguments.
    expected: Expected,
}
/// Optional construction and rendering arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Message, including explicitly empty strings.
    message: Option<String>,
    /// Presence selects verbatim frames.
    indicator: Option<Indicator>,
    /// Callback behavior.
    style: Option<Style>,
    /// Viewport columns.
    width: Option<usize>,
}
/// Styles used by the fixture callbacks.
#[derive(Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
enum Style {
    /// Cyan spinner and dim message.
    #[default]
    Colored,
    /// Returns supplied text unchanged.
    Identity,
    /// Spinner callback returns no text.
    EmptySpinner,
}
/// Supplied indicator options.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Indicator {
    /// Owned frames.
    frames: Option<Vec<String>>,
    /// Requested milliseconds.
    #[serde(rename = "intervalMs")]
    interval_ms: Option<f64>,
}
/// Independently recorded output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Leading row and padded text rows.
    rows: Vec<String>,
    /// Ordered callback name/argument pairs.
    calls: Vec<[String; 2]>,
}
/// Executes only the queries owned by a named behavior.
fn cases(name: &str, count: usize) {
    let corpus: Vec<Case> = parse(include_str!("fixtures/loader_cases.json"));
    if name == "loader_defaults_style_the_spinner_and_message" {
        assert_eq!(corpus.len(), 35);
    }
    let mut consumed = 0;
    for case in corpus.into_iter().filter(|case| case.test == name) {
        consumed += 1;
        let runtime = ManualRuntime::new();
        let terminal = RecordingTerminal::new(16, 24);
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::default(),
            None,
        );
        succeeds(tui.stop());
        let calls = Rc::new(RefCell::new(Vec::<[String; 2]>::new()));
        let width = case.input.width.unwrap_or(16);
        let loader = construct(tui, case.input, &calls);
        assert_eq!(loader.render(width), case.expected.rows);
        assert_eq!(*calls.borrow(), case.expected.calls);
        if name == "loader_defaults_style_the_spinner_and_message" && consumed == 1 {
            assert!(loader.input_handler().is_none());
            assert!(loader.focusable().is_none());
            assert!(!loader.wants_key_release());
        }
        if name == "loader_static_indicators_keep_spacing_without_animation" {
            assert_eq!(runtime.pending(), 0);
        }
        loader.stop();
    }
    assert_eq!(consumed, count);
}
#[test]
fn loader_defaults_style_the_spinner_and_message() {
    cases("loader_defaults_style_the_spinner_and_message", 5);
}

#[test]
fn loader_static_indicators_keep_spacing_without_animation() {
    cases("loader_static_indicators_keep_spacing_without_animation", 5);
}

#[test]
fn loader_blank_messages_keep_the_leading_row() {
    cases("loader_blank_messages_keep_the_leading_row", 3);
}

#[test]
fn loader_layout_delegates_wrapping_and_escapes_to_text() {
    cases("loader_layout_delegates_wrapping_and_escapes_to_text", 2);
}

#[test]
fn loader_narrow_rows_reuse_bounded_text_layout() {
    cases("loader_narrow_rows_reuse_bounded_text_layout", 20);
}

/// Constructs the widget with supplied construction options and recorded callbacks.
fn construct(tui: TUI, input: Input, calls: &Rc<RefCell<Vec<[String; 2]>>>) -> Loader {
    let spinner_calls = Rc::clone(calls);
    let message_calls = Rc::clone(calls);
    let style = Rc::new(input.style.unwrap_or_default());
    let spinner_style = Rc::clone(&style);
    Loader::new(
        tui,
        Rc::new(move |text| {
            spinner_calls
                .borrow_mut()
                .push(["spinner".into(), text.into()]);
            match *spinner_style {
                Style::Colored => format!("\x1b[36m{text}\x1b[39m"),
                Style::Identity => text.into(),
                Style::EmptySpinner => String::new(),
            }
        }),
        Rc::new(move |text| {
            message_calls
                .borrow_mut()
                .push(["message".into(), text.into()]);
            match *style {
                Style::Identity => text.into(),
                Style::Colored | Style::EmptySpinner => format!("\x1b[2m{text}\x1b[22m"),
            }
        }),
        input.message,
        input.indicator.map(|options| LoaderIndicatorOptions {
            frames: options.frames,
            interval_ms: options.interval_ms,
        }),
    )
}

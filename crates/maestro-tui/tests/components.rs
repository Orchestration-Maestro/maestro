//! Reusable component, overlay, terminal, editor and completion contracts.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};
use std::time::Duration;

use maestro_tui::autocomplete::{ArgumentCompletions, CompletionOptions, CursorPosition};
use maestro_tui::editor_component::BorderColor;
use maestro_tui::tui::{ChildArray, ComponentHandle, InputHandler};
use maestro_tui::{
    AutocompleteProvider, CURSOR_MARKER, Component, Container, EditorComponent, OverlayMargin,
    OverlayOptions, SlashCommand, Terminal, TruncatedText, is_focusable, visible_width,
};

mod fixtures {
    pub mod editors;
    pub mod futures;
    pub mod providers;
    pub mod terminals;
    pub mod truncated_cases;
    pub mod widgets;
}
use fixtures::editors::{Bare, Rich};
use fixtures::futures::{YieldOnce, block_on};
use fixtures::providers::{CommandProvider, CountingProvider, item};
use fixtures::terminals::{Probe, Recording, Screen};
use fixtures::truncated_cases::TRUNCATED_CASES;
use fixtures::widgets::{Block, Field, InvalidationTrace, Passive};

const RESET: &str = "\x1b[0m";

#[test]
fn component_capabilities_distinguish_missing_input_and_focus() {
    let passive = Passive("p");
    assert!(passive.input_handler().is_none() && passive.focusable().is_none());
    assert!(!passive.wants_key_release());
    assert!(!is_focusable(Some(&passive)) && !is_focusable(None));

    let field = Field::default();
    assert!(
        !field.focus.get() && is_focusable(Some(&field)),
        "an unfocused field is still focusable"
    );
    if let Some(focus) = field.focusable() {
        focus.focus_flag().set(true);
    }
    assert!(
        field
            .focusable()
            .is_some_and(|focus| focus.focus_flag().get())
    );
    assert_eq!(field.render(10), [format!("> {CURSOR_MARKER}")]);
    assert_eq!(visible_width(CURSOR_MARKER), 0);

    if let Some(input) = field.input_handler() {
        input.handle_input("\x1b[A");
    }
    assert_eq!(*field.received.borrow(), ["\x1b[A"]);
    assert!(!field.wants_key_release());
    field.release.set(true);
    assert!(field.wants_key_release());
}

#[test]
fn container_retains_shared_children_order_and_invalidation() {
    let trace = InvalidationTrace::default();
    let first: ComponentHandle = Rc::new(Block {
        name: "first",
        lines: RefCell::new(vec!["one".into(), "two".into()]),
        trace: Rc::clone(&trace),
    });
    let second = Rc::new(Block {
        name: "second",
        lines: RefCell::new(vec!["three".into()]),
        trace: Rc::clone(&trace),
    });
    let second_handle: ComponentHandle = second.clone();
    let passive: ComponentHandle = Rc::new(Passive("p"));

    let container = Container::new();
    for child in [&second_handle, &first, &passive, &first] {
        container.add_child(Rc::clone(child));
    }
    assert_eq!(
        container.render(7),
        ["three", "one", "two", "p:7", "one", "two"]
    );

    second.lines.borrow_mut().push("edited".into());
    assert_eq!(container.render(7)[..2], ["three", "edited"]);

    container.invalidate();
    assert_eq!(
        *trace.borrow(),
        ["second", "first", "first"],
        "children are invalidated in order, a shared child once per position"
    );

    container.remove_child(&first);
    let children = container.children();
    assert_eq!(children.borrow().len(), 3);
    assert!(Rc::ptr_eq(&children.borrow()[0], &second_handle));
    assert!(
        Rc::ptr_eq(&children.borrow()[1], &passive),
        "only the first occurrence leaves"
    );
    assert!(Rc::ptr_eq(&children.borrow()[2], &first));
    let stranger: ComponentHandle = Rc::new(Passive("x"));
    container.remove_child(&stranger);
    assert_eq!(children.borrow().len(), 3);

    container.clear();
    assert!(container.children().borrow().is_empty() && container.render(7).is_empty());
    assert!(Container::default().children().borrow().is_empty());
}

#[test]
fn container_children_is_the_rendered_array_that_assignment_replaces() {
    let (a, b, c): (ComponentHandle, ComponentHandle, ComponentHandle) = (
        Rc::new(Passive("a")),
        Rc::new(Passive("b")),
        Rc::new(Passive("c")),
    );
    let container = Container::new();
    container.add_child(Rc::clone(&a));
    let kept = container.children();
    assert!(
        Rc::ptr_eq(&kept, &container.children()),
        "one array, not copies"
    );

    kept.borrow_mut().push(Rc::clone(&b));
    assert_eq!(
        container.render(7),
        ["a:7", "b:7"],
        "an addition through the handle is seen"
    );
    kept.borrow_mut().remove(0);
    assert_eq!(
        container.render(7),
        ["b:7"],
        "an edit through the handle shows in the next render"
    );
    container.add_child(Rc::clone(&c));
    assert_eq!(
        kept.borrow().len(),
        2,
        "an addition to the container shows in the handle"
    );

    let replacement: ChildArray = Rc::new(RefCell::new(vec![Rc::clone(&a)]));
    container.set_children(Rc::clone(&replacement));
    assert!(Rc::ptr_eq(&container.children(), &replacement));
    kept.borrow_mut().clear();
    assert_eq!(
        container.render(7),
        ["a:7"],
        "the old handle no longer reaches rendering"
    );
    replacement.borrow_mut().push(Rc::clone(&c));
    assert_eq!(
        container.render(7),
        ["a:7", "c:7"],
        "the assigned array is the one rendered"
    );

    container.clear();
    replacement.borrow_mut().push(b);
    assert!(
        container.render(7).is_empty(),
        "clear leaves the old array to its holders"
    );
    assert_eq!(replacement.borrow().len(), 3);
    assert!(!Rc::ptr_eq(&container.children(), &replacement));
}

#[test]
fn tui_module_reexports_the_width_measure() {
    assert_eq!(maestro_tui::tui::visible_width("\x1b[31mHello\x1b[0m"), 5);
}

fn render_text(text: &str, padding_x: usize, padding_y: usize, width: usize) -> Vec<String> {
    TruncatedText::new(text.to_owned(), padding_x, padding_y).render(width)
}

#[test]
fn single_line_padding_fills_viewport() {
    assert_eq!(
        render_text("Hello world", 1, 0, 50),
        [format!(" Hello world{}", " ".repeat(38))]
    );
    assert_eq!(
        render_text("Hello world", 1, 0, 30),
        [format!(" Hello world{}", " ".repeat(18))]
    );
}

#[test]
fn vertical_padding_has_full_width() {
    let lines = render_text("Hello", 0, 2, 40);
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], " ".repeat(40));
    assert_eq!(lines[2], format!("Hello{}", " ".repeat(35)));
    assert!(lines.iter().all(|line| visible_width(line) == 40));
}

#[test]
fn long_first_line_truncates_before_padding() {
    let text = "This is a very long piece of text that will definitely exceed the available width";
    assert_eq!(
        render_text(text, 1, 0, 30),
        [format!(" This is a very long piece{RESET}...{RESET} ")]
    );
}

#[test]
fn styled_first_line_keeps_color_and_padding() {
    let text = "\x1b[31mHello\x1b[0m \x1b[34mworld\x1b[0m";
    assert_eq!(
        render_text(text, 1, 0, 40),
        [format!(" {text}{}", " ".repeat(28))]
    );
}

#[test]
fn styled_first_line_resets_before_ellipsis() {
    let text = "\x1b[31mThis is a very long red text that will be truncated\x1b[0m";
    let lines = render_text(text, 1, 0, 20);
    assert_eq!(
        lines,
        [format!(" \x1b[31mThis is a very {RESET}...{RESET} ")]
    );
    assert_eq!(visible_width(&lines[0]), 20);
}

#[test]
fn empty_first_line_is_padded() {
    assert_eq!(render_text("", 1, 0, 30), [" ".repeat(30)]);
    assert_eq!(render_text("\nsecond", 1, 0, 6), ["      "]);
}

#[test]
fn component_hides_later_source_lines() {
    let lines = render_text("First line\nSecond line\nThird line", 1, 0, 40);
    assert_eq!(lines, [format!(" First line{}", " ".repeat(29))]);
}

#[test]
fn long_multiline_text_only_truncates_first_line() {
    let text = "This is a very long first line that needs truncation\nSecond line";
    assert_eq!(
        render_text(text, 1, 0, 25),
        [format!(" This is a very long {RESET}...{RESET} ")]
    );
}

#[test]
fn truncated_component_bounds_padding_and_first_line() {
    for (text, padding_x, width, expected) in TRUNCATED_CASES {
        assert_eq!(
            render_text(text, *padding_x, 1, *width),
            *expected,
            "{text:?} {padding_x} {width}"
        );
    }
}

fn assert_overlay_defaults() {
    let defaults = OverlayOptions::default();
    assert!(
        defaults.width.is_none() && defaults.min_width.is_none() && defaults.max_height.is_none()
    );
    assert!(
        defaults.anchor.is_none() && defaults.offset_x.is_none() && defaults.offset_y.is_none()
    );
    assert!(defaults.row.is_none() && defaults.col.is_none() && defaults.margin.is_none());
    assert!(defaults.visible.is_none() && defaults.non_capturing.is_none());
    let sides = OverlayMargin::default();
    assert!(
        [sides.top, sides.right, sides.bottom, sides.left]
            .iter()
            .all(Option::is_none)
    );
}

fn visibility_options(seen: &Rc<RefCell<Vec<(usize, usize)>>>) -> OverlayOptions {
    let recorder = Rc::clone(seen);
    OverlayOptions {
        visible: Some(Rc::new(move |width, height| {
            recorder.borrow_mut().push((width, height));
            width >= 80
        })),
        ..OverlayOptions::default()
    }
}

#[test]
fn overlay_defaults_unset_visibility_callback() {
    assert_overlay_defaults();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let options = visibility_options(&seen);
    let visible = options
        .visible
        .as_ref()
        .map(|callback| (callback(100, 30), callback(79, 24)));
    assert_eq!(visible, Some((true, false)));
    assert_eq!(*seen.borrow(), [(100, 30), (79, 24)]);
}

/// Drives any terminal through every operation.
fn paint(terminal: &mut dyn Terminal) -> io::Result<()> {
    terminal.start(Box::new(|_| {}), Box::new(|| {}))?;
    terminal.hide_cursor()?;
    terminal.set_title("maestro")?;
    terminal.set_progress(true)?;
    terminal.move_by(-2)?;
    terminal.move_by(3)?;
    terminal.clear_line()?;
    terminal.clear_from_cursor()?;
    terminal.clear_screen()?;
    terminal.write("hello")?;
    terminal.set_progress(false)?;
    terminal.show_cursor()?;
    block_on(terminal.drain_input(None, None))?;
    let (max, idle) = (Duration::from_millis(1000), Duration::from_millis(50));
    block_on(terminal.drain_input(Some(max), Some(idle)))?;
    terminal.stop()
}

/// Input and resize callbacks reach the caller through an adapter.
fn assert_callbacks_reach_the_caller(terminal: &mut dyn Probe) -> io::Result<()> {
    let received = Rc::new(RefCell::new(Vec::new()));
    let resizes = Rc::new(Cell::new(0));
    let (sink, counter) = (Rc::clone(&received), Rc::clone(&resizes));
    terminal.start(
        Box::new(move |data| sink.borrow_mut().push(data.to_owned())),
        Box::new(move || counter.set(counter.get() + 1)),
    )?;
    terminal.press("\x1b[A");
    terminal.press("q");
    terminal.resize();
    assert_eq!(
        (received.take(), resizes.get()),
        (vec!["\x1b[A".to_owned(), "q".to_owned()], 1)
    );
    Ok(())
}

#[test]
fn terminal_contracts_swap_adapters_without_changing_callers() {
    let mut recording = Recording::new(false);
    paint(&mut recording).expect("the recording adapter accepts every effect");
    assert_eq!(
        recording.log,
        [
            "start",
            "hide",
            "title maestro",
            "progress true",
            "move -2",
            "move 3",
            "clear-line",
            "clear-below",
            "clear-screen",
            "write hello",
            "progress false",
            "show",
            "drain None None",
            "drain Some(1s) Some(50ms)",
            "stop",
        ]
    );
    assert_eq!(
        (
            recording.columns(),
            recording.rows(),
            recording.kitty_protocol_active()
        ),
        (80, 24, true)
    );

    let mut screen = Screen::default();
    paint(&mut screen).expect("the screen adapter accepts every effect");
    assert_eq!(
        screen.output,
        "\x1b[?25l\x1b]0;maestro\x07\x1b]9;4;3\x07\x1b[2A\x1b[3B\x1b[2K\x1b[J\x1b[2J\x1b[Hhello\x1b]9;4;0;\x07\x1b[?25h"
    );
    assert_eq!(
        (
            screen.columns(),
            screen.rows(),
            screen.kitty_protocol_active()
        ),
        (120, 40, false)
    );

    let mut refusing = Recording::new(true);
    let failure = paint(&mut refusing).expect_err("a refused write reaches the caller");
    assert_eq!(
        (failure.kind(), failure.to_string().as_str()),
        (io::ErrorKind::BrokenPipe, "write refused")
    );
    assert_eq!(
        refusing.log.last().map(String::as_str),
        Some("clear-screen")
    );

    assert_callbacks_reach_the_caller(&mut Recording::new(false)).expect("recording starts");
    assert_callbacks_reach_the_caller(&mut Screen::default()).expect("screen starts");
}

fn suggestions_len(
    lines: &[String],
    cursor: CursorPosition,
    signal: &Cell<bool>,
    force: Option<bool>,
) -> Result<Option<usize>, maestro_tui::autocomplete::CompletionError> {
    let options = CompletionOptions { signal, force };
    Ok(
        block_on(CommandProvider.get_suggestions(lines, cursor, options))?
            .map(|found| found.items.len()),
    )
}

fn assert_command_provider() -> Result<(), maestro_tui::autocomplete::CompletionError> {
    let lines = vec!["\u{e9} /he".to_owned()];
    let cursor = CursorPosition {
        line: 0,
        col: lines[0].len(),
    };
    assert_eq!(
        cursor.col, 6,
        "the cursor counts bytes, not cells or scalars"
    );

    let flag = Cell::new(false);
    let options = CompletionOptions {
        signal: &flag,
        force: None,
    };
    let found = block_on(CommandProvider.get_suggestions(&lines, cursor, options))?;
    assert_eq!(
        found.as_ref().map(|found| found.prefix.as_str()),
        Some("/he")
    );
    let values: Vec<_> = found
        .iter()
        .flat_map(|found| &found.items)
        .map(|chosen| chosen.value.as_str())
        .collect();
    assert_eq!(values, ["/help", "/hello"]);
    assert_eq!(suggestions_len(&lines, cursor, &flag, Some(true))?, Some(3));
    assert_eq!(
        suggestions_len(&lines, cursor, &flag, Some(false))?,
        Some(2)
    );
    flag.set(true);
    assert_eq!(
        suggestions_len(&lines, cursor, &flag, None)?,
        None,
        "raising the signal cancels the request"
    );

    let applied = CommandProvider.apply_completion(&lines, cursor, &item("/help"), "/he");
    assert_eq!(applied.lines, ["\u{e9} /help"]);
    assert_eq!((applied.cursor_line, applied.cursor_col), (0, 8));
    assert_eq!(
        applied.lines[0].chars().count(),
        7,
        "byte offset 8 differs from the scalar count"
    );
    assert_eq!(
        CommandProvider.should_trigger_file_completion(&lines, cursor),
        Some(true)
    );
    let plain = vec!["no slash".to_owned()];
    assert_eq!(
        CommandProvider.should_trigger_file_completion(&plain, CursorPosition { line: 0, col: 8 }),
        Some(false)
    );
    assert_eq!(
        CountingProvider.should_trigger_file_completion(&lines, cursor),
        None
    );
    Ok(())
}

fn assert_counting_provider() -> Result<(), maestro_tui::autocomplete::CompletionError> {
    let seen = RefCell::new(String::new());
    for (text, expected) in [("", None), ("none", Some(0)), ("some", Some(1))] {
        let lines = vec![text.to_owned()];
        let options = CompletionOptions {
            signal: &seen,
            force: None,
        };
        let result = block_on(CountingProvider.get_suggestions(
            &lines,
            CursorPosition { line: 0, col: 0 },
            options,
        ));
        assert_eq!(result?.map(|found| found.items.len()), expected, "{text:?}");
    }
    assert_eq!(*seen.borrow(), "seen;seen;seen;");
    Ok(())
}

fn assert_slash_commands() -> Result<(), maestro_tui::autocomplete::CompletionError> {
    let ready: ArgumentCompletions = Rc::new(|prefix| {
        let prefix = prefix.to_owned();
        Box::pin(async move { Ok(Some(vec![item(&format!("{prefix}one"))])) })
    });
    let suspended: ArgumentCompletions = Rc::new(|_| {
        Box::pin(async {
            YieldOnce(false).await;
            Ok(None)
        })
    });
    let command = |name: &str, completions| SlashCommand {
        name: name.into(),
        description: None,
        argument_hint: None,
        get_argument_completions: completions,
    };
    let commands = [
        command("ready", Some(ready)),
        command("later", Some(suspended)),
        command("plain", None),
    ];
    let outcomes = commands
        .iter()
        .map(|each| {
            each.get_argument_completions
                .as_ref()
                .map(|complete| block_on(complete("a")).map(|found| found.map(|found| found.len())))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(outcomes, [Some(Some(1)), Some(None), None]);
    Ok(())
}

#[test]
fn completion_contracts_keep_signals_results_and_byte_cursors()
-> Result<(), maestro_tui::autocomplete::CompletionError> {
    assert_command_provider()?;
    assert_counting_provider()?;
    assert_slash_commands()?;
    Ok(())
}

/// A log shared with a callback.
type Log = Rc<RefCell<Vec<String>>>;

/// Wires submit, change and border callbacks into shared logs.
fn wire_callbacks(editor: &Rich) -> (Log, Log) {
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let changed = Rc::new(RefCell::new(Vec::new()));
    let (submit_log, change_log) = (Rc::clone(&submitted), Rc::clone(&changed));
    editor.set_on_submit(Some(Rc::new(move |text| {
        submit_log.borrow_mut().push(text.to_owned());
    })));
    editor.set_on_change(Some(Rc::new(move |text| {
        change_log.borrow_mut().push(text.to_owned());
    })));
    editor.set_border_color(Rc::new(|text| format!("\x1b[34m{text}{RESET}")));
    (submitted, changed)
}

fn assert_rich_editor() {
    let rich = Rich::new();
    assert!(
        rich.on_submit().is_none() && rich.on_change().is_none() && rich.border_color().is_none()
    );
    let (submitted, changed) = wire_callbacks(&rich);
    if let Some(input) = rich.input_handler() {
        input.handle_input("hi");
        input.handle_input("\r");
    }
    assert_eq!(
        (submitted.take(), changed.take()),
        (vec!["hi".to_owned()], vec!["hi".to_owned()])
    );
    rich.set_on_change(None);
    rich.handle_input("!");
    assert_eq!(
        changed.take(),
        Vec::<String>::new(),
        "a replaced callback no longer runs"
    );
    assert_eq!(
        rich.render(10),
        [format!("\x1b[34m-{RESET}"), "hi!".to_owned()]
    );

    rich.set_text("see [paste]");
    assert_eq!(rich.get_text(), "see [paste]");
    assert_eq!(rich.get_expanded_text().as_deref(), Some("see pasted text"));
    assert_eq!(rich.add_to_history("older"), Some(()));
    assert_eq!(rich.insert_text_at_cursor("!"), Some(()));
    assert_eq!(
        (
            rich.set_padding_x(2.0),
            rich.set_autocomplete_max_visible(7.0)
        ),
        (Some(()), Some(()))
    );
    assert_eq!(
        rich.set_autocomplete_provider(Rc::new(CommandProvider)),
        Some(())
    );
    assert_eq!(*rich.history.borrow(), ["older"]);
    assert_eq!(
        (
            rich.padding.get().to_bits(),
            rich.visible.get().to_bits(),
            rich.provider.borrow().is_some()
        ),
        (2.0_f64.to_bits(), 7.0_f64.to_bits(), true)
    );
}

fn assert_bare_editor() {
    let bare = Bare {
        text: RefCell::default(),
        submit: RefCell::default(),
        change: RefCell::default(),
    };
    bare.set_text("plain");
    assert_eq!(bare.get_text(), "plain");
    assert_eq!(
        bare.get_expanded_text().unwrap_or_else(|| bare.get_text()),
        "plain",
        "callers fall back to the plain text"
    );
    assert_eq!(bare.add_to_history("x"), None);
    assert_eq!(bare.insert_text_at_cursor("x"), None);
    assert_eq!(
        (
            bare.set_padding_x(1.0),
            bare.set_autocomplete_max_visible(3.0)
        ),
        (None, None)
    );
    assert_eq!(
        bare.set_autocomplete_provider(Rc::new(CountingProvider)),
        None
    );
    assert!(bare.border_color().is_none());
    assert_eq!(bare.set_border_color(Rc::new(str::to_owned)), None);
    bare.handle_input("!");
    assert_eq!(bare.render(5), ["plain!"]);
}

/// What the callbacks of [`editor_reentered_by`] record: the lines `render` returned.
type Rendered = Rc<RefCell<Vec<Vec<String>>>>;

/// Types `typed` into the editor behind `weak` unless it is empty, then records what its
/// `render` returns.
fn reenter(weak: &Weak<Rich>, typed: &str, seen: &Rendered) {
    let editor = weak.upgrade();
    if let Some(editor) = editor {
        if !typed.is_empty() {
            editor.handle_input(typed);
        }
        seen.borrow_mut().push(editor.render(10));
    }
}

/// A border painter that re-enters its editor with `x` the first time it paints.
fn painter_reentering(weak: Weak<Rich>, seen: Rendered) -> BorderColor {
    let entered = Cell::new(false);
    Rc::new(move |text| {
        if !entered.replace(true) {
            reenter(&weak, "x", &seen);
        }
        format!("[{text}]")
    })
}

/// An editor whose border painter (`trigger` is `"paint"`), submit callback (`"submit"`) or
/// change callback (`"change"`) re-enters the editor itself.
fn editor_reentered_by(trigger: &str, seen: &Rendered) -> Rc<Rich> {
    Rc::new_cyclic(|weak: &Weak<Rich>| {
        let rich = Rich::new();
        let (weak, seen) = (weak.clone(), Rc::clone(seen));
        match trigger {
            "paint" => drop(rich.border.replace(Some(painter_reentering(weak, seen)))),
            "submit" => drop(
                rich.submit
                    .replace(Some(Rc::new(move |_| reenter(&weak, "", &seen)))),
            ),
            _ => drop(
                rich.change
                    .replace(Some(Rc::new(move |_| reenter(&weak, "", &seen)))),
            ),
        }
        rich
    })
}

/// A callback that types into or renders its own editor sees the text and the border at that
/// moment, whichever callback it is.
fn assert_editor_callbacks_may_reenter_their_editor() {
    let seen = Rendered::default();
    let painted = editor_reentered_by("paint", &seen);
    assert_eq!(painted.render(10), ["[-]", "x"]);
    assert_eq!(seen.take(), [["[-]", "x"]], "painter");

    let submitting = editor_reentered_by("submit", &seen);
    submitting.handle_input("hi");
    assert!(seen.borrow().is_empty());
    submitting.handle_input("\r");
    assert_eq!(seen.take(), [["-", "hi"]], "submit");

    let changing = editor_reentered_by("change", &seen);
    changing.handle_input("hi");
    assert_eq!(seen.take(), [["-", "hi"]], "change");
}

#[test]
fn editor_contracts_keep_required_input_and_optional_hooks() {
    assert_rich_editor();
    assert_bare_editor();
    assert_editor_callbacks_may_reenter_their_editor();
}

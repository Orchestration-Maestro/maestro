//! Rendered output of the box, text and spacer widgets.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Box, Component, Spacer, Text};
use serde::Deserialize;

use crate::fixtures;
use fixtures::widget_probes::{Calls, background, live_background, tinted};

/// One recorded text rendering: the widget it builds, the width it renders at and the rows and
/// background calls it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Behaviour the case belongs to.
    test: String,
    /// How the widget is built and rendered.
    input: CaseInput,
    /// What rendering must produce.
    expected: CaseOutput,
}

/// How a recorded case builds its text widget and the width it renders at.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseInput {
    /// Text of the widget.
    text: String,
    /// Viewport width to render at.
    width: usize,
    /// Horizontal padding of the widget.
    padding_x: usize,
    /// Vertical padding of the widget.
    padding_y: usize,
    /// SGR background colour of the callback, or no callback.
    style: Option<u8>,
}

/// The rows and background calls a recorded case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseOutput {
    /// Rows the render returns.
    lines: Vec<String>,
    /// Rows the background function receives, in call order.
    calls: Vec<String>,
}

/// Renders every recorded case of one behaviour and returns how many there were.
fn check_cases(test: &str) -> Result<usize, serde_json::Error> {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/widget_cases.json"))?;
    let mut checked = 0;
    for case in cases.iter().filter(|case| case.test == test) {
        let CaseInput {
            text,
            width,
            padding_x,
            padding_y,
            style,
        } = &case.input;
        let calls = Calls::default();
        let background = style.map(|color| background("", color, &calls));
        let widget = Text::new(text.clone(), *padding_x, *padding_y, background);
        assert_eq!(
            widget.render(*width),
            case.expected.lines,
            "rows of {:?} at width {width}",
            case.input.text
        );
        assert_eq!(
            *calls.borrow(),
            case.expected.calls,
            "background calls for {:?} at width {width}",
            case.input.text
        );
        checked += 1;
    }
    Ok(checked)
}

#[test]
fn spacer_count_changes_without_width_or_cache() {
    let spacer = Spacer::default();
    for width in [0, 1, 80] {
        assert_eq!(spacer.render(width), [""], "one blank row at width {width}");
    }

    spacer.set_lines(0);
    assert!(spacer.render(80).is_empty());

    spacer.set_lines(3);
    spacer.invalidate();
    for width in [0, 80] {
        assert_eq!(spacer.render(width), ["", "", ""]);
    }
    assert_eq!(Spacer::new(2).render(5), ["", ""]);
}

#[test]
fn text_set_text_replaces_content_and_can_hide_it() {
    let text = Text::new("old".into(), 1, 0, None);
    assert_eq!(text.render(6), [" old  "]);
    text.set_text("new".into());
    assert_eq!(text.render(6), [" new  "]);
    text.set_text(" \u{feff} ".into());
    assert!(text.render(6).is_empty());
}

#[test]
fn text_blankness_uses_terminal_whitespace() -> Result<(), serde_json::Error> {
    assert_eq!(check_cases("text_blankness_uses_terminal_whitespace")?, 30);
    Ok(())
}

#[test]
fn text_wraps_tabs_lines_graphemes_and_escapes() -> Result<(), serde_json::Error> {
    assert_eq!(
        check_cases("text_wraps_tabs_lines_graphemes_and_escapes")?,
        12
    );
    Ok(())
}

#[test]
fn text_padding_styles_content_before_reused_vertical_rows() -> Result<(), serde_json::Error> {
    assert_eq!(
        check_cases("text_padding_styles_content_before_reused_vertical_rows")?,
        3
    );
    Ok(())
}

#[test]
fn text_narrow_viewports_keep_whole_graphemes() -> Result<(), serde_json::Error> {
    assert_eq!(
        check_cases("text_narrow_viewports_keep_whole_graphemes")?,
        20
    );
    Ok(())
}

/// A release notice whose version number carries an inverse blank: a styled blank row keeps its
/// cell through padding and a width change, and the rows are rebuilt when the text is replaced.
#[test]
fn text_retains_visible_styled_blank_rows() {
    let dark = "\u{1b}[1m\u{1b}[38;2;255;255;0mUpdate Available\u{1b}[39m\u{1b}[22m\n\
                \u{1b}[38;2;128;128;128mNew version \u{1b}[7m \u{1b}[27m is available. Run \
                \u{1b}[39m\u{1b}[38;2;138;190;183mmaestro update\u{1b}[39m";
    let light = "\u{1b}[1m\u{1b}[38;2;154;115;38mUpdate Available\u{1b}[39m\u{1b}[22m\n\
                 \u{1b}[38;2;108;108;108mNew version \u{1b}[7m \u{1b}[27m is available. Run \
                 \u{1b}[39m\u{1b}[38;2;90;128;128mmaestro update\u{1b}[39m";
    let dark_rows = [
        " \u{1b}[1m\u{1b}[38;2;255;255;0mUpdate       ",
        " \u{1b}[1;38;2;255;255;0mAvailable\u{1b}[39m\u{1b}[22m    ",
        " \u{1b}[38;2;128;128;128mNew version  ",
        " \u{1b}[38;2;128;128;128m \u{1b}[7m \u{1b}[27m is        ",
        " \u{1b}[38;2;128;128;128mavailable.   ",
        " \u{1b}[38;2;128;128;128mRun \u{1b}[39m\u{1b}[38;2;138;190;183mmaestro  ",
        " \u{1b}[38;2;138;190;183mupdate\u{1b}[39m       ",
    ];
    let text = Text::new(dark.to_owned(), 1, 0, None);
    assert_eq!(text.render(14), dark_rows);

    let light_rows = [
        " \u{1b}[1m\u{1b}[38;2;154;115;38mUpdate       ",
        " \u{1b}[1;38;2;154;115;38mAvailable\u{1b}[39m\u{1b}[22m    ",
        " \u{1b}[38;2;108;108;108mNew version  ",
        " \u{1b}[38;2;108;108;108m \u{1b}[7m \u{1b}[27m is        ",
        " \u{1b}[38;2;108;108;108mavailable.   ",
        " \u{1b}[38;2;108;108;108mRun \u{1b}[39m\u{1b}[38;2;90;128;128mmaestro  ",
        " \u{1b}[38;2;90;128;128mupdate\u{1b}[39m       ",
    ];
    let narrow: Vec<&str> = vec![
        " \u{1b}[1m\u{1b}[38;2;255;255;0mUpdate ",
        " \u{1b}[1;38;2;255;255;0mAvaila ",
        " \u{1b}[1;38;2;255;255;0mble\u{1b}[39m\u{1b}[22m    ",
        " \u{1b}[38;2;128;128;128mNew    ",
        " \u{1b}[38;2;128;128;128mversio ",
        " \u{1b}[38;2;128;128;128mn \u{1b}[7m \u{1b}[27m is ",
        " \u{1b}[38;2;128;128;128mavaila ",
        " \u{1b}[38;2;128;128;128mble.   ",
        " \u{1b}[38;2;128;128;128mRun    ",
        " \u{1b}[38;2;128;128;128m\u{1b}[39m\u{1b}[38;2;138;190;183mmaestr ",
        " \u{1b}[38;2;138;190;183mo      ",
        " \u{1b}[38;2;138;190;183mupdate\u{1b}[39m ",
    ];
    assert_eq!(text.render(8), narrow, "a width change re-wraps the blanks");

    text.set_text(light.to_owned());
    assert_eq!(text.render(14), light_rows, "prepared styling is replaced");
    text.invalidate();
    assert_eq!(text.render(14), light_rows, "a repaint keeps the same rows");
}

/// A child that records the widths it renders at and shows `x` only when it has a cell.
struct Letter(RefCell<Vec<usize>>);

impl Component for Letter {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.borrow_mut().push(width);
        vec!["x".repeat(width.min(1))]
    }
}

#[test]
fn box_padding_respects_the_child_viewport() {
    let boundaries: [(usize, usize, usize, &[&str], usize); 5] = [
        (0, 0, 0, &[""], 0),
        (0, 0, 1, &["x"], 1),
        (1, 0, 1, &["x"], 1),
        (1, 0, 2, &["  "], 0),
        (2, 1, 3, &["   ", " x ", "   "], 1),
    ];
    for (padding_x, padding_y, width, rows, child_width) in boundaries {
        let letter = Rc::new(Letter(RefCell::default()));
        let container = Box::new(padding_x, padding_y, None);
        container.add_child(Rc::clone(&letter) as ComponentHandle);
        assert_eq!(
            container.render(width),
            rows,
            "rows at width {width} with padding {padding_x}x{padding_y}"
        );
        assert_eq!(*letter.0.borrow(), [child_width], "child width at {width}");
    }
}

#[test]
fn box_empty_children_skip_background_and_vertical_padding() {
    let calls = Calls::default();
    let container = Box::new(1, 2, Some(background("", 41, &calls)));
    assert!(container.render(8).is_empty(), "no children");

    container.add_child(Rc::new(Text::new("\t \n".into(), 0, 0, None)));
    assert!(container.render(8).is_empty(), "a child without rows");
    assert!(
        calls.borrow().is_empty(),
        "nothing to paint, so no sample either"
    );

    container.add_child(Rc::new(Spacer::new(1)));
    let blank = "        ";
    assert_eq!(container.render(8), tinted(41, &[blank; 5]));
    let mut painted = vec!["test"];
    painted.extend([blank; 5]);
    assert_eq!(*calls.borrow(), painted, "a blank row is content");
}

/// Rows of a red box padded two columns and one row at width 10, holding `inner` rows
/// styled with background `color`, then a spacer row.
fn layered(color: u8, inner: &[&str]) -> Vec<String> {
    let blank = "\x1b[41m          \x1b[49m".to_owned();
    let mut rows = vec![blank.clone()];
    rows.extend(
        inner
            .iter()
            .map(|row| format!("\x1b[41m  \x1b[{color}m{row}\x1b[49m  \x1b[49m")),
    );
    rows.extend([blank.clone(), blank]);
    rows
}

#[test]
fn nested_widgets_keep_background_layers_and_invalidation() {
    let inner_color = Rc::new(Cell::new(44));
    let inner_style = live_background("", &inner_color, &Calls::default());
    let outer = Box::new(2, 1, Some(background("", 41, &Calls::default())));
    let text = Rc::new(Text::new("hi\t\n界".into(), 0, 0, Some(inner_style)));
    outer.add_child(Rc::clone(&text) as ComponentHandle);
    outer.add_child(Rc::new(Spacer::new(1)));

    assert_eq!(outer.render(10), layered(44, &["hi    ", "界    "]));

    inner_color.set(42);
    assert_eq!(
        outer.render(10),
        layered(44, &["hi    ", "界    "]),
        "the inner text keeps its cached rows"
    );
    outer.invalidate();
    assert_eq!(outer.render(10), layered(42, &["hi    ", "界    "]));

    inner_color.set(44);
    text.set_text("bye".into());
    assert_eq!(outer.render(10), layered(44, &["bye   "]));
}

#[test]
fn widget_defaults_keep_padding_and_passive_capabilities() {
    let container = Box::default();
    container.add_child(Rc::new(Text::new("X".into(), 0, 0, None)));
    let text = Text::default();
    assert!(
        text.render(6).is_empty(),
        "default text is empty and hidden"
    );
    text.set_text("X".into());
    let padded = ["      ", " X    ", "      "];
    assert_eq!(container.render(6), padded);
    assert_eq!(text.render(6), padded);
    assert_eq!(Spacer::default().render(6), [""]);

    let widgets: [&dyn Component; 3] = [&container, &text, &Spacer::default()];
    for widget in widgets {
        assert!(widget.input_handler().is_none() && widget.focusable().is_none());
        assert!(!widget.wants_key_release());
    }
}

#[test]
fn text_paints_consecutive_blank_continuations() {
    for (on, off) in [("\x1b[44m", "\x1b[49m"), ("\x1b[7m", "\x1b[27m")] {
        let text = Text::new(format!("- {on}   {off}"), 0, 0, None);
        assert_eq!(
            text.render(1),
            [
                "-".to_owned(),
                format!(" {on}"),
                format!("{on} "),
                format!("{on} "),
                format!("{on} {off}")
            ]
        );
    }
    let text = Text::new("- \x1b[4m   \x1b[24m".to_owned(), 0, 0, None);
    assert_eq!(
        text.render(1),
        [
            "-",
            "\x1b[4m \x1b[24m",
            "\x1b[4m \x1b[24m",
            "\x1b[4m \x1b[24m"
        ]
    );
}

#[test]
fn text_keeps_filtered_authored_lines_and_metadata() {
    for marker in ["", "\x1b]133;A\x07", maestro_tui::CURSOR_MARKER] {
        let text = Text::new(format!("a\n\x1b[31m{marker}  \x1b[39m\nb"), 0, 0, None);
        assert_eq!(
            text.render(1),
            ["a".to_owned(), format!("{marker} "), "b".to_owned()]
        );
    }
}

#[test]
fn text_keeps_metadata_from_discarded_continuations() {
    for marker in [maestro_tui::CURSOR_MARKER, "\x1b]133;A\x07"] {
        let text = Text::new(format!("- \x1b[4m{marker} \x1b[24m"), 0, 0, None);
        let underlined = text.render(1);
        text.set_text(format!("\t{marker}a"));
        assert_eq!(
            (underlined, text.render(1)),
            (
                vec!["-".to_owned(), format!("\x1b[4m{marker} \x1b[24m")],
                vec![format!("{marker}a")]
            )
        );
    }
}

/// Asserts the rows `Text` renders for each case, naming every input that differs.
fn assert_renders(cases: &[crate::ansi_wrapping::BoundaryCase]) {
    let actual: Vec<_> = cases
        .iter()
        .map(|case| {
            let text = Text::new(case.text.clone(), 0, 0, None);
            (case.text.as_str(), text.render(case.width))
        })
        .collect();
    let expected: Vec<_> = cases
        .iter()
        .map(|case| (case.text.as_str(), case.rendered.clone()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn text_pads_after_the_authored_boundary_reset() {
    assert_renders(&crate::ansi_wrapping::boundary_cases());
}

#[test]
fn text_paints_no_row_of_inherited_style_alone() {
    assert_renders(&crate::ansi_wrapping::inherited_style_cases());
}

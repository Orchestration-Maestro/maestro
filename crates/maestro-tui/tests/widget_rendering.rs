//! Rendered output of the box, text and spacer widgets.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Box, Component, Spacer, Text};
use serde::Deserialize;

mod fixtures {
    pub mod widget_probes;
}
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

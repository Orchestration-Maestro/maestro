//! Quote width saturation through the public component interface.
use crate::support::markdown_cases as markdown;
use maestro_tui::{Component, Markdown, MarkdownOptions, TerminalImage};
use std::{cell::Cell, rc::Rc};

#[test]
fn markdown_quote_width_floor_does_not_emit_border() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut theme = markdown::plain();
    theme.quote_border = Box::new(move |text| {
        observed.set(observed.get() + 1);
        text.to_owned()
    });
    let component = Markdown::new(
        "> x".to_owned(),
        MarkdownOptions {
            clean_content: false,
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(theme),
        TerminalImage::new(|_| None, || 1),
    );
    let rows = component.render(2);
    assert_eq!(calls.get(), 0, "an unfit border callback ran");
    assert_eq!(rows, ["x "]);
    assert_eq!(component.render(3), ["│ x"]);
    assert_eq!(calls.get(), 1, "the fitting border callback must run once");
}

#[test]
fn markdown_quote_saturation_keeps_fitting_borders() {
    let component = Markdown::new(
        format!("{} x", ">".repeat(48)),
        MarkdownOptions {
            clean_content: false,
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        TerminalImage::new(|_| None, || 1),
    );
    let rows = component.render(80);
    assert_eq!(rows.len(), 1, "generated borders amplified the content");
    assert_eq!(rows, [format!("{}x ", "│ ".repeat(39))]);
    markdown::cases("markdown_quote_saturation_keeps_fitting_borders");
}

#[test]
fn markdown_quote_fitting_layout_keeps_bytes() {
    for case in markdown::corpus("markdown_quote_fitting_layout_keeps_bytes") {
        assert_eq!(markdown::component(&case).render(case.width), case.lines);
    }
}

#[test]
fn markdown_quote_deep_resize_keeps_content() {
    let cases = markdown::corpus("markdown_quote_deep_resize_keeps_content");
    let (groups, remainder) = cases.as_chunks::<3>();
    assert!(remainder.is_empty());
    for widths in groups {
        let component = markdown::component(&widths[0]);
        for case in widths {
            assert_eq!(component.render(case.width), case.lines);
        }
        component.set_text("x".to_owned());
        assert_eq!(component.render(2), ["x "]);
    }
}

#[test]
fn markdown_quote_reply_rows_follow_input() {
    for case in markdown::corpus("markdown_quote_reply_rows_follow_input") {
        let rows = markdown::component(&case).render(case.width);
        assert_eq!(rows, case.lines);
        assert!(rows.len() <= case.text.chars().count());
    }
}

#[test]
fn markdown_quote_saturated_content_keeps_styles() {
    markdown::cases("markdown_quote_saturated_content_keeps_styles");
}

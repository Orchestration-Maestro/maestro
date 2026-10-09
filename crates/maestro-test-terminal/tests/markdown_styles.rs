//! Markdown callbacks, style boundaries and retained state.
#[path = "support/markdown_cases.rs"]
mod markdown;

#[test]
fn markdown_inline_styles_restore_parent_and_strip_terminal_prefix() {
    markdown::cases("markdown_inline_styles_restore_parent_and_strip_terminal_prefix");
}

#[test]
fn markdown_padding_background_stays_outside_text_style() {
    markdown::cases("markdown_padding_background_stays_outside_text_style");
}

#[test]
fn markdown_image_escape_rows_skip_wrap_padding_and_background() {
    use maestro_tui::{Component, DefaultTextStyle, Markdown, MarkdownOptions, TerminalImage};
    for escape in ["\x1b_Ga=T;AAAA\x1b\\", "\x1b]1337;File=inline=1:AAAA\x07"] {
        let component = Markdown::new(
            escape.to_owned(),
            MarkdownOptions {
                padding_x: 2,
                padding_y: 1,
                default_text_style: Some(DefaultTextStyle {
                    bg_color: Some(Box::new(|s| format!("\x1b[44m{s}\x1b[49m"))),
                    ..DefaultTextStyle::default()
                }),
            },
            std::rc::Rc::new(markdown::ansi()),
            TerminalImage::new(|_| None, || 1),
        );
        assert_eq!(
            component.render(4),
            ["\x1b[44m    \x1b[49m", escape, "\x1b[44m    \x1b[49m"]
        );
    }
}

use maestro_tui::{
    Component, DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, TerminalImage,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Constructs a component with the supplied callbacks through one shared caller.
fn component(text: &str, theme: MarkdownTheme, style: Option<DefaultTextStyle>) -> Markdown {
    Markdown::new(
        text.to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: style,
        },
        Rc::new(theme),
        TerminalImage::new(|_| None, || 1),
    )
}

/// One recorded highlighting invocation.
#[derive(serde::Deserialize)]
struct HighlightCase {
    /// Authored code block.
    text: String,
    /// Expected complete code argument.
    code: String,
    /// Expected optional information argument.
    info: Option<String>,
    /// Expected finalized rows.
    lines: Vec<String>,
}

#[test]
fn markdown_highlighter_receives_exact_code_and_language() {
    let cases: Vec<HighlightCase> =
        serde_json::from_str(include_str!("fixtures/markdown_highlight.json")).unwrap();
    for case in cases {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&calls);
        let mut theme = markdown::plain();
        theme.highlight_code = Some(Box::new(move |code, info| {
            observed
                .borrow_mut()
                .push((code.to_owned(), info.map(str::to_owned)));
            vec!["CUSTOM".to_owned(), "line2".to_owned()]
        }));
        let rows = component(&case.text, theme, None).render(30);
        assert_eq!(*calls.borrow(), [(case.code, case.info)]);
        assert_eq!(rows, case.lines);
    }
}

#[test]
fn markdown_cache_invalidates_on_text_width_and_explicit_request() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut theme = markdown::plain();
    theme.highlight_code = Some(Box::new(move |_, _| {
        observed.set(observed.get() + 1);
        vec!["fresh".to_owned()]
    }));
    let component = component("```\nx\n```", theme, None);
    let expected = [
        "```                 ",
        "  fresh             ",
        "```                 ",
    ];
    assert_eq!(component.render(20), expected);
    assert_eq!(calls.get(), 1);
    assert_eq!(component.render(20), expected);
    assert_eq!(calls.get(), 1);
    component.invalidate();
    assert_eq!(component.render(20), expected);
    assert_eq!(calls.get(), 2);
    component.set_text("```\nx\n```".to_owned());
    assert_eq!(component.render(20), expected);
    assert_eq!(calls.get(), 3);
    assert_eq!(
        component.render(21),
        [
            "```                  ",
            "  fresh              ",
            "```                  "
        ]
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn markdown_definition_only_render_is_cache_consistent() {
    let component = component("[x]: /u", markdown::plain(), None);
    assert_eq!(component.render(20), [""]);
    assert_eq!(component.render(20), [""]);
}

#[test]
fn markdown_reentrant_set_text_does_not_cache_stale_rows() {
    let handle = Rc::new(RefCell::new(Weak::<Markdown>::new()));
    let callback_handle = Rc::clone(&handle);
    let entered = Cell::new(false);
    let component = Rc::new(component(
        "old",
        markdown::plain(),
        Some(DefaultTextStyle {
            color: Some(Box::new(move |text| {
                if !entered.replace(true) {
                    callback_handle
                        .borrow()
                        .upgrade()
                        .unwrap()
                        .set_text("new".to_owned());
                }
                text.to_owned()
            })),
            ..DefaultTextStyle::default()
        }),
    ));
    *handle.borrow_mut() = Rc::downgrade(&component);
    assert_eq!(component.render(20), ["old                 "]);
    assert_eq!(component.render(20), ["new                 "]);
    drop(component);
    assert!(handle.borrow().upgrade().is_none());
}

#[test]
fn markdown_callback_failure_propagates_without_fallback() {
    let failure = std::sync::Arc::new("caller failure");
    let thrown = std::sync::Arc::clone(&failure);
    let mut theme = markdown::plain();
    theme.highlight_code = Some(Box::new(move |_, _| {
        std::panic::panic_any(std::sync::Arc::clone(&thrown))
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        component("```\nx\n```", theme, None).render(20)
    }));
    let caught = result
        .unwrap_err()
        .downcast::<std::sync::Arc<&str>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&failure, &caught));
}

#[test]
fn markdown_style_prefix_handles_missing_repeated_and_reset() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/markdown_prefix.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let mode = case["mode"].as_str().unwrap().to_owned();
        let style = DefaultTextStyle {
            color: Some(Box::new(move |s| match mode.as_str() {
                "missing" => s.replace('\0', ""),
                "repeated" => format!("\x1b[90m\x1b[90m{s}\x1b[39m"),
                "reset" => format!("\x1b[90m{s}\x1b[0m"),
                _ => panic!("unexpected fixture mode"),
            })),
            ..DefaultTextStyle::default()
        };
        let rows = component("a **b** `c` ~~de~~\nf", markdown::ansi(), Some(style)).render(40);
        assert_eq!(
            rows,
            serde_json::from_value::<Vec<String>>(case["lines"].clone()).unwrap()
        );
    }
}

#[test]
fn markdown_invalidation_refreshes_live_style_prefix() {
    let code = Rc::new(Cell::new(90));
    let live = Rc::clone(&code);
    let component = component(
        "a `b` c",
        markdown::plain(),
        Some(DefaultTextStyle {
            color: Some(Box::new(move |s| {
                format!("\x1b[{}m{s}\x1b[39m", live.get())
            })),
            ..DefaultTextStyle::default()
        }),
    );
    assert_eq!(
        component.render(20),
        ["\x1b[90ma \x1b[39mb\x1b[90m\x1b[90m c\x1b[39m               "]
    );
    code.set(31);
    component.invalidate();
    assert_eq!(
        component.render(20),
        ["\x1b[31ma \x1b[39mb\x1b[31m\x1b[31m c\x1b[39m               "]
    );
}

#[test]
fn markdown_capabilities_are_observed_on_cache_miss() {
    let terminal = TerminalImage::new(|_| None, || 1);
    let component = Markdown::new(
        "[x](u)".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        terminal.clone(),
    );
    assert_eq!(component.render(20), ["x (u)               "]);
    terminal.set_capabilities(maestro_tui::TerminalCapabilities {
        hyperlinks: true,
        ..maestro_tui::TerminalCapabilities::default()
    });
    assert_eq!(component.render(20), ["x (u)               "]);
    component.invalidate();
    assert_eq!(
        component.render(20),
        ["\x1b]8;;u\x1b\\x\x1b]8;;\x1b\\                   "]
    );
}

#[test]
fn markdown_highlighter_output_is_not_replaced() {
    for (returned, expected) in [
        (
            vec![],
            vec!["```unknown meta         ", "```                     "],
        ),
        (
            vec![""],
            vec![
                "```unknown meta         ",
                "                        ",
                "```                     ",
            ],
        ),
        (
            vec!["CUSTOM"],
            vec![
                "```unknown meta         ",
                "  CUSTOM                ",
                "```                     ",
            ],
        ),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&calls);
        let mut theme = markdown::plain();
        theme.highlight_code = Some(Box::new(move |code, info| {
            observed
                .borrow_mut()
                .push((code.to_owned(), info.map(str::to_owned)));
            returned.iter().map(|s| (*s).to_owned()).collect()
        }));
        assert_eq!(
            component("```unknown meta\nx\n```", theme, None).render(24),
            expected
        );
        assert_eq!(
            *calls.borrow(),
            [("x".to_owned(), Some("unknown meta".to_owned()))]
        );
    }
}

#[test]
fn maestro_markdown_preserve_gray_italic_styling_after_inline_code() {
    markdown::cases("maestro_markdown_preserve_gray_italic_styling_after_inline_code");
}

#[test]
fn maestro_markdown_preserve_gray_italic_styling_after_bold_text() {
    markdown::cases("maestro_markdown_preserve_gray_italic_styling_after_bold_text");
}

#[test]
fn maestro_markdown_apply_consistent_styling_to_all_lines_in_lazy_continuation_blockquote() {
    markdown::cases(
        "maestro_markdown_apply_consistent_styling_to_all_lines_in_lazy_continuation_blockquote",
    );
}

#[test]
fn maestro_markdown_apply_consistent_styling_to_explicit_multiline_blockquote() {
    markdown::cases("maestro_markdown_apply_consistent_styling_to_explicit_multiline_blockquote");
}

#[test]
fn maestro_markdown_properly_indent_wrapped_blockquote_lines_with_styling() {
    markdown::cases("maestro_markdown_properly_indent_wrapped_blockquote_lines_with_styling");
}

#[test]
fn maestro_markdown_render_inline_formatting_inside_blockquotes_and_reapply_quote_styling_after() {
    markdown::cases(
        "maestro_markdown_render_inline_formatting_inside_blockquotes_and_reapply_quote_styling_after",
    );
}

#[test]
fn maestro_markdown_preserve_heading_styling_after_inline_code() {
    markdown::cases("maestro_markdown_preserve_heading_styling_after_inline_code");
}

#[test]
fn maestro_markdown_preserve_heading_styling_after_inline_code_for_h1() {
    markdown::cases("maestro_markdown_preserve_heading_styling_after_inline_code_for_h1");
}

#[test]
fn maestro_markdown_not_leak_h1_underline_into_padding_when_inline_code_is_the_last_token() {
    let terminal = support::virtual_terminal::VirtualTerminal::new(80, 4);
    let runtime = support::manual_runtime::ManualRuntime::new();
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    let widget = Rc::new(component(
        "# Important distinction from `open()`",
        markdown::ansi(),
        None,
    ));
    tui.add_child(widget);
    tui.start().unwrap();
    runtime.settle().unwrap();
    assert_eq!(terminal.viewport()[0], "Important distinction from open()");
    assert!(terminal.is_underlined(0, 0));
    for column in 33..80 {
        assert!(
            !terminal.is_underlined(0, column),
            "padding column {column}"
        );
    }
    tui.stop().unwrap();
}

#[test]
fn maestro_markdown_preserve_heading_styling_after_bold_text() {
    markdown::cases("maestro_markdown_preserve_heading_styling_after_bold_text");
}

#[test]
fn markdown_decorations_apply_in_fixed_order_without_duplicates() {
    use maestro_tui::TextDecoration::{Bold, Italic, Strikethrough, Underline};
    for decorations in [
        vec![Bold, Italic, Strikethrough, Underline],
        vec![Underline, Italic, Bold, Strikethrough, Bold],
    ] {
        assert_eq!(
            component(
                "styled",
                markdown::ansi(),
                Some(DefaultTextStyle {
                    decorations,
                    ..DefaultTextStyle::default()
                })
            )
            .render(10),
            ["\x1b[4m\x1b[9m\x1b[3m\x1b[1mstyled\x1b[22m\x1b[23m\x1b[29m\x1b[24m    "]
        );
    }
}

#[allow(
    dead_code,
    reason = "Screen support is shared by several test targets."
)]
mod support {
    pub mod manual_runtime;
    pub mod recording_terminal;
    pub mod virtual_terminal;
}

/// A Markdown component followed by a plain input row.
struct MarkdownWithInput {
    /// Rendered message.
    markdown: Markdown,
    /// Number of message rows in the completed render.
    count: Cell<usize>,
}
impl Component for MarkdownWithInput {
    fn render(&self, width: usize) -> Vec<String> {
        let mut rows = self.markdown.render(width);
        self.count.set(rows.len());
        rows.push("INPUT".to_owned());
        rows
    }
    fn invalidate(&self) {
        self.markdown.invalidate();
    }
}

#[test]
fn maestro_markdown_not_leak_styles_into_following_lines_when_rendered_in_tui() {
    let terminal = support::virtual_terminal::VirtualTerminal::new(80, 6);
    let runtime = support::manual_runtime::ManualRuntime::new();
    let tui = maestro_tui::TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    let markdown = Markdown::new(
        "This is thinking with `inline code`".to_owned(),
        MarkdownOptions {
            padding_x: 1,
            padding_y: 0,
            default_text_style: Some(DefaultTextStyle {
                color: Some(Box::new(|s| markdown::paint(s, &[(90, 39)]))),
                decorations: vec![maestro_tui::TextDecoration::Italic],
                ..DefaultTextStyle::default()
            }),
        },
        Rc::new(markdown::ansi()),
        TerminalImage::new(|_| None, || 1),
    );
    let widget = Rc::new(MarkdownWithInput {
        markdown,
        count: Cell::new(0),
    });
    tui.add_child(widget.clone());
    tui.start().unwrap();
    runtime.settle().unwrap();
    assert_eq!(widget.count.get(), 1);
    assert_eq!(terminal.viewport()[1], "INPUT");
    assert!(terminal.is_italic(0, 1));
    assert!(!terminal.is_italic(widget.count.get(), 0));
    tui.stop().unwrap();
}

#[test]
fn markdown_background_runs_after_code_and_content_layout() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let highlighted = Rc::clone(&calls);
    let mut theme = markdown::plain();
    theme.highlight_code = Some(Box::new(move |_, _| {
        highlighted.borrow_mut().push("highlight".to_owned());
        vec!["custom".to_owned()]
    }));
    let background = Rc::clone(&calls);
    let widget = Markdown::new(
        "```\nx\n```".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 2,
            default_text_style: Some(DefaultTextStyle {
                bg_color: Some(Box::new(move |s| {
                    background.borrow_mut().push(format!("background:{s}"));
                    s.to_owned()
                })),
                ..DefaultTextStyle::default()
            }),
        },
        Rc::new(theme),
        TerminalImage::new(|_| None, || 1),
    );
    assert_eq!(
        widget.render(10),
        [
            "          ",
            "          ",
            "```       ",
            "  custom  ",
            "```       ",
            "          ",
            "          "
        ]
    );
    assert_eq!(
        *calls.borrow(),
        [
            "highlight",
            "background:```       ",
            "background:  custom  ",
            "background:```       ",
            "background:          ",
            "background:          "
        ]
    );
}

#[test]
fn markdown_default_prefix_is_sampled_once_per_layout() {
    let samples = Rc::new(Cell::new(0));
    let observed = Rc::clone(&samples);
    let widget = component(
        "a **b *c* `d`**\n\nnext",
        markdown::plain(),
        Some(DefaultTextStyle {
            color: Some(Box::new(move |s| {
                if s == "\0" {
                    observed.set(observed.get() + 1);
                }
                s.to_owned()
            })),
            ..DefaultTextStyle::default()
        }),
    );
    assert_eq!(
        widget.render(12),
        ["a b c d     ", "            ", "next        "]
    );
    assert_eq!(samples.get(), 1);
    assert_eq!(
        widget.render(12),
        ["a b c d     ", "            ", "next        "]
    );
    assert_eq!(samples.get(), 1);
    widget.invalidate();
    assert_eq!(
        widget.render(12),
        ["a b c d     ", "            ", "next        "]
    );
    assert_eq!(samples.get(), 2);
}

#[test]
fn markdown_quote_prefix_is_shared_across_paragraphs_and_items() {
    let samples = Rc::new(Cell::new(0));
    let observed = Rc::clone(&samples);
    let mut theme = markdown::plain();
    theme.quote = Box::new(move |s| {
        if s == "\0" {
            observed.set(observed.get() + 1);
        }
        s.to_owned()
    });
    let widget = component("> first\n>\n> second\n> - a\n> - b", theme, None);
    assert_eq!(
        widget.render(12),
        [
            "│ first     ",
            "│           ",
            "│ second    ",
            "│ - a       ",
            "│ - b       "
        ]
    );
    assert_eq!(samples.get(), 1);
}

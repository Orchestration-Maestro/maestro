//! Markdown layout through the public component interface.
#[path = "support/markdown_cases.rs"]
mod markdown;
use maestro_tui::{Component, DefaultTextStyle, Markdown, MarkdownOptions, TerminalImage};
use std::rc::Rc;

#[test]
fn markdown_empty_and_whitespace_use_ecmascript_set() {
    for text in ["", " \t\r\n", "\u{feff}", "\u{a0}", "\u{2028}", "\u{2029}"] {
        let component = Markdown::new(
            text.to_owned(),
            MarkdownOptions {
                padding_x: 2,
                padding_y: 1,
                default_text_style: Some(DefaultTextStyle::default()),
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        );
        assert!(component.render(9).is_empty(), "{text:?}");
    }
    let component = Markdown::new(
        "\u{85}".to_owned(),
        MarkdownOptions {
            padding_x: 2,
            padding_y: 1,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        TerminalImage::new(|_| None, || 1),
    );
    assert_eq!(
        component.render(9),
        ["         ", "  \u{85}       ", "         "]
    );
}

#[test]
fn markdown_padding_and_tabs_preserve_content() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/markdown_padding.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let component = Markdown::new(
            row["text"].as_str().unwrap().to_owned(),
            MarkdownOptions {
                padding_x: row["padding_x"].as_u64().unwrap().try_into().unwrap(),
                padding_y: row["padding_y"].as_u64().unwrap().try_into().unwrap(),
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        );
        let expected: Vec<String> = serde_json::from_value(row["lines"].clone()).unwrap();
        assert_eq!(
            component.render(row["width"].as_u64().unwrap().try_into().unwrap()),
            expected,
            "{}",
            row["text"]
        );
    }
}

#[test]
fn markdown_heading_levels_and_setext_keep_prefix_policy() {
    markdown::cases("markdown_heading_levels_and_setext_keep_prefix_policy");
}

#[test]
fn markdown_block_spacing_uses_next_block_and_explicit_gaps() {
    markdown::cases("markdown_block_spacing_uses_next_block_and_explicit_gaps");
}

#[test]
fn markdown_code_preserves_full_info_and_indentation() {
    markdown::cases("markdown_code_preserves_full_info_and_indentation");
}

#[test]
fn markdown_code_indent_is_exact_and_optional() {
    markdown::cases("markdown_code_indent_is_exact_and_optional");
}

#[test]
fn markdown_rule_width_caps_at_eighty() {
    markdown::cases("markdown_rule_width_caps_at_eighty");
}

#[test]
fn markdown_html_is_literal_with_block_trim() {
    markdown::cases("markdown_html_is_literal_with_block_trim");
}

#[test]
fn markdown_images_keep_authored_alt_markup() {
    markdown::cases("markdown_images_keep_authored_alt_markup");
}

#[test]
fn markdown_strike_uses_native_delimiters() {
    markdown::cases("markdown_strike_uses_native_delimiters");
}

#[test]
fn markdown_link_labels_and_targets_preserve_spelling() {
    markdown::cases("markdown_link_labels_and_targets_preserve_spelling");
}

#[test]
fn markdown_link_capability_selects_osc8_for_all_kinds() {
    markdown::cases("markdown_link_capability_selects_osc8_for_all_kinds");
}

#[test]
fn markdown_bare_link_recognition_preserves_punctuation() {
    markdown::cases("markdown_bare_link_recognition_preserves_punctuation");
}

#[test]
fn markdown_nested_links_follow_commonmark() {
    markdown::cases("markdown_nested_links_follow_commonmark");
}

#[test]
fn markdown_reference_keys_use_commonmark_casefold() {
    markdown::cases("markdown_reference_keys_use_commonmark_casefold");
}

#[test]
fn markdown_ordered_list_interruption_and_starts() {
    markdown::cases("markdown_ordered_list_interruption_and_starts");
}

#[test]
fn markdown_list_loose_empty_and_lazy_continuations() {
    markdown::cases("markdown_list_loose_empty_and_lazy_continuations");
}

#[test]
fn markdown_nested_list_indentation_is_theme_independent() {
    markdown::cases("markdown_nested_list_indentation_is_theme_independent");
    for theme in [markdown::plain(), markdown::ansi()] {
        let terminal = support::virtual_terminal::VirtualTerminal::new(40, 8);
        let runtime = support::manual_runtime::ManualRuntime::new();
        let tui = maestro_tui::TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        let widget = Rc::new(Markdown::new(
            "- parent\n  - child\n    - grandchild".to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(theme),
            TerminalImage::new(|_| None, || 1),
        ));
        tui.add_child(widget);
        tui.start().unwrap();
        runtime.settle().unwrap();
        assert_eq!(
            &terminal.viewport()[..3],
            ["- parent", "  - child", "    - grandchild"]
        );
        tui.stop().unwrap();
    }
}

#[test]
fn markdown_quotes_keep_context_blank_and_recursive_blocks() {
    markdown::cases("markdown_quotes_keep_context_blank_and_recursive_blocks");
}

#[test]
fn markdown_wide_graphemes_wrap_without_splitting() {
    markdown::cases("markdown_wide_graphemes_wrap_without_splitting");
}

#[test]
fn markdown_narrow_lists_preserve_wrapped_rows() {
    markdown::cases("markdown_narrow_lists_preserve_wrapped_rows");
    for (width, last) in [(1, "z"), (2, "zz"), (3, "z"), (5, "xyzz")] {
        let terminal = support::virtual_terminal::VirtualTerminal::new(width, 20);
        let runtime = support::manual_runtime::ManualRuntime::new();
        let tui = maestro_tui::TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        let widget = Rc::new(Markdown::new(
            "- xyzz".to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        ));
        let rows = widget.render(width);
        tui.add_child(widget);
        tui.start().unwrap();
        runtime.settle().unwrap();
        assert_eq!(terminal.viewport()[rows.len() - 1], last);
        tui.stop().unwrap();
    }
}

#[test]
fn markdown_authored_label_ranges_keep_markup() {
    markdown::cases("markdown_authored_label_ranges_keep_markup");
}

#[test]
fn markdown_nested_blocks_preserve_blank_ownership() {
    markdown::cases("markdown_nested_blocks_preserve_blank_ownership");
}

#[test]
fn markdown_native_entity_and_escape_contexts() {
    markdown::cases("markdown_native_entity_and_escape_contexts");
}

#[test]
fn markdown_autolink_scheme_and_domain_boundaries() {
    markdown::cases("markdown_autolink_scheme_and_domain_boundaries");
}

#[test]
fn markdown_autolink_tail_boundaries() {
    markdown::cases("markdown_autolink_tail_boundaries");
}

#[test]
fn markdown_autolinks_respect_inline_framing() {
    markdown::cases("markdown_autolinks_respect_inline_framing");
}

#[test]
fn markdown_extended_autolinks_match_standard_examples() {
    markdown::cases("markdown_extended_autolinks_match_standard_examples");
}

#[test]
fn maestro_markdown_render_simple_nested_list() {
    markdown::cases("maestro_markdown_render_simple_nested_list");
}

#[test]
fn maestro_markdown_render_deeply_nested_list() {
    markdown::cases("maestro_markdown_render_deeply_nested_list");
}

#[test]
fn maestro_markdown_render_ordered_nested_list() {
    markdown::cases("maestro_markdown_render_ordered_nested_list");
}

#[test]
fn maestro_markdown_render_mixed_ordered_and_unordered_nested_lists() {
    markdown::cases("maestro_markdown_render_mixed_ordered_and_unordered_nested_lists");
}

#[test]
fn maestro_markdown_maintain_numbering_when_code_blocks_are_not_indented_llm_output() {
    markdown::cases(
        "maestro_markdown_maintain_numbering_when_code_blocks_are_not_indented_llm_output",
    );
}

#[test]
fn maestro_markdown_have_only_one_blank_line_between_code_block_and_following_paragraph() {
    markdown::cases(
        "maestro_markdown_have_only_one_blank_line_between_code_block_and_following_paragraph",
    );
}

#[test]
fn maestro_markdown_normalize_paragraph_and_code_block_spacing_to_one_blank_line() {
    markdown::cases(
        "maestro_markdown_normalize_paragraph_and_code_block_spacing_to_one_blank_line",
    );
}

#[test]
fn maestro_markdown_not_add_a_trailing_blank_line_when_code_block_is_the_last_rendered_block() {
    markdown::cases(
        "maestro_markdown_not_add_a_trailing_blank_line_when_code_block_is_the_last_rendered_block",
    );
}

#[test]
fn maestro_markdown_have_only_one_blank_line_between_divider_and_following_paragraph() {
    markdown::cases(
        "maestro_markdown_have_only_one_blank_line_between_divider_and_following_paragraph",
    );
}

#[test]
fn maestro_markdown_not_add_a_trailing_blank_line_when_divider_is_the_last_rendered_block() {
    markdown::cases(
        "maestro_markdown_not_add_a_trailing_blank_line_when_divider_is_the_last_rendered_block",
    );
}

#[test]
fn maestro_markdown_have_only_one_blank_line_between_heading_and_following_paragraph() {
    markdown::cases(
        "maestro_markdown_have_only_one_blank_line_between_heading_and_following_paragraph",
    );
}

#[test]
fn maestro_markdown_not_add_a_trailing_blank_line_when_heading_is_the_last_rendered_block() {
    markdown::cases(
        "maestro_markdown_not_add_a_trailing_blank_line_when_heading_is_the_last_rendered_block",
    );
}

#[test]
fn maestro_markdown_have_only_one_blank_line_between_blockquote_and_following_paragraph() {
    markdown::cases(
        "maestro_markdown_have_only_one_blank_line_between_blockquote_and_following_paragraph",
    );
}

#[test]
fn maestro_markdown_not_add_a_trailing_blank_line_when_blockquote_is_the_last_rendered_block() {
    markdown::cases(
        "maestro_markdown_not_add_a_trailing_blank_line_when_blockquote_is_the_last_rendered_block",
    );
}

#[test]
fn maestro_markdown_render_list_content_inside_blockquotes() {
    markdown::cases("maestro_markdown_render_list_content_inside_blockquotes");
}

#[test]
fn maestro_markdown_wrap_long_blockquote_lines_and_add_border_to_each_wrapped_line() {
    markdown::cases(
        "maestro_markdown_wrap_long_blockquote_lines_and_add_border_to_each_wrapped_line",
    );
}

#[test]
fn maestro_markdown_render_text_as_strikethrough() {
    markdown::cases("maestro_markdown_render_text_as_strikethrough");
}

#[test]
fn maestro_markdown_not_duplicate_url_for_autolinked_emails() {
    markdown::cases("maestro_markdown_not_duplicate_url_for_autolinked_emails");
}

#[test]
fn maestro_markdown_not_duplicate_url_for_bare_urls() {
    markdown::cases("maestro_markdown_not_duplicate_url_for_bare_urls");
}

#[test]
fn maestro_markdown_show_url_in_parentheses_when_hyperlinks_are_not_supported() {
    markdown::cases("maestro_markdown_show_url_in_parentheses_when_hyperlinks_are_not_supported");
}

#[test]
fn maestro_markdown_show_mailto_url_in_parentheses_when_hyperlinks_are_not_supported() {
    markdown::cases(
        "maestro_markdown_show_mailto_url_in_parentheses_when_hyperlinks_are_not_supported",
    );
}

#[test]
fn maestro_markdown_emit_osc_8_hyperlink_sequence_when_terminal_supports_hyperlinks() {
    markdown::cases(
        "maestro_markdown_emit_osc_8_hyperlink_sequence_when_terminal_supports_hyperlinks",
    );
}

#[test]
fn maestro_markdown_use_osc_8_for_mailto_links_when_terminal_supports_hyperlinks() {
    markdown::cases(
        "maestro_markdown_use_osc_8_for_mailto_links_when_terminal_supports_hyperlinks",
    );
}

#[test]
fn maestro_markdown_use_osc_8_for_bare_urls_when_terminal_supports_hyperlinks() {
    markdown::cases("maestro_markdown_use_osc_8_for_bare_urls_when_terminal_supports_hyperlinks");
}

#[test]
fn maestro_markdown_render_content_with_html_like_tags_as_text() {
    markdown::cases("maestro_markdown_render_content_with_html_like_tags_as_text");
}

#[test]
fn maestro_markdown_render_html_tags_in_code_blocks_correctly() {
    markdown::cases("maestro_markdown_render_html_tags_in_code_blocks_correctly");
}

#[test]
fn markdown_quotes_keep_explicit_gap_before_lists() {
    let component = Markdown::new(
        "> paragraph\n>\n> - item".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        TerminalImage::new(|_| None, || 1),
    );
    assert_eq!(
        component.render(20),
        [
            "│ paragraph         ",
            "│                   ",
            "│ - item            "
        ]
    );
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

#[test]
fn markdown_http_prefixed_email_locals_use_mailto_hyperlinks() {
    let images = TerminalImage::new(|_| None, || 1);
    images.set_capabilities(maestro_tui::TerminalCapabilities {
        hyperlinks: true,
        ..Default::default()
    });
    let widget = Markdown::new(
        "httpuser@example.com https@example.com".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        images,
    );
    assert_eq!(
        widget.render(40),
        [
            "\x1b]8;;mailto:httpuser@example.com\x1b\\httpuser@example.com\x1b]8;;\x1b\\ \x1b]8;;mailto:https@example.com\x1b\\https@example.com\x1b]8;;\x1b\\  "
        ]
    );
}

#[test]
fn markdown_authored_labels_exclude_container_markers_and_keep_escapes() {
    let rows: Vec<Vec<String>> = [
        r"![\*a](u)",
        r"[\*a](*a)",
        r"- # \*a",
        "> ![a\n> b](u)",
        "> - > a\n>   > b",
        "> - ![a\n>   b](u)",
        "- [ab]\n  ===",
        "  - ![a\n    b](u)",
        "-\n  ![a\n  b](u)",
    ]
    .into_iter()
    .map(|text| {
        Markdown::new(
            text.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        )
        .render(30)
    })
    .collect();
    let expected: Vec<Vec<String>> = [
        vec![r"\*a"],
        vec!["*a (*a)"],
        vec![r"- \*a"],
        vec!["│ a", "│ b"],
        vec!["│ - a", "│ b"],
        vec!["│ - a", "│ b"],
        vec!["- [ab]"],
        vec!["- a", "b"],
        vec!["- a", "b"],
    ]
    .into_iter()
    .map(|lines| lines.into_iter().map(|line| format!("{line:30}")).collect())
    .collect();
    assert_eq!(rows, expected);
}

#[test]
fn markdown_list_html_retains_literal_text_and_parent_style() {
    let rows: Vec<Vec<String>> = [
        "- <div>x</div>",
        "1. <div>x</div>",
        "> - <div>x</div>",
        "- a\n  - <div>x</div>",
    ]
    .into_iter()
    .map(|text| {
        Markdown::new(
            text.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: Some(DefaultTextStyle {
                    color: Some(Box::new(|text| format!("\x1b[32m{text}\x1b[39m"))),
                    ..DefaultTextStyle::default()
                }),
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        )
        .render(30)
    })
    .collect();
    assert_eq!(
        rows,
        vec![
            vec![format!("- \x1b[32m<div>x</div>\x1b[39m{}", " ".repeat(16))],
            vec![format!("1. \x1b[32m<div>x</div>\x1b[39m{}", " ".repeat(15))],
            vec![format!("│ - <div>x</div>{}", " ".repeat(14))],
            vec![
                format!("- \x1b[32ma\x1b[39m{}", " ".repeat(27)),
                format!("  - \x1b[32m<div>x</div>\x1b[39m{}", " ".repeat(14))
            ],
        ]
    );
}

#[test]
fn markdown_www_requires_a_period_after_the_prefix() {
    let terminal = TerminalImage::new(|_| None, || 1);
    let mut capabilities = terminal.get_capabilities();
    capabilities.hyperlinks = true;
    terminal.set_capabilities(capabilities);
    let component = Markdown::new(
        "www.a www.com www.a.com www.com.org http://www.a".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        terminal,
    );
    let expected = format!(
        "www.a www.com {} {} {}",
        maestro_tui::hyperlink("www.a.com", "http://www.a.com"),
        maestro_tui::hyperlink("www.com.org", "http://www.com.org"),
        maestro_tui::hyperlink("http://www.a", "http://www.a")
    );
    assert_eq!(component.render(48), [expected]);
}

#[test]
fn markdown_quote_continuation_autolinks_exclude_container_prefix() {
    let terminal = TerminalImage::new(|_| None, || 1);
    let mut capabilities = terminal.get_capabilities();
    capabilities.hyperlinks = true;
    terminal.set_capabilities(capabilities);
    let component = Markdown::new(
        "> intro\n>https://example.com".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        terminal,
    );
    assert_eq!(
        component.render(21),
        [
            format!("│ intro{}", " ".repeat(14)),
            format!(
                "│ {}",
                maestro_tui::hyperlink("https://example.com", "https://example.com")
            ),
        ]
    );
}

#[test]
fn markdown_list_heading_fallback_retains_authored_bracket_escapes() {
    for text in [r"- # \[x\]", "- \\[x\\]\n  ==="] {
        let component = Markdown::new(
            text.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        );
        assert_eq!(component.render(7), [r"- \[x\]"], "{text}");
    }
}

#[test]
fn markdown_consumed_definitions_do_not_create_blank_rows() {
    for (text, expected) in [
        ("[u]: /target\n[u]", vec!["u (/target)"]),
        ("[u]: /target\n", vec![""]),
    ] {
        let component = Markdown::new(
            text.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            TerminalImage::new(|_| None, || 1),
        );
        assert_eq!(component.render(11), expected, "{text:?}");
        assert_eq!(component.render(11), expected, "cached {text:?}");
    }
}

#[test]
fn markdown_rejected_web_candidates_leave_emails_eligible() {
    for (input, expected) in [
        (
            "www.user@example.com",
            maestro_tui::hyperlink("www.user@example.com", "mailto:www.user@example.com"),
        ),
        (
            "https://localhost/a@example.com",
            format!(
                "https://localhost/{}",
                maestro_tui::hyperlink("a@example.com", "mailto:a@example.com")
            ),
        ),
    ] {
        let terminal = TerminalImage::new(|_| None, || 1);
        let mut capabilities = terminal.get_capabilities();
        capabilities.hyperlinks = true;
        terminal.set_capabilities(capabilities);
        let component = Markdown::new(
            input.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            terminal,
        );
        assert_eq!(component.render(input.len()), [expected], "{input}");
    }
}

#[test]
fn markdown_autolink_suffix_rules_repeat_until_stable() {
    for (input, tail) in [
        ("https://example.com/a.)", ".)"),
        ("https://example.com/a.&unknown;)", ".&unknown;)"),
    ] {
        let terminal = TerminalImage::new(|_| None, || 1);
        let mut capabilities = terminal.get_capabilities();
        capabilities.hyperlinks = true;
        terminal.set_capabilities(capabilities);
        let component = Markdown::new(
            input.to_owned(),
            MarkdownOptions {
                padding_x: 0,
                padding_y: 0,
                default_text_style: None,
            },
            Rc::new(markdown::plain()),
            terminal,
        );
        assert_eq!(
            component.render(input.len()),
            [format!(
                "{}{tail}",
                maestro_tui::hyperlink("https://example.com/a", "https://example.com/a")
            )],
            "{input}"
        );
    }
}

#[test]
fn markdown_duplicate_definitions_are_consumed_without_gaps() {
    let component = Markdown::new(
        "[u]: /a\n[u]: /b\n[u]".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        TerminalImage::new(|_| None, || 1),
    );
    assert_eq!(component.render(6), ["u (/a)"]);
}

#[test]
fn markdown_mixed_escape_units_preserve_email_boundaries() {
    let terminal = TerminalImage::new(|_| None, || 1);
    let mut capabilities = terminal.get_capabilities();
    capabilities.hyperlinks = true;
    terminal.set_capabilities(capabilities);
    let component = Markdown::new(
        r"\*foo@example.com tail".to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        terminal,
    );
    assert_eq!(
        component.render(21),
        [format!(
            "*{} tail",
            maestro_tui::hyperlink("foo@example.com", "mailto:foo@example.com")
        )]
    );
}

#[test]
fn markdown_mixed_escape_units_preserve_web_start() {
    assert_mixed_escape_link(
        r"\(https://example.com",
        &format!(
            "({}",
            maestro_tui::hyperlink("https://example.com", "https://example.com")
        ),
    );
}

#[test]
fn markdown_mixed_escape_units_preserve_web_suffix() {
    assert_mixed_escape_link(
        r"https://example.com/\*a.",
        &format!(
            "{}.",
            maestro_tui::hyperlink(r"https://example.com/\*a", r"https://example.com/\*a")
        ),
    );
}

fn assert_mixed_escape_link(input: &str, expected: &str) {
    let terminal = TerminalImage::new(|_| None, || 1);
    let mut capabilities = terminal.get_capabilities();
    capabilities.hyperlinks = true;
    terminal.set_capabilities(capabilities);
    let component = Markdown::new(
        input.to_owned(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(markdown::plain()),
        terminal,
    );
    let rows = component.render(80);
    assert_eq!(rows.len(), 1, "{input}");
    assert_eq!(rows[0].trim_end(), expected, "{input}");
}

#[test]
fn markdown_autolinks_keep_internal_authored_entities() {
    for target in ["https://example.com/a&lt;b", "https://example.com/a&nbsp;b"] {
        assert_mixed_escape_link(target, &maestro_tui::hyperlink(target, target));
    }
}

#[test]
fn markdown_autolinks_do_not_recognize_decoded_email_separators() {
    for (input, display) in [
        ("a&#64;example.com", "a@example.com"),
        (r"a\@example.com", "a@example.com"),
        ("user&commat;example.com", "user@example.com"),
    ] {
        assert_mixed_escape_link(input, display);
    }
}

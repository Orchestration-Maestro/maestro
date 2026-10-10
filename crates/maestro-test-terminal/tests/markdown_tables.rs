//! Markdown tables through the public component interface and an emulated screen.
#[path = "support/markdown_cases.rs"]
mod markdown;
#[allow(
    dead_code,
    reason = "Screen support is shared by several test targets."
)]
mod support {
    pub mod manual_runtime;
    pub mod recording_terminal;
    pub mod virtual_terminal;
}
use maestro_tui::{TerminalImage, extract_ansi_code};
use std::rc::Rc;

/// The text a terminal shows for one recorded row.
fn shown(row: &str) -> String {
    let mut text = String::new();
    let mut rest = row;
    while let Some(scalar) = rest.chars().next() {
        let length = if let Some(code) = extract_ansi_code(rest, 0) {
            code.length
        } else {
            text.push(scalar);
            scalar.len_utf8()
        };
        rest = &rest[length..];
    }
    text.trim_end().to_owned()
}

/// Asserts the recorded rows, then replays them on an emulated screen.
fn table_cases(name: &str) {
    markdown::cases(name);
    for case in markdown::corpus(name) {
        let terminal =
            support::virtual_terminal::VirtualTerminal::new(case.width, case.lines.len() + 1);
        let runtime = support::manual_runtime::ManualRuntime::new();
        let tui = maestro_tui::TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        tui.add_child(Rc::new(markdown::component(&case)));
        assert!(tui.start().is_ok() && runtime.settle().is_ok());
        let expected: Vec<String> = case.lines.iter().map(|row| shown(row)).collect();
        assert_eq!(
            terminal.viewport()[..case.lines.len()],
            expected,
            "{name}: {:?}",
            case.text
        );
        assert!(tui.stop().is_ok());
    }
}

#[test]
fn maestro_markdown_render_simple_table() {
    table_cases("maestro_markdown_render_simple_table");
}

#[test]
fn maestro_markdown_render_row_dividers_between_data_rows() {
    table_cases("maestro_markdown_render_row_dividers_between_data_rows");
}

#[test]
fn maestro_markdown_keep_column_width_at_least_the_longest_word() {
    table_cases("maestro_markdown_keep_column_width_at_least_the_longest_word");
}

#[test]
fn maestro_markdown_render_table_with_alignment() {
    table_cases("maestro_markdown_render_table_with_alignment");
}

#[test]
fn maestro_markdown_handle_tables_with_varying_column_widths() {
    table_cases("maestro_markdown_handle_tables_with_varying_column_widths");
}

#[test]
fn maestro_markdown_wrap_table_cells_when_table_exceeds_available_width() {
    table_cases("maestro_markdown_wrap_table_cells_when_table_exceeds_available_width");
}

#[test]
fn maestro_markdown_wrap_long_cell_content_to_multiple_lines() {
    table_cases("maestro_markdown_wrap_long_cell_content_to_multiple_lines");
}

#[test]
fn maestro_markdown_wrap_long_unbroken_tokens_inside_table_cells_not_only_at_line_start() {
    table_cases(
        "maestro_markdown_wrap_long_unbroken_tokens_inside_table_cells_not_only_at_line_start",
    );
}

#[test]
fn maestro_markdown_wrap_styled_inline_code_inside_table_cells_without_breaking_borders() {
    table_cases(
        "maestro_markdown_wrap_styled_inline_code_inside_table_cells_without_breaking_borders",
    );
}

#[test]
fn maestro_markdown_handle_extremely_narrow_width_gracefully() {
    table_cases("maestro_markdown_handle_extremely_narrow_width_gracefully");
}

#[test]
fn maestro_markdown_render_table_correctly_when_it_fits_naturally() {
    table_cases("maestro_markdown_render_table_correctly_when_it_fits_naturally");
}

#[test]
fn maestro_markdown_respect_paddingx_when_calculating_table_width() {
    table_cases("maestro_markdown_respect_paddingx_when_calculating_table_width");
}

#[test]
fn maestro_markdown_not_add_a_trailing_blank_line_when_table_is_the_last_rendered_block() {
    table_cases(
        "maestro_markdown_not_add_a_trailing_blank_line_when_table_is_the_last_rendered_block",
    );
}

#[test]
fn maestro_markdown_render_lists_and_tables_together() {
    table_cases("maestro_markdown_render_lists_and_tables_together");
}

#[test]
fn markdown_table_spacing_follows_next_block() {
    markdown::cases("markdown_table_spacing_follows_next_block");
}

#[test]
fn markdown_table_narrow_fallback_keeps_source_rows() {
    markdown::cases("markdown_table_narrow_fallback_keeps_source_rows");
}

#[test]
fn markdown_table_word_minimum_caps_at_thirty_cells() {
    markdown::cases("markdown_table_word_minimum_caps_at_thirty_cells");
}

#[test]
fn markdown_table_word_split_uses_ecmascript_whitespace() {
    markdown::cases("markdown_table_word_split_uses_ecmascript_whitespace");
}

#[test]
fn markdown_table_minimums_shrink_proportionally() {
    markdown::cases("markdown_table_minimums_shrink_proportionally");
}

#[test]
fn markdown_table_growth_fills_available_width() {
    markdown::cases("markdown_table_growth_fills_available_width");
}

#[test]
fn markdown_table_empty_column_counts_one_cell() {
    markdown::cases("markdown_table_empty_column_counts_one_cell");
}

#[test]
fn markdown_table_cells_keep_inline_and_message_styles() {
    markdown::cases("markdown_table_cells_keep_inline_and_message_styles");
}

#[test]
fn markdown_table_rows_follow_header_columns() {
    markdown::cases("markdown_table_rows_follow_header_columns");
}

#[test]
fn markdown_table_wide_glyphs_use_cell_widths() {
    markdown::cases("markdown_table_wide_glyphs_use_cell_widths");
}

#[test]
fn markdown_table_inside_quote_uses_quote_style() {
    markdown::cases("markdown_table_inside_quote_uses_quote_style");
}

#[test]
fn markdown_table_inside_list_item_keeps_source_rows() {
    markdown::cases("markdown_table_inside_list_item_keeps_source_rows");
}

#[test]
fn markdown_table_lazy_lines_follow_commonmark() {
    markdown::cases("markdown_table_lazy_lines_follow_commonmark");
}

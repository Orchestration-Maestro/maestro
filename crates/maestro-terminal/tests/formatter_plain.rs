//! Plain-text output over retained terminal pages.

use maestro_terminal::{
    Page, Terminal,
    formatter::{Options, PageFormatter, TrailingState},
    point::Tag,
};

fn terminal(cols: u16, rows: u16, text: &str) -> Terminal {
    let mut terminal = Terminal::new(cols, rows, 100_000).unwrap();
    maestro_terminal::stream::Stream::new(false).next_slice(
        text.as_bytes(),
        &mut maestro_terminal::stream_terminal::Handler::new(&mut terminal),
    );
    terminal
}

fn page(terminal: &Terminal) -> &Page {
    let pages = &terminal.screens.active().pages;
    pages.page(pages.get_top_left(Tag::Screen).node).unwrap()
}

#[test]
fn page_plain_single_line() {
    let terminal = terminal(80, 24, "hello, world");
    let page = page(&terminal);
    let formatter = PageFormatter::init(page, Options::default());
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello, world");
    assert_eq!(
        state,
        TrailingState {
            rows: usize::from(page.size.rows),
            cells: 68
        }
    );
}

#[test]
fn terminal_formatter_plain_no_selection() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let formatter =
        maestro_terminal::formatter::TerminalFormatter::init(&terminal, Options::default());
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
}

#[test]
fn page_plain_single_line_soft_wrapped_unwrapped() {
    let terminal = terminal(3, 5, "hello!");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: true,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let _state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello!");
}

#[test]
fn page_plain_multiline() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_multiline_rectangle() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 1;
    formatter.end_x = Some(3);
    formatter.rectangle = true;
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "ell\norl");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_multi_blank_lines() {
    let terminal = terminal(80, 24, "hello\r\n\r\n\r\nworld");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello\n\n\nworld");
    assert_eq!(state.rows, usize::from(page.size.rows) - 3);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_trailing_blank_lines() {
    let terminal = terminal(80, 24, "hello\r\nworld\r\n\r\n");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_trailing_whitespace() {
    let terminal = terminal(80, 24, "hello   \r\nworld   ");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_trailing_whitespace_no_trim() {
    let terminal = terminal(80, 24, "hello   \r\nworld  ");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: false,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello   \nworld  ");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 7);
}

#[test]
fn page_plain_with_prior_trailing_state_rows() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.trailing_state = Some(TrailingState { rows: 2, cells: 0 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "\n\nhello");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_with_prior_trailing_state_cells_no_wrapped_line() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.trailing_state = Some(TrailingState { rows: 0, cells: 3 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_with_prior_trailing_state_cells_with_wrap_continuation() {
    let mut terminal = terminal(80, 24, "world");
    let pages = &mut terminal.screens.active_mut().pages;
    let node = pages.get_top_left(Tag::Screen).node;
    pages
        .page_mut(node)
        .unwrap()
        .get_row_mut(0)
        .set_wrap_continuation(true);
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: true,
            trim: true,
            ..Options::default()
        },
    );
    formatter.trailing_state = Some(TrailingState { rows: 0, cells: 3 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "   world");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_soft_wrapped_without_unwrap() {
    let terminal = terminal(10, 24, "hello world test");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello worl\nd test");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 6);
}

#[test]
fn page_plain_soft_wrapped_with_unwrap() {
    let terminal = terminal(10, 24, "hello world test");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: true,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello world test");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 6);
}

#[test]
fn page_plain_soft_wrapped_3_lines_without_unwrap() {
    let terminal = terminal(10, 24, "hello world this is a test");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello worl\nd this is\na test");
    assert_eq!(state.rows, usize::from(page.size.rows) - 2);
    assert_eq!(state.cells, usize::from(page.size.cols) - 6);
}

#[test]
fn page_plain_soft_wrapped_3_lines_with_unwrap() {
    let terminal = terminal(10, 24, "hello world this is a test");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: true,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello world this is a test");
    assert_eq!(state.rows, usize::from(page.size.rows) - 2);
    assert_eq!(state.cells, usize::from(page.size.cols) - 6);
}

#[test]
fn page_plain_start_y_subset() {
    let terminal = terminal(80, 24, "hello\r\nworld\r\ntest");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 1;
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world\ntest");
    assert_eq!(state.rows, usize::from(page.size.rows) - 2);
    assert_eq!(state.cells, usize::from(page.size.cols) - 4);
}

#[test]
fn page_plain_end_y_subset() {
    let terminal = terminal(80, 24, "hello\r\nworld\r\ntest");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.end_y = Some(1);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
    assert_eq!(state.rows, 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_start_y_and_end_y_range() {
    let terminal = terminal(80, 24, "hello\r\nworld\r\ntest\r\nfoo");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 1;
    formatter.end_y = Some(2);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world\ntest");
    assert_eq!(state.rows, 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 4);
}

#[test]
fn page_plain_start_y_out_of_bounds() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 30;
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "");
    assert_eq!(state.rows, 0);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_end_y_greater_than_rows() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.end_y = Some(30);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_end_y_less_than_start_y() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 5;
    formatter.end_y = Some(2);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "");
    assert_eq!(state.rows, 0);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_start_x_on_first_row_only() {
    let terminal = terminal(80, 24, "hello world");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 6;
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 11);
}

#[test]
fn page_plain_end_x_on_last_row_only() {
    let terminal = terminal(80, 24, "first line\r\nsecond line\r\nthird line");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.end_y = Some(2);
    formatter.end_x = Some(4);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "first line\nsecond line\nthird");
    assert_eq!(state.rows, 1);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_start_x_and_end_x_multiline() {
    let terminal = terminal(80, 24, "hello world\r\ntest case\r\nfoo bar");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 6;
    formatter.end_y = Some(2);
    formatter.end_x = Some(2);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world\ntest case\nfoo");
    assert_eq!(state.rows, 1);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_start_x_out_of_bounds() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 100;
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "");
    assert_eq!(state.rows, 0);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_end_x_greater_than_cols() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.end_x = Some(100);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_end_x_less_than_start_x_single_row() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 10;
    formatter.end_y = Some(0);
    formatter.end_x = Some(5);
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "");
    assert_eq!(state.rows, 0);
    assert_eq!(state.cells, 0);
}

#[test]
fn page_plain_start_y_non_zero_ignores_trailing_state() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 1;
    formatter.trailing_state = Some(TrailingState { rows: 5, cells: 10 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world");
    assert_eq!(state.rows, usize::from(page.size.rows) - 1);
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_start_x_non_zero_ignores_trailing_state() {
    let terminal = terminal(80, 24, "hello world");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_x = 6;
    formatter.trailing_state = Some(TrailingState { rows: 2, cells: 8 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "world");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 11);
}

#[test]
fn page_plain_start_y_and_start_x_zero_uses_trailing_state() {
    let terminal = terminal(80, 24, "hello");
    let page = page(&terminal);
    let mut formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    formatter.start_y = 0;
    formatter.start_x = 0;
    formatter.trailing_state = Some(TrailingState { rows: 2, cells: 0 });
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "\n\nhello");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 5);
}

#[test]
fn page_plain_single_line_with_styling() {
    let terminal = terminal(80, 24, "hello, \x1b[1mworld\x1b[0m");
    let page = page(&terminal);
    let formatter = PageFormatter::init(
        page,
        Options {
            unwrap: false,
            trim: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    let state = formatter.format_with_state(&mut output).unwrap();
    assert_eq!(output, "hello, world");
    assert_eq!(state.rows, usize::from(page.size.rows));
    assert_eq!(state.cells, usize::from(page.size.cols) - 12);
}

#[test]
fn page_codepoint_map_single_replacement() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    let terminal = terminal(80, 24, "hello world");
    let maps = [CodepointMap {
        range: 'o'..='o',
        replacement: Replacement::Codepoint('x'),
    }];
    let formatter = PageFormatter::init(
        page(&terminal),
        Options {
            codepoint_map: &maps,
            ..Options::default()
        },
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hellx wxrld");
}

#[test]
fn page_plain_single_wide_char() {
    let terminal = terminal(80, 24, "1A⚡");
    let page = page(&terminal);
    for (start, expected) in [(0, "1A⚡"), (2, "⚡"), (3, "⚡")] {
        let mut formatter = PageFormatter::init(page, Options::default());
        formatter.start_x = start;
        let mut output = String::new();
        let state = formatter.format_with_state(&mut output).unwrap();
        assert_eq!(output, expected);
        assert_eq!(
            state,
            TrailingState {
                rows: usize::from(page.size.rows),
                cells: 76
            }
        );
    }
}

#[test]
fn page_plain_single_wide_char_soft_wrapped_unwrapped() {
    let terminal = terminal(3, 24, "1A⚡");
    let page = page(&terminal);
    for (start_x, start_y, end, expected) in [
        (0, 0, None, "1A⚡"),
        (0, 0, Some((2, 0)), "1A⚡"),
        (2, 0, None, "⚡"),
        (1, 1, None, "⚡"),
    ] {
        let mut formatter = PageFormatter::init(
            page,
            Options {
                unwrap: true,
                ..Options::default()
            },
        );
        formatter.start_x = start_x;
        formatter.start_y = start_y;
        if let Some((x, y)) = end {
            formatter.end_x = Some(x);
            formatter.end_y = Some(y);
        }
        let mut output = String::new();
        let state = formatter.format_with_state(&mut output).unwrap();
        assert_eq!(output, expected);
        if end.is_none() {
            assert_eq!(
                state,
                TrailingState {
                    rows: usize::from(page.size.rows - 1),
                    cells: 1
                }
            );
        }
    }
}

fn mapped(text: &str, maps: &[maestro_terminal::formatter::CodepointMap<'_>]) -> String {
    let terminal = terminal(80, 24, text);
    let formatter = PageFormatter::init(
        page(&terminal),
        Options {
            codepoint_map: maps,
            ..Options::default()
        },
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    output
}

#[test]
fn page_codepoint_map_conflicting_replacement_prefers_last() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(
        mapped(
            "hello",
            &[
                CodepointMap {
                    range: 'o'..='o',
                    replacement: Replacement::Codepoint('x')
                },
                CodepointMap {
                    range: 'o'..='o',
                    replacement: Replacement::Codepoint('y')
                },
            ]
        ),
        "helly"
    );
}

#[test]
fn page_codepoint_map_replace_with_string() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(
        mapped(
            "hello",
            &[CodepointMap {
                range: 'o'..='o',
                replacement: Replacement::String("XYZ")
            }]
        ),
        "hellXYZ"
    );
}

#[test]
fn page_codepoint_map_range_replacement() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(
        mapped(
            "abcdefg",
            &[CodepointMap {
                range: 'b'..='e',
                replacement: Replacement::Codepoint('X')
            }]
        ),
        "aXXXXfg"
    );
}

#[test]
fn page_codepoint_map_multiple_ranges() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(
        mapped(
            "hello world",
            &[
                CodepointMap {
                    range: 'a'..='m',
                    replacement: Replacement::Codepoint('A')
                },
                CodepointMap {
                    range: 'n'..='z',
                    replacement: Replacement::Codepoint('Z')
                },
            ]
        ),
        "AAAAZ ZZZAA"
    );
}

#[test]
fn page_codepoint_map_unicode_replacement() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(
        mapped(
            "hello ⚡ world",
            &[CodepointMap {
                range: '⚡'..='⚡',
                replacement: Replacement::Codepoint('🔥')
            }]
        ),
        "hello 🔥 world"
    );
}

#[test]
fn page_codepoint_map_empty_map() {
    assert_eq!(mapped("hello world", &[]), "hello world");
}

#[test]
fn page_plain_graphemes_and_replaced_suffix() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    assert_eq!(mapped("e\u{301} 👩‍💻", &[]), "e\u{301} 👩‍💻");
    assert_eq!(
        mapped(
            "e\u{301}",
            &[CodepointMap {
                range: '\u{301}'..='\u{301}',
                replacement: Replacement::String("!")
            }]
        ),
        "e!"
    );
}

#[test]
fn page_plain_blank_cells_between_text() {
    assert_eq!(mapped("a\x1b[4Gb", &[]), "a  b");
}

#[test]
fn formatter_destination_failure_propagates() {
    struct Refuse;
    impl std::fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let terminal = terminal(80, 24, "hello");
    assert!(
        maestro_terminal::formatter::TerminalFormatter::init(&terminal, Options::default())
            .format(&mut Refuse)
            .is_err()
    );
}

fn screen_pin(terminal: &Terminal, x: u16, y: u32) -> maestro_terminal::Pin {
    terminal
        .screens
        .active()
        .pages
        .pin(maestro_terminal::Point::Screen(
            maestro_terminal::Coordinate { x, y },
        ))
        .unwrap()
}

#[test]
fn pagelist_plain_single_line() {
    let terminal = terminal(80, 24, "hello, world");
    let formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello, world");
}

#[test]
fn pagelist_plain_with_x_offset_on_single_page() {
    let terminal = terminal(80, 24, "hello world\r\ntest case\r\nfoo bar");
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(screen_pin(&terminal, 6, 0));
    formatter.bottom_right = Some(screen_pin(&terminal, 2, 2));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "world\ntest case\nfoo");
}

#[test]
fn pagelist_plain_with_start_x_only() {
    let terminal = terminal(80, 24, "hello world");
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(screen_pin(&terminal, 6, 0));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "world");
}

#[test]
fn pagelist_plain_with_end_x_only() {
    let terminal = terminal(80, 24, "hello world\r\ntest");
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.bottom_right = Some(screen_pin(&terminal, 2, 1));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello world\ntes");
}

fn page_boundary(cols: u16, text: &str) -> (Terminal, u32) {
    let mut terminal = terminal(cols, 3, "");
    let capacity = page(&terminal).capacity.rows;
    let prefix = "\r\n".repeat(usize::from(capacity - 1));
    maestro_terminal::stream::Stream::new(false).next_slice(
        format!("{prefix}{text}").as_bytes(),
        &mut maestro_terminal::stream_terminal::Handler::new(&mut terminal),
    );
    let pages = &terminal.screens.active().pages;
    assert_ne!(
        pages.get_top_left(Tag::Screen).node,
        pages.get_bottom_right(Tag::Screen).unwrap().node
    );
    (terminal, u32::from(capacity - 1))
}

#[test]
fn pagelist_plain_spanning_two_pages() {
    let (terminal, _) = page_boundary(80, "page one\r\npage two");
    let formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output.trim_start_matches('\n'), "page one\npage two");
}

#[test]
fn pagelist_soft_wrapped_line_spanning_two_pages_without_unwrap() {
    let (terminal, _) = page_boundary(10, "hello world test");
    let formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output.trim_start_matches('\n'), "hello worl\nd test");
}

#[test]
fn pagelist_soft_wrapped_line_spanning_two_pages_with_unwrap() {
    let (terminal, _) = page_boundary(10, "hello world test");
    let formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options {
            unwrap: true,
            ..Options::default()
        },
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output.trim_start_matches('\n'), "hello world test");
}

#[test]
fn pagelist_plain_with_x_offset_spanning_two_pages() {
    let (terminal, y) = page_boundary(80, "hello world\r\nfoo bar test");
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(screen_pin(&terminal, 6, y));
    formatter.bottom_right = Some(screen_pin(&terminal, 2, y + 1));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "world\nfoo");
}

fn rectangle(text: &str, rows: u16, start: (u16, u32), end: (u16, u32)) -> String {
    let terminal = terminal(30, rows, text);
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(screen_pin(&terminal, start.0, start.1));
    formatter.bottom_right = Some(screen_pin(&terminal, end.0, end.1));
    formatter.rectangle = true;
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    output
}

const LOREM: &str = "Lorem ipsum dolor\r\nsit amet, consectetur\r\nadipiscing elit, sed do\r\neiusmod tempor incididunt\r\nut labore et dolore";

#[test]
fn pagelist_plain_rectangle_basic() {
    assert_eq!(rectangle(LOREM, 5, (2, 1), (6, 3)), "t ame\nipisc\nusmod");
}

#[test]
fn pagelist_plain_rectangle_with_eol() {
    assert_eq!(
        rectangle(LOREM, 5, (12, 0), (26, 4)),
        "dolor\nnsectetur\nlit, sed do\nor incididunt\n dolore"
    );
}

#[test]
fn pagelist_plain_rectangle_more_complex_with_breaks() {
    assert_eq!(
        rectangle(
            &format!("{LOREM}\r\n\r\nmagna aliqua. Ut enim\r\nad minim veniam, quis"),
            8,
            (11, 2),
            (26, 7)
        ),
        "elit, sed do\npor incididunt\nt dolore\n\na. Ut enim\nniam, quis"
    );
}

#[test]
fn screen_plain_single_line() {
    let terminal = terminal(80, 24, "hello, world");
    let formatter = maestro_terminal::formatter::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello, world");
}

#[test]
fn screen_plain_multiline() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let formatter = maestro_terminal::formatter::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
}

#[test]
fn screen_plain_with_selection() {
    let terminal = terminal(80, 24, "line1\r\nline2\r\nline3");
    let mut formatter = maestro_terminal::formatter::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal::formatter::Content::Selection(Some(
        maestro_terminal::selection::Selection::init(
            screen_pin(&terminal, 0, 1),
            screen_pin(&terminal, 4, 1),
            false,
        ),
    ));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "line2");
}

#[test]
fn terminal_formatter_with_selection() {
    let terminal = terminal(80, 24, "line1\r\nline2\r\nline3");
    let mut formatter =
        maestro_terminal::formatter::TerminalFormatter::init(&terminal, Options::default());
    formatter.content = maestro_terminal::formatter::Content::Selection(Some(
        maestro_terminal::selection::Selection::init(
            screen_pin(&terminal, 0, 1),
            screen_pin(&terminal, 4, 1),
            false,
        ),
    ));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "line2");
}

#[test]
fn terminal_formatter_no_content() {
    let terminal = terminal(80, 24, "hello");
    let mut formatter =
        maestro_terminal::formatter::TerminalFormatter::init(&terminal, Options::default());
    formatter.content = maestro_terminal::formatter::Content::None;
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn screen_plain_selection_orders_rectangle_endpoints() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let mut formatter = maestro_terminal::formatter::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal::formatter::Content::Selection(Some(
        maestro_terminal::selection::Selection::init(
            screen_pin(&terminal, 3, 0),
            screen_pin(&terminal, 1, 1),
            true,
        ),
    ));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "ell\norl");
}

#[test]
fn formatter_rejects_invalid_pins() {
    let terminal = terminal(80, 24, "hello");
    let invalid = maestro_terminal::Pin {
        x: 80,
        ..screen_pin(&terminal, 0, 0)
    };
    let mut formatter = maestro_terminal::formatter::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(invalid);
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
    let mut formatter = maestro_terminal::formatter::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal::formatter::Content::Selection(Some(
        maestro_terminal::selection::Selection::init(invalid, screen_pin(&terminal, 4, 0), false),
    ));
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn format_plain_reflects_terminal_changes() {
    use maestro_terminal::formatter::TerminalFormatter;
    let mut terminal = terminal(80, 24, "Hello");
    let mut output = String::new();
    TerminalFormatter::init(&terminal, Options::default())
        .format(&mut output)
        .unwrap();
    assert_eq!(output, "Hello");
    maestro_terminal::stream::Stream::new(false).next_slice(
        b"\r\nWorld",
        &mut maestro_terminal::stream_terminal::Handler::new(&mut terminal),
    );
    output.clear();
    TerminalFormatter::init(&terminal, Options::default())
        .format(&mut output)
        .unwrap();
    assert_eq!(output, "Hello\nWorld");
}

#[test]
fn format_plain_with_selection() {
    use maestro_terminal::{
        formatter::{Content, TerminalFormatter},
        selection::Selection,
    };
    let terminal = terminal(80, 24, "Hello World");
    let mut formatter = TerminalFormatter::init(&terminal, Options::default());
    formatter.content = Content::Selection(Some(Selection::init(
        screen_pin(&terminal, 6, 0),
        screen_pin(&terminal, 10, 0),
        false,
    )));
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "World");
}

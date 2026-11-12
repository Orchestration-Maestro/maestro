//! Plain-text output over retained terminal pages.

use maestro_terminal_emulator::{
    Page, Terminal,
    formatting::{
        CodepointMap, Options, PageFormatter, PageListFormatter, Replacement, TrailingState,
    },
    point::Tag,
};

fn terminal(cols: u16, rows: u16, text: &str) -> Terminal {
    let mut terminal = Terminal::new(cols, rows, 100_000).unwrap();
    maestro_terminal_emulator::stream::Stream::new(false).next_slice(
        text.as_bytes(),
        &mut maestro_terminal_emulator::stream_terminal::Handler::new(&mut terminal),
    );
    terminal
}

fn page(terminal: &Terminal) -> &Page {
    let pages = &terminal.screens.active().pages;
    pages.page(pages.top_left(Tag::Screen).node).unwrap()
}

#[test]
fn terminal_formatter_plain_no_selection() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let formatter = maestro_terminal_emulator::formatting::TerminalFormatter::init(
        &terminal,
        Options::default(),
    );
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello\nworld");
}

#[test]
fn page_plain_with_prior_trailing_state_cells_with_wrap_continuation() {
    let mut terminal = terminal(80, 24, "world");
    let pages = &mut terminal.screens.active_mut().pages;
    let node = pages.top_left(Tag::Screen).node;
    pages
        .page_mut(node)
        .unwrap()
        .row_mut(0)
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

fn mapped(text: &str, maps: &[maestro_terminal_emulator::formatting::CodepointMap<'_>]) -> String {
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
fn formatter_destination_failure_propagates() {
    struct Refuse;
    impl std::fmt::Write for Refuse {
        fn write_str(&mut self, _: &str) -> std::fmt::Result {
            Err(std::fmt::Error)
        }
    }
    let terminal = terminal(80, 24, "hello");
    assert!(
        maestro_terminal_emulator::formatting::TerminalFormatter::init(
            &terminal,
            Options::default()
        )
        .format(&mut Refuse)
        .is_err()
    );
}

fn screen_pin(terminal: &Terminal, x: u16, y: u32) -> maestro_terminal_emulator::Pin {
    terminal
        .screens
        .active()
        .pages
        .pin(maestro_terminal_emulator::Point::Screen(
            maestro_terminal_emulator::Coordinate { x, y },
        ))
        .unwrap()
}

fn page_boundary(cols: u16, text: &str) -> (Terminal, u32) {
    let mut terminal = terminal(cols, 3, "");
    let capacity = page(&terminal).capacity.rows;
    let prefix = "\r\n".repeat(usize::from(capacity - 1));
    maestro_terminal_emulator::stream::Stream::new(false).next_slice(
        format!("{prefix}{text}").as_bytes(),
        &mut maestro_terminal_emulator::stream_terminal::Handler::new(&mut terminal),
    );
    let pages = &terminal.screens.active().pages;
    assert_ne!(
        pages.top_left(Tag::Screen).node,
        pages.get_bottom_right(Tag::Screen).unwrap().node
    );
    (terminal, u32::from(capacity - 1))
}

#[test]
fn pagelist_plain_with_x_offset_spanning_two_pages() {
    let (terminal, y) = page_boundary(80, "hello world\r\nfoo bar test");
    let mut formatter = maestro_terminal_emulator::formatting::PageListFormatter::init(
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
    let mut formatter = maestro_terminal_emulator::formatting::PageListFormatter::init(
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
fn screen_plain_single_line() {
    let terminal = terminal(80, 24, "hello, world");
    let formatter = maestro_terminal_emulator::formatting::ScreenFormatter::init(
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
    let formatter = maestro_terminal_emulator::formatting::ScreenFormatter::init(
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
    let mut formatter = maestro_terminal_emulator::formatting::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal_emulator::formatting::Content::Selection(Some(
        maestro_terminal_emulator::selection::Selection::init(
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
    let mut formatter = maestro_terminal_emulator::formatting::TerminalFormatter::init(
        &terminal,
        Options::default(),
    );
    formatter.content = maestro_terminal_emulator::formatting::Content::Selection(Some(
        maestro_terminal_emulator::selection::Selection::init(
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
    let mut formatter = maestro_terminal_emulator::formatting::TerminalFormatter::init(
        &terminal,
        Options::default(),
    );
    formatter.content = maestro_terminal_emulator::formatting::Content::None;
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn screen_plain_selection_orders_rectangle_endpoints() {
    let terminal = terminal(80, 24, "hello\r\nworld");
    let mut formatter = maestro_terminal_emulator::formatting::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal_emulator::formatting::Content::Selection(Some(
        maestro_terminal_emulator::selection::Selection::init(
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
    let invalid = maestro_terminal_emulator::Pin {
        x: 80,
        ..screen_pin(&terminal, 0, 0)
    };
    let mut formatter = maestro_terminal_emulator::formatting::PageListFormatter::init(
        &terminal.screens.active().pages,
        Options::default(),
    );
    formatter.top_left = Some(invalid);
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
    let mut formatter = maestro_terminal_emulator::formatting::ScreenFormatter::init(
        terminal.screens.active(),
        Options::default(),
    );
    formatter.content = maestro_terminal_emulator::formatting::Content::Selection(Some(
        maestro_terminal_emulator::selection::Selection::init(
            invalid,
            screen_pin(&terminal, 4, 0),
            false,
        ),
    ));
    formatter.format(&mut output).unwrap();
    assert!(output.is_empty());
}

#[test]
fn format_plain_reflects_terminal_changes() {
    use maestro_terminal_emulator::formatting::TerminalFormatter;
    let mut terminal = terminal(80, 24, "Hello");
    let mut output = String::new();
    TerminalFormatter::init(&terminal, Options::default())
        .format(&mut output)
        .unwrap();
    assert_eq!(output, "Hello");
    maestro_terminal_emulator::stream::Stream::new(false).next_slice(
        b"\r\nWorld",
        &mut maestro_terminal_emulator::stream_terminal::Handler::new(&mut terminal),
    );
    output.clear();
    TerminalFormatter::init(&terminal, Options::default())
        .format(&mut output)
        .unwrap();
    assert_eq!(output, "Hello\nWorld");
}

#[test]
fn format_plain_with_selection() {
    use maestro_terminal_emulator::{
        formatting::{Content, TerminalFormatter},
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

/// A count the formatter reports, relative to the page it formatted.
#[derive(Clone, Copy)]
enum Count {
    /// The page's own count minus this many.
    Short(usize),
    /// This exact count.
    Exact(usize),
}

impl Count {
    /// The count the formatter must report for a page whose own count is `page`.
    fn of(self, page: u16) -> usize {
        match self {
            Self::Short(by) => usize::from(page) - by,
            Self::Exact(count) => count,
        }
    }
}

/// One plain page formatting case: the terminal, the formatter settings and the result.
#[derive(Clone, Copy)]
struct PlainCase {
    /// Name that a failing assertion reports.
    name: &'static str,
    /// Terminal width in cells.
    cols: u16,
    /// Terminal height in rows.
    rows: u16,
    /// Bytes the terminal reads.
    text: &'static str,
    /// Join soft-wrapped rows.
    unwrap: bool,
    /// Omit trailing spaces.
    trim: bool,
    /// First column formatted on the first row.
    start_x: u16,
    /// First row formatted.
    start_y: u16,
    /// Last column formatted on the last row.
    end_x: Option<u16>,
    /// Last row formatted.
    end_y: Option<u16>,
    /// Format a rectangle instead of a stream.
    rectangle: bool,
    /// Blanks carried in from a previous page.
    trailing: Option<TrailingState>,
    /// The formatted text.
    expected: &'static str,
    /// The rows left pending, when the case states them with the cells.
    pending: Option<(Count, Count)>,
}

/// Defaults a case overrides: an 80 by 24 terminal and the default options.
const PLAIN: PlainCase = PlainCase {
    name: "",
    cols: 80,
    rows: 24,
    text: "",
    unwrap: false,
    trim: true,
    start_x: 0,
    start_y: 0,
    end_x: None,
    end_y: None,
    rectangle: false,
    trailing: None,
    expected: "",
    pending: None,
};

/// The plain page cases.
const PLAIN_CASES: &[PlainCase] = &[
    PlainCase {
        name: "page_plain_single_line",
        text: "hello, world",
        expected: "hello, world",
        pending: Some((Count::Short(0), Count::Short(12))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_single_line_soft_wrapped_unwrapped",
        cols: 3,
        rows: 5,
        text: "hello!",
        unwrap: true,
        expected: "hello!",
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_multiline",
        text: "hello\r\nworld",
        expected: "hello\nworld",
        pending: Some((Count::Short(1), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_multiline_rectangle",
        text: "hello\r\nworld",
        start_x: 1,
        end_x: Some(3),
        rectangle: true,
        expected: "ell\norl",
        pending: Some((Count::Short(1), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_multi_blank_lines",
        text: "hello\r\n\r\n\r\nworld",
        expected: "hello\n\n\nworld",
        pending: Some((Count::Short(3), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_trailing_blank_lines",
        text: "hello\r\nworld\r\n\r\n",
        expected: "hello\nworld",
        pending: Some((Count::Short(1), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_trailing_whitespace",
        text: "hello   \r\nworld   ",
        expected: "hello\nworld",
        pending: Some((Count::Short(1), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_trailing_whitespace_no_trim",
        text: "hello   \r\nworld  ",
        trim: false,
        expected: "hello   \nworld  ",
        pending: Some((Count::Short(1), Count::Short(7))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_with_prior_trailing_state_rows",
        text: "hello",
        trailing: Some(TrailingState { rows: 2, cells: 0 }),
        expected: "\n\nhello",
        pending: Some((Count::Short(0), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_with_prior_trailing_state_cells_no_wrapped_line",
        text: "hello",
        trailing: Some(TrailingState { rows: 0, cells: 3 }),
        expected: "hello",
        pending: Some((Count::Short(0), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_soft_wrapped_without_unwrap",
        cols: 10,
        text: "hello world test",
        expected: "hello worl\nd test",
        pending: Some((Count::Short(1), Count::Short(6))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_soft_wrapped_with_unwrap",
        cols: 10,
        text: "hello world test",
        unwrap: true,
        expected: "hello world test",
        pending: Some((Count::Short(1), Count::Short(6))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_soft_wrapped_3_lines_without_unwrap",
        cols: 10,
        text: "hello world this is a test",
        expected: "hello worl\nd this is\na test",
        pending: Some((Count::Short(2), Count::Short(6))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_soft_wrapped_3_lines_with_unwrap",
        cols: 10,
        text: "hello world this is a test",
        unwrap: true,
        expected: "hello world this is a test",
        pending: Some((Count::Short(2), Count::Short(6))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_y_subset",
        text: "hello\r\nworld\r\ntest",
        start_y: 1,
        expected: "world\ntest",
        pending: Some((Count::Short(2), Count::Short(4))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_y_subset",
        text: "hello\r\nworld\r\ntest",
        end_y: Some(1),
        expected: "hello\nworld",
        pending: Some((Count::Exact(1), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_y_and_end_y_range",
        text: "hello\r\nworld\r\ntest\r\nfoo",
        start_y: 1,
        end_y: Some(2),
        expected: "world\ntest",
        pending: Some((Count::Exact(1), Count::Short(4))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_y_out_of_bounds",
        text: "hello",
        start_y: 30,
        expected: "",
        pending: Some((Count::Exact(0), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_y_greater_than_rows",
        text: "hello",
        end_y: Some(30),
        expected: "hello",
        pending: Some((Count::Short(0), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_y_less_than_start_y",
        text: "hello",
        start_y: 5,
        end_y: Some(2),
        expected: "",
        pending: Some((Count::Exact(0), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_x_on_first_row_only",
        text: "hello world",
        start_x: 6,
        expected: "world",
        pending: Some((Count::Short(0), Count::Short(11))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_x_on_last_row_only",
        text: "first line\r\nsecond line\r\nthird line",
        end_y: Some(2),
        end_x: Some(4),
        expected: "first line\nsecond line\nthird",
        pending: Some((Count::Exact(1), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_x_and_end_x_multiline",
        text: "hello world\r\ntest case\r\nfoo bar",
        start_x: 6,
        end_y: Some(2),
        end_x: Some(2),
        expected: "world\ntest case\nfoo",
        pending: Some((Count::Exact(1), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_x_out_of_bounds",
        text: "hello",
        start_x: 100,
        expected: "",
        pending: Some((Count::Exact(0), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_x_greater_than_cols",
        text: "hello",
        end_x: Some(100),
        expected: "hello",
        pending: Some((Count::Short(0), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_end_x_less_than_start_x_single_row",
        text: "hello",
        start_x: 10,
        end_y: Some(0),
        end_x: Some(5),
        expected: "",
        pending: Some((Count::Exact(0), Count::Exact(0))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_y_non_zero_ignores_trailing_state",
        text: "hello\r\nworld",
        start_y: 1,
        trailing: Some(TrailingState { rows: 5, cells: 10 }),
        expected: "world",
        pending: Some((Count::Short(1), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_x_non_zero_ignores_trailing_state",
        text: "hello world",
        start_x: 6,
        trailing: Some(TrailingState { rows: 2, cells: 8 }),
        expected: "world",
        pending: Some((Count::Short(0), Count::Short(11))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_start_y_and_start_x_zero_uses_trailing_state",
        text: "hello",
        trailing: Some(TrailingState { rows: 2, cells: 0 }),
        expected: "\n\nhello",
        pending: Some((Count::Short(0), Count::Short(5))),
        ..PLAIN
    },
    PlainCase {
        name: "page_plain_single_line_with_styling",
        text: "hello, \x1b[1mworld\x1b[0m",
        expected: "hello, world",
        pending: Some((Count::Short(0), Count::Short(12))),
        ..PLAIN
    },
];

#[test]
fn page_plain_cases_format_and_report_trailing_state() {
    for case in PLAIN_CASES {
        let terminal = terminal(case.cols, case.rows, case.text);
        let page = page(&terminal);
        let mut formatter = PageFormatter::init(
            page,
            Options {
                unwrap: case.unwrap,
                trim: case.trim,
                ..Options::default()
            },
        );
        formatter.start_x = case.start_x;
        formatter.start_y = case.start_y;
        formatter.end_x = case.end_x;
        formatter.end_y = case.end_y;
        formatter.rectangle = case.rectangle;
        formatter.trailing_state = case.trailing;
        let mut output = String::new();
        let state = formatter.format_with_state(&mut output).unwrap();
        assert_eq!(output, case.expected, "{}", case.name);
        if let Some((rows, cells)) = case.pending {
            assert_eq!(state.rows, rows.of(page.size.rows), "{} rows", case.name);
            assert_eq!(state.cells, cells.of(page.size.cols), "{} cells", case.name);
        }
    }
}

/// One codepoint-map case: the text, the substitutions and the formatted result.
struct MapCase {
    /// Name that a failing assertion reports.
    name: &'static str,
    /// Bytes the terminal reads.
    text: &'static str,
    /// Substitutions applied while formatting.
    maps: &'static [CodepointMap<'static>],
    /// The formatted text.
    expected: &'static str,
}

/// The codepoint-map cases.
const MAP_CASES: &[MapCase] = &[
    MapCase {
        name: "page_codepoint_map_single_replacement",
        text: "hello world",
        maps: &[CodepointMap {
            range: 'o'..='o',
            replacement: Replacement::Codepoint('x'),
        }],
        expected: "hellx wxrld",
    },
    MapCase {
        name: "page_codepoint_map_conflicting_replacement_prefers_last",
        text: "hello",
        maps: &[
            CodepointMap {
                range: 'o'..='o',
                replacement: Replacement::Codepoint('x'),
            },
            CodepointMap {
                range: 'o'..='o',
                replacement: Replacement::Codepoint('y'),
            },
        ],
        expected: "helly",
    },
    MapCase {
        name: "page_codepoint_map_replace_with_string",
        text: "hello",
        maps: &[CodepointMap {
            range: 'o'..='o',
            replacement: Replacement::String("XYZ"),
        }],
        expected: "hellXYZ",
    },
    MapCase {
        name: "page_codepoint_map_range_replacement",
        text: "abcdefg",
        maps: &[CodepointMap {
            range: 'b'..='e',
            replacement: Replacement::Codepoint('X'),
        }],
        expected: "aXXXXfg",
    },
    MapCase {
        name: "page_codepoint_map_multiple_ranges",
        text: "hello world",
        maps: &[
            CodepointMap {
                range: 'a'..='m',
                replacement: Replacement::Codepoint('A'),
            },
            CodepointMap {
                range: 'n'..='z',
                replacement: Replacement::Codepoint('Z'),
            },
        ],
        expected: "AAAAZ ZZZAA",
    },
    MapCase {
        name: "page_codepoint_map_unicode_replacement",
        text: "hello ⚡ world",
        maps: &[CodepointMap {
            range: '⚡'..='⚡',
            replacement: Replacement::Codepoint('🔥'),
        }],
        expected: "hello 🔥 world",
    },
    MapCase {
        name: "page_codepoint_map_empty_map",
        text: "hello world",
        maps: &[],
        expected: "hello world",
    },
    MapCase {
        name: "page_plain_graphemes_keep_their_marks",
        text: "e\u{301} 👩‍💻",
        maps: &[],
        expected: "e\u{301} 👩‍💻",
    },
    MapCase {
        name: "page_plain_graphemes_replaced_suffix",
        text: "e\u{301}",
        maps: &[CodepointMap {
            range: '\u{301}'..='\u{301}',
            replacement: Replacement::String("!"),
        }],
        expected: "e!",
    },
    MapCase {
        name: "page_plain_blank_cells_between_text",
        text: "a\x1b[4Gb",
        maps: &[],
        expected: "a  b",
    },
];

#[test]
fn page_codepoint_map_cases_format_their_replacements() {
    for case in MAP_CASES {
        assert_eq!(mapped(case.text, case.maps), case.expected, "{}", case.name);
    }
}

/// One page-list case: the text, the pins that bound it and the formatted result.
struct PinCase {
    /// Name that a failing assertion reports.
    name: &'static str,
    /// Bytes the terminal reads.
    text: &'static str,
    /// Column and row where formatting starts.
    top_left: Option<(u16, u32)>,
    /// Column and row where formatting ends.
    bottom_right: Option<(u16, u32)>,
    /// The formatted text.
    expected: &'static str,
}

/// The page-list cases on one page.
const PIN_CASES: &[PinCase] = &[
    PinCase {
        name: "pagelist_plain_single_line",
        text: "hello, world",
        top_left: None,
        bottom_right: None,
        expected: "hello, world",
    },
    PinCase {
        name: "pagelist_plain_with_x_offset_on_single_page",
        text: "hello world\r\ntest case\r\nfoo bar",
        top_left: Some((6, 0)),
        bottom_right: Some((2, 2)),
        expected: "world\ntest case\nfoo",
    },
    PinCase {
        name: "pagelist_plain_with_start_x_only",
        text: "hello world",
        top_left: Some((6, 0)),
        bottom_right: None,
        expected: "world",
    },
    PinCase {
        name: "pagelist_plain_with_end_x_only",
        text: "hello world\r\ntest",
        top_left: None,
        bottom_right: Some((2, 1)),
        expected: "hello world\ntes",
    },
];

#[test]
fn pagelist_pin_cases_format_their_range() {
    for case in PIN_CASES {
        let terminal = terminal(80, 24, case.text);
        let mut formatter =
            PageListFormatter::init(&terminal.screens.active().pages, Options::default());
        formatter.top_left = case.top_left.map(|(x, y)| screen_pin(&terminal, x, y));
        formatter.bottom_right = case.bottom_right.map(|(x, y)| screen_pin(&terminal, x, y));
        let mut output = String::new();
        formatter.format(&mut output).unwrap();
        assert_eq!(output, case.expected, "{}", case.name);
    }
}

/// Text that crosses a page boundary: the width, the text, whether rows join and the result.
const BOUNDARY_CASES: &[(&str, u16, &str, bool, &str)] = &[
    (
        "pagelist_plain_spanning_two_pages",
        80,
        "page one\r\npage two",
        false,
        "page one\npage two",
    ),
    (
        "pagelist_soft_wrapped_line_spanning_two_pages_without_unwrap",
        10,
        "hello world test",
        false,
        "hello worl\nd test",
    ),
    (
        "pagelist_soft_wrapped_line_spanning_two_pages_with_unwrap",
        10,
        "hello world test",
        true,
        "hello world test",
    ),
];

#[test]
fn pagelist_boundary_cases_join_pages() {
    for (name, cols, text, unwrap, expected) in BOUNDARY_CASES {
        let (terminal, _) = page_boundary(*cols, text);
        let formatter = PageListFormatter::init(
            &terminal.screens.active().pages,
            Options {
                unwrap: *unwrap,
                ..Options::default()
            },
        );
        let mut output = String::new();
        formatter.format(&mut output).unwrap();
        assert_eq!(output.trim_start_matches('\n'), *expected, "{name}");
    }
}

#[test]
fn pagelist_rectangle_cases_keep_their_columns() {
    let long = format!("{LOREM}\r\n\r\nmagna aliqua. Ut enim\r\nad minim veniam, quis");
    for (name, text, rows, start, end, expected) in [
        (
            "pagelist_plain_rectangle_basic",
            LOREM,
            5,
            (2, 1),
            (6, 3),
            "t ame\nipisc\nusmod",
        ),
        (
            "pagelist_plain_rectangle_with_eol",
            LOREM,
            5,
            (12, 0),
            (26, 4),
            "dolor\nnsectetur\nlit, sed do\nor incididunt\n dolore",
        ),
        (
            "pagelist_plain_rectangle_more_complex_with_breaks",
            long.as_str(),
            8,
            (11, 2),
            (26, 7),
            "elit, sed do\npor incididunt\nt dolore\n\na. Ut enim\nniam, quis",
        ),
    ] {
        assert_eq!(rectangle(text, rows, start, end), expected, "{name}");
    }
}

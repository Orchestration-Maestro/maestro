//! Styled text emitted from retained terminal storage.

use maestro_terminal::{
    Terminal,
    formatter::{Format, Options, PageFormatter, ScreenFormatter, TerminalFormatter},
};

fn terminal(cols: u16, text: &str) -> Terminal {
    let mut terminal = Terminal::new(cols, 24, 100_000).unwrap();
    maestro_terminal::stream::Stream::new(false).next_slice(
        text.as_bytes(),
        &mut maestro_terminal::stream_terminal::Handler::new(&mut terminal),
    );
    terminal
}

fn page_output(cols: u16, text: &str, opts: Options<'_>) -> String {
    let terminal = terminal(cols, text);
    let pages = &terminal.screens.active().pages;
    let page = pages
        .page(
            pages
                .get_top_left(maestro_terminal::point::Tag::Screen)
                .node,
        )
        .unwrap();
    let mut output = String::new();
    PageFormatter::init(page, opts).format(&mut output).unwrap();
    output
}

fn vt() -> Options<'static> {
    Options {
        emit: Format::Vt,
        ..Options::default()
    }
}

#[test]
fn page_vt_single_line_plain_text() {
    assert_eq!(page_output(80, "hello", vt()), "hello");
}

#[test]
fn page_vt_single_line_with_bold() {
    assert_eq!(
        page_output(80, "\x1b[1mhello\x1b[0m", vt()),
        "\x1b[0m\x1b[1mhello\x1b[0m"
    );
}

#[test]
fn page_vt_multiple_styles() {
    assert_eq!(
        page_output(80, "\x1b[1mhello \x1b[3mworld\x1b[0m", vt()),
        "\x1b[0m\x1b[1mhello \x1b[0m\x1b[1m\x1b[3mworld\x1b[0m"
    );
}

#[test]
fn page_vt_with_foreground_color() {
    assert_eq!(
        page_output(80, "\x1b[31mred\x1b[0m", vt()),
        "\x1b[0m\x1b[38;5;1mred\x1b[0m"
    );
}

#[test]
fn page_vt_multi_line_with_styles() {
    assert_eq!(
        page_output(80, "\x1b[1mfirst\x1b[0m\r\n\x1b[3msecond\x1b[0m", vt()),
        "\x1b[0m\x1b[1mfirst\x1b[0m\r\n\x1b[0m\x1b[3msecond\x1b[0m"
    );
}

#[test]
fn page_vt_duplicate_style_not_emitted_twice() {
    assert_eq!(
        page_output(80, "\x1b[1mhel\x1b[1mlo\x1b[0m", vt()),
        "\x1b[0m\x1b[1mhello\x1b[0m"
    );
}

#[test]
fn page_vt_style_reset_properly_closes_styles() {
    assert_eq!(
        page_output(80, "\x1b[1mbold\x1b[0mnormal", vt()),
        "\x1b[0m\x1b[1mbold\x1b[0mnormal"
    );
}

#[test]
fn page_vt_with_background_and_foreground_colors() {
    use maestro_terminal::color::Rgb;
    let opts = Options {
        foreground: Some(Rgb {
            r: 0xab,
            g: 0xcd,
            b: 0xef,
        }),
        background: Some(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        }),
        ..vt()
    };
    assert_eq!(
        page_output(80, "hello", opts),
        "\x1b]10;rgb:ab/cd/ef\x1b\\\x1b]11;rgb:12/34/56\x1b\\hello"
    );
}

#[test]
fn page_codepoint_map_with_styled_formats() {
    use maestro_terminal::formatter::{CodepointMap, Replacement};
    let map = [CodepointMap {
        range: 'e'..='e',
        replacement: Replacement::Codepoint('X'),
    }];
    assert_eq!(
        page_output(
            10,
            "\x1b[31mred text\x1b[0m",
            Options {
                codepoint_map: &map,
                ..vt()
            }
        ),
        "\x1b[0m\x1b[38;5;1mrXd tXxt\x1b[0m"
    );
}

#[test]
fn page_vt_background_color_on_trailing_blank_cells() {
    let output = page_output(
        20,
        "CPU:\x1b[41m\x1b[K\x1b[0m\r\nline2",
        Options {
            trim: false,
            ..vt()
        },
    );
    assert_eq!(
        output,
        format!("CPU:\x1b[0m\x1b[48;5;1m{}\x1b[0m\r\nline2", " ".repeat(16))
    );
}

#[test]
fn pagelist_vt_spanning_two_pages() {
    use maestro_terminal::{formatter::PageListFormatter, point::Tag};
    let mut terminal = terminal(80, "");
    let list = &terminal.screens.active().pages;
    let rows = list
        .page(list.get_top_left(Tag::Screen).node)
        .unwrap()
        .capacity
        .rows;
    let text = format!(
        "{}\x1b[1mpage one\r\npage two",
        "\r\n".repeat(usize::from(rows - 1))
    );
    maestro_terminal::stream::Stream::new(false).next_slice(
        text.as_bytes(),
        &mut maestro_terminal::stream_terminal::Handler::new(&mut terminal),
    );
    assert_ne!(
        terminal
            .screens
            .active()
            .pages
            .get_top_left(Tag::Screen)
            .node,
        terminal
            .screens
            .active()
            .pages
            .get_bottom_right(Tag::Screen)
            .unwrap()
            .node
    );
    let mut output = String::new();
    PageListFormatter::init(&terminal.screens.active().pages, vt())
        .format(&mut output)
        .unwrap();
    assert_eq!(
        output.trim_start_matches("\r\n"),
        "\x1b[0m\x1b[1mpage one\x1b[0m\r\n\x1b[0m\x1b[1mpage two\x1b[0m"
    );
}

#[test]
fn screen_vt_with_kitty_keyboard() {
    let terminal = terminal(80, "\x1b[=3;1uhello");
    let mut formatter = ScreenFormatter::init(terminal.screens.active(), vt());
    formatter.extra.kitty_keyboard = true;
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "hello\x1b[=3u");
    let replay = terminal_from_output(&output);
    assert_eq!(
        terminal.kitty_keyboard_flags(),
        replay.kitty_keyboard_flags()
    );
}

fn terminal_from_output(output: &str) -> Terminal {
    terminal(80, output)
}

#[test]
fn terminal_keyboard_extras_follow_content_and_skip_defaults() {
    use maestro_terminal::formatter::{Content, ScreenExtra, TerminalExtra};
    let terminal = terminal(80, "\x1b[>4;2m\x1b[=3;1uhello");
    let mut formatter = TerminalFormatter::init(&terminal, vt());
    formatter.content = Content::None;
    formatter.extra = TerminalExtra {
        keyboard: true,
        screen: ScreenExtra {
            kitty_keyboard: true,
        },
    };
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "\x1b[=3u\x1b[>4;2m");
    formatter.opts.emit = Format::Plain;
    output.clear();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "");
    let defaults = terminal_from_output("");
    formatter.terminal = &defaults;
    formatter.opts.emit = Format::Vt;
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "");
}

#[test]
fn vt_content_does_not_emit_per_cell_hyperlinks() {
    assert_eq!(
        page_output(
            80,
            "\x1b]8;;https://example.com\x1b\\hello\x1b]8;;\x1b\\",
            vt()
        ),
        "hello"
    );
}

#[test]
fn foreign_formatter_vt_content() {
    let terminal = terminal(80, "Test");
    let formatter = TerminalFormatter::init(&terminal, vt());
    let mut output = String::new();
    formatter.format(&mut output).unwrap();
    assert_eq!(output, "Test");
}

#[test]
fn style_vt_emits_all_attributes_and_colors() {
    use maestro_terminal::{Style, color::Rgb, sgr::Underline, style::Color};
    let mut style = Style {
        fg_color: Color::Rgb(Rgb { r: 1, g: 2, b: 3 }),
        bg_color: Color::Palette(4),
        underline_color: Color::Palette(5),
        ..Style::default()
    };
    style.flags.set_bold(true);
    style.flags.set_faint(true);
    style.flags.set_italic(true);
    style.flags.set_blink(true);
    style.flags.set_inverse(true);
    style.flags.set_invisible(true);
    style.flags.set_strikethrough(true);
    style.flags.set_overline(true);
    for (underline, code) in [
        (Underline::Single, "4"),
        (Underline::Double, "4:2"),
        (Underline::Curly, "4:3"),
        (Underline::Dotted, "4:4"),
        (Underline::Dashed, "4:5"),
    ] {
        style.flags.set_underline(underline);
        let mut output = String::new();
        style.format_vt(&mut output, None).unwrap();
        assert_eq!(
            output,
            format!(
                "\x1b[0m\x1b[1m\x1b[2m\x1b[3m\x1b[5m\x1b[7m\x1b[8m\x1b[9m\x1b[53m\x1b[{code}m\x1b[38;2;1;2;3m\x1b[48;5;4m\x1b[58;5;5m"
            )
        );
    }
}

#[test]
fn style_vt_resolves_palette() {
    use maestro_terminal::{Style, color::Rgb, style::Color};
    let palette = [Rgb {
        r: 10,
        g: 20,
        b: 30,
    }; 256];
    let style = Style {
        fg_color: Color::Palette(4),
        ..Style::default()
    };
    let mut output = String::new();
    style.format_vt(&mut output, Some(&palette)).unwrap();
    assert_eq!(output, "\x1b[0m\x1b[38;2;10;20;30m");
    assert_eq!(
        page_output(
            80,
            "\x1b[31mred",
            Options {
                palette: Some(&palette),
                ..vt()
            }
        ),
        "\x1b[0m\x1b[38;2;10;20;30mred\x1b[0m"
    );
}

#[test]
fn vt_wide_graphemes_and_soft_wrap_preserve_styles() {
    assert_eq!(
        page_output(
            4,
            "\x1b[1m界e\u{301}XY",
            Options {
                unwrap: true,
                ..vt()
            }
        ),
        "\x1b[0m\x1b[1m界e\u{301}XY\x1b[0m"
    );
}

#[test]
fn styled_extraction_replays_background_only_rows() {
    use maestro_terminal::render::RenderState;
    for input in [
        "A\r\n\x1b[44m\x1b[K\x1b[0m\r\nB",
        "A\r\nB\r\n\x1b[44m\x1b[K\x1b[0m",
    ] {
        let mut source = Terminal::new(4, 3, 100_000).unwrap();
        source.write(input.as_bytes());
        let mut output = String::new();
        TerminalFormatter::init(&source, vt())
            .format(&mut output)
            .unwrap();
        assert!(output.contains("\x1b[48;5;4m"), "{output:?}");
        let mut replay = Terminal::new(4, 3, 100_000).unwrap();
        replay.write(output.as_bytes());
        let mut expected = RenderState::default();
        let mut actual = RenderState::default();
        expected.update(&mut source);
        actual.update(&mut replay);
        for (expected, actual) in expected.rows().zip(actual.rows()) {
            for (expected, actual) in expected.cells().zip(actual.cells()) {
                assert_eq!(expected.background(), actual.background());
                assert_eq!(expected.foreground(), actual.foreground());
                // A replayed space and an erased blank paint the same cell.
                assert_eq!(
                    expected.graphemes_utf8().trim_end(),
                    actual.graphemes_utf8().trim_end()
                );
            }
        }
        let mut plain = String::new();
        TerminalFormatter::init(&source, Options::default())
            .format(&mut plain)
            .unwrap();
        assert_eq!(
            plain,
            if input.ends_with('B') {
                "A\n\nB"
            } else {
                "A\nB"
            }
        );
    }
    assert_eq!(page_output(4, "A\r\n\r\n", vt()), "A");
}

#[test]
fn page_vt_emits_background_only_content() {
    assert_eq!(
        page_output(4, "\x1b[44m\x1b[K\x1b[0m", vt()),
        "\x1b[0m\x1b[48;5;4m    \x1b[0m"
    );
}

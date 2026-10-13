//! Regression witnesses for the demonstration theme callbacks.
#![cfg(unix)]

#[path = "../examples/test_themes/color.rs"]
mod color;

#[test]
fn example_roles_use_the_embedded_dark_palette() {
    for (role, prefix) in [
        ("accent", "\x1b[38;2;217;160;102m"),
        ("muted", "\x1b[38;2;217;160;102m"),
        ("text", "\x1b[38;2;242;232;220m"),
        ("border", "\x1b[38;2;94;115;131m"),
    ] {
        let callback = color::color(role).unwrap();
        assert_eq!(callback("Accent!"), format!("{prefix}Accent!\x1b[39m"));
        assert_eq!(
            callback("雪 e\u{0301} 👩‍💻"),
            format!("{prefix}雪 e\u{0301} 👩‍💻\x1b[39m")
        );
    }
}

#[test]
fn example_absent_role_returns_invalid_data() {
    let error = color::color("absent-role").err().unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn example_decoration_reopens_matching_closes() {
    let accent = color::color("accent").unwrap();
    assert_eq!(
        accent("A\x1b[39mB\x1b[39mC"),
        "\x1b[38;2;217;160;102mA\x1b[38;2;217;160;102mB\x1b[38;2;217;160;102mC\x1b[39m"
    );
    for (open, close) in [
        ("\x1b[1m", "\x1b[22m"),
        ("\x1b[2m", "\x1b[22m"),
        ("\x1b[3m", "\x1b[23m"),
        ("\x1b[9m", "\x1b[29m"),
        ("\x1b[4m", "\x1b[24m"),
    ] {
        assert_eq!(
            color::decorate(&format!("A{close}B{close}C"), open, close),
            format!("{open}A{open}B{open}C{close}")
        );
    }
    assert_eq!(
        color::decorate("A\x1b[0mB", "\x1b[1m", "\x1b[22m"),
        "\x1b[1mA\x1b[0mB\x1b[22m"
    );
    assert_eq!(
        color::decorate(&accent("Left\x1b[22mRight"), "\x1b[1m", "\x1b[22m"),
        "\x1b[1m\x1b[38;2;217;160;102mLeft\x1b[1mRight\x1b[39m\x1b[22m"
    );
    for text in ["", "A\nB", "A\r\nB"] {
        assert_eq!(
            accent(text),
            format!("\x1b[38;2;217;160;102m{text}\x1b[39m")
        );
        assert_eq!(
            color::decorate(text, "\x1b[1m", "\x1b[22m"),
            format!("\x1b[1m{text}\x1b[22m")
        );
    }
}

#[path = "../examples/test_themes/default_editor_theme.rs"]
mod default_editor_theme;
#[path = "../examples/test_themes/default_markdown_theme.rs"]
mod default_markdown_theme;
#[path = "../examples/test_themes/default_select_list_theme.rs"]
mod default_select_list_theme;

/// Checks the selection mapping for either factory's independently built callbacks.
fn assert_selection(theme: &maestro_tui::SelectListTheme) {
    assert_eq!(
        (theme.selected_prefix)("Accent!"),
        "\x1b[38;2;217;160;102mAccent!\x1b[39m"
    );
    assert_eq!(
        (theme.selected_text)("Accent!"),
        "\x1b[1m\x1b[38;2;217;160;102mAccent!\x1b[39m\x1b[22m"
    );
    for callback in [&theme.description, &theme.scroll_info, &theme.no_match] {
        assert_eq!(
            callback("Accent!"),
            "\x1b[2m\x1b[38;2;217;160;102mAccent!\x1b[39m\x1b[22m"
        );
    }
}

#[test]
fn example_theme_callbacks_keep_branded_styles() {
    assert_selection(&default_select_list_theme::default_select_list_theme().unwrap());
    let editor = default_editor_theme::default_editor_theme().unwrap();
    assert_eq!(
        (editor.border_color)("Accent!"),
        "\x1b[2m\x1b[38;2;94;115;131mAccent!\x1b[39m\x1b[22m"
    );
    assert_selection(&editor.select_list);
    assert_markdown(&default_markdown_theme::default_markdown_theme().unwrap());
}

/// Checks the Markdown factory's foreground and emphasis callback mapping.
fn assert_markdown(markdown: &maestro_tui::MarkdownTheme) {
    assert_eq!(
        (markdown.heading)("Accent!"),
        "\x1b[1m\x1b[38;2;217;160;102mAccent!\x1b[39m\x1b[22m"
    );
    for callback in [&markdown.link, &markdown.code, &markdown.list_bullet] {
        assert_eq!(callback("Accent!"), "\x1b[38;2;217;160;102mAccent!\x1b[39m");
    }
    assert_eq!(
        (markdown.link_url)("Accent!"),
        "\x1b[2m\x1b[38;2;217;160;102mAccent!\x1b[39m\x1b[22m"
    );
    assert_eq!(
        (markdown.code_block)("Accent!"),
        "\x1b[38;2;242;232;220mAccent!\x1b[39m"
    );
    for callback in [
        &markdown.code_block_border,
        &markdown.quote_border,
        &markdown.hr,
    ] {
        assert_eq!(
            callback("Accent!"),
            "\x1b[2m\x1b[38;2;94;115;131mAccent!\x1b[39m\x1b[22m"
        );
    }
    assert_eq!(
        (markdown.quote)("Accent!"),
        "\x1b[3m\x1b[38;2;217;160;102mAccent!\x1b[39m\x1b[23m"
    );
    assert_eq!((markdown.bold)("Accent!"), "\x1b[1mAccent!\x1b[22m");
    assert_eq!((markdown.italic)("Accent!"), "\x1b[3mAccent!\x1b[23m");
    assert_eq!(
        (markdown.strikethrough)("Accent!"),
        "\x1b[9mAccent!\x1b[29m"
    );
    assert_eq!((markdown.underline)("Accent!"), "\x1b[4mAccent!\x1b[24m");
    assert!(markdown.highlight_code.is_none());
    assert!(markdown.code_block_indent.is_none());
}

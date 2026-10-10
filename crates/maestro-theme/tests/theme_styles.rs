//! Text decorations, captured border callbacks and retained component styles.
#[cfg(test)]
mod published;

use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, SyntaxHighlighter, SyntaxSpan, Theme, ThemeColor,
    ThemeOptions, get_editor_theme, get_markdown_theme, get_select_list_theme,
    get_settings_list_theme,
};
use maestro_tui::{
    Component, EditorTheme, Markdown, MarkdownOptions, MarkdownTheme, SelectListTheme,
    SettingsListTheme, TerminalImage,
};
use published::{published, theme, unpublished};
use serde_json::{Value, json};
use std::rc::Rc;

/// Fixture records whose members are exactly `keys`, so an unread field fails.
#[cfg(test)]
fn records(text: &str, keys: &[&str]) -> Vec<Value> {
    let records: Vec<Value> = serde_json::from_str(text).unwrap();
    for record in &records {
        let mut found: Vec<_> = record
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        found.sort_unstable();
        assert_eq!(found, keys);
    }
    records
}

/// Decoration fixture cases.
#[cfg(test)]
fn decorations() -> Vec<Value> {
    records(
        include_str!("fixtures/decorations.json"),
        &["enabled", "expected", "style", "text"],
    )
}

/// Apply a decoration by its fixture name.
#[cfg(test)]
fn decorate(style: &str, text: &str, enabled: bool) -> String {
    match style {
        "bold" => Theme::bold(text, enabled),
        "italic" => Theme::italic(text, enabled),
        "underline" => Theme::underline(text, enabled),
        "inverse" => Theme::inverse(text, enabled),
        "strikethrough" => Theme::strikethrough(text, enabled),
        other => panic!("unknown style {other}"),
    }
}

/// Check the cases selected by `select`, returning how many ran.
#[cfg(test)]
fn check(select: impl Fn(bool, &str) -> bool) -> usize {
    let cases: Vec<_> = decorations()
        .into_iter()
        .filter(|c| select(c["enabled"].as_bool().unwrap(), c["text"].as_str().unwrap()))
        .collect();
    for case in &cases {
        let (style, text) = (
            case["style"].as_str().unwrap(),
            case["text"].as_str().unwrap(),
        );
        let enabled = case["enabled"].as_bool().unwrap();
        assert_eq!(
            decorate(style, text, enabled),
            case["expected"].as_str().unwrap(),
            "{style} enabled={enabled} {text:?}"
        );
    }
    cases.len()
}

#[test]
fn decorations_keep_empty_and_disabled_text() {
    assert_eq!(check(|on, text| !on || text.is_empty()), 100);
    let literal = theme(0);
    assert_eq!(
        literal.fg(&ThemeColor::Accent, "x").unwrap(),
        "\x1b[38;5;1mx\x1b[39m"
    );
    assert_eq!(Theme::bold("x", true), "\x1b[1mx\x1b[22m");
}

#[test]
fn decorations_reopen_only_their_matching_close() {
    let plain = |on: bool, text: &str| on && !text.is_empty() && !text.contains(['\n', '\r']);
    assert_eq!(check(plain), 65);
}

#[test]
fn decorations_close_and_reopen_around_line_breaks() {
    let broken = |on: bool, text: &str| on && text.contains(['\n', '\r']);
    assert_eq!(check(broken), 25);
}

#[test]
fn border_callbacks_retain_their_prepared_instance() {
    let first = theme(0);
    let (state, _live) = published(&first);
    let cases = records(
        include_str!("fixtures/borders.json"),
        &["expected", "level"],
    );
    let callbacks: Vec<_> = cases
        .iter()
        .map(|case| match case["level"].as_str().unwrap() {
            "bash" => first.get_bash_mode_border_color(),
            level => first.get_thinking_border_color(level),
        })
        .collect();
    state.set_theme_instance(theme(100)).unwrap();
    for (case, callback) in cases.iter().zip(&callbacks) {
        assert_eq!(
            callback("border"),
            case["expected"].as_str().unwrap(),
            "{case}"
        );
    }
    assert_eq!(cases.len(), 9);
}

/// A highlighter that supports no label.
#[cfg(test)]
fn no_syntax() -> Rc<dyn SyntaxHighlighter> {
    struct NoSyntax;
    impl SyntaxHighlighter for NoSyntax {
        fn supports_language(&self, _language: &str) -> bool {
            false
        }

        fn highlight(
            &self,
            _code: &str,
            _language: &str,
        ) -> Result<Vec<SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Vec::new())
        }
    }
    Rc::new(NoSyntax)
}

/// The four component style records created from one live handle.
#[cfg(test)]
struct Helpers {
    markdown: MarkdownTheme,
    select: SelectListTheme,
    editor: EditorTheme,
    settings: SettingsListTheme,
}

/// Create every helper over `live`, with decorations enabled.
#[cfg(test)]
fn helpers(live: &LiveTheme) -> Helpers {
    Helpers {
        markdown: get_markdown_theme(live.clone(), no_syntax(), true),
        select: get_select_list_theme(live.clone()),
        editor: get_editor_theme(live.clone()),
        settings: get_settings_list_theme(live.clone()),
    }
}

/// Output of a select-list record for the text `x`.
#[cfg(test)]
fn select_fields(select: &SelectListTheme) -> Value {
    json!({
        "selectedPrefix": (select.selected_prefix)("x"),
        "selectedText": (select.selected_text)("x"),
        "description": (select.description)("x"),
        "scrollInfo": (select.scroll_info)("x"),
        "noMatch": (select.no_match)("x"),
    })
}

/// Output of the recorded style fields and the cursor, in the shape of the recorded fixture.
#[cfg(test)]
fn snapshot(h: &Helpers) -> Value {
    let (m, s) = (&h.markdown, &h.settings);
    json!({
        "markdown": {
            "heading": (m.heading)("x"), "link": (m.link)("x"), "linkUrl": (m.link_url)("x"),
            "code": (m.code)("x"), "codeBlock": (m.code_block)("x"),
            "codeBlockBorder": (m.code_block_border)("x"), "quote": (m.quote)("x"),
            "quoteBorder": (m.quote_border)("x"), "hr": (m.hr)("x"),
            "listBullet": (m.list_bullet)("x"), "bold": (m.bold)("x"),
            "italic": (m.italic)("x"), "underline": (m.underline)("x"),
            "strikethrough": (m.strikethrough)("x"),
        },
        "select": select_fields(&h.select),
        "editor": {
            "borderColor": (h.editor.border_color)("x"),
            "selectList": select_fields(&h.editor.select_list),
        },
        "settings": {
            "cursor": s.cursor, "description": (s.description)("x"), "hint": (s.hint)("x"),
            "label": [(s.label)("x", false), (s.label)("x", true)],
            "value": [(s.value)("x", false), (s.value)("x", true)],
        },
    })
}

#[test]
fn retained_component_fields_read_the_live_theme() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/live_styles.json")).unwrap();
    let (state, live) = published(&theme(0));
    let retained = helpers(&live);
    assert_eq!(snapshot(&retained), fixture["before"]);
    state.set_theme_instance(theme(100)).unwrap();
    assert_eq!(snapshot(&retained), fixture["after"]);
}

#[test]
fn settings_cursor_is_captured_while_callbacks_change() {
    let (state, live) = published(&theme(0));
    let retained = get_settings_list_theme(live.clone());
    state.set_theme_instance(theme(100)).unwrap();
    assert_eq!(retained.cursor, "\x1b[38;5;1m\u{2192} \x1b[39m");
    assert_eq!((retained.label)("x", true), "\x1b[38;5;101mx\x1b[39m");
    assert_eq!(
        get_settings_list_theme(live).cursor,
        "\x1b[38;5;101m\u{2192} \x1b[39m"
    );
}

#[test]
fn real_markdown_observes_callback_invalidation() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/real_markdown.json")).unwrap();
    let rows = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_str().unwrap().to_owned())
            .collect()
    };
    let width = usize::try_from(fixture["width"].as_u64().unwrap()).unwrap();
    let (state, live) = published(&theme(0));
    let options = MarkdownOptions {
        padding_x: 0,
        padding_y: 0,
        default_text_style: None,
    };
    let styles = Rc::new(get_markdown_theme(live, no_syntax(), true));
    let image = TerminalImage::new(|_| None, || 1);
    let text = fixture["text"].as_str().unwrap().to_owned();
    let markdown = Rc::new(Markdown::new(text, options, styles, image));
    assert_eq!(markdown.render(width), rows(&fixture["before"]));

    state.set_theme_instance(theme(100)).unwrap();
    assert_eq!(markdown.render(width), rows(&fixture["before"]));

    let invalidated = Rc::clone(&markdown);
    state.on_theme_change(Rc::new(move || {
        invalidated.invalidate();
        Ok(())
    }));
    state.set_theme_instance(theme(100)).unwrap();
    assert_eq!(markdown.render(width), rows(&fixture["after"]));
}

#[test]
fn uninitialized_component_callbacks_return_plain_text() {
    let state = unpublished();
    let h = helpers(&state.theme());
    let snap = snapshot(&h);
    let plain = |value: &Value| value.as_str() == Some("x");
    for group in ["markdown", "select"] {
        for (name, value) in snap[group].as_object().unwrap() {
            let styled = name == "strikethrough";
            assert_eq!(plain(value), !styled, "{group}.{name}");
        }
    }
    assert!(plain(&snap["editor"]["borderColor"]));
    assert_eq!(snap["settings"]["cursor"], "\u{2192} ");
    assert_eq!(snap["settings"]["label"], json!(["x", "x"]));
    assert_eq!(snap["settings"]["value"], json!(["x", "x"]));
}

#[test]
fn missing_color_callbacks_return_plain_text() {
    let (state, live) = published(&theme(0));
    let retained = helpers(&live);
    let partial = Theme::new(
        [(ThemeColor::Accent, ColorValue::Index(9))],
        [],
        ColorMode::Truecolor,
        ThemeOptions::default(),
    )
    .unwrap();
    state.set_theme_instance(Rc::new(partial)).unwrap();
    assert_eq!(
        (retained.select.selected_text)("x"),
        "\x1b[38;5;9mx\x1b[39m"
    );
    assert_eq!((retained.select.description)("x"), "x");
    assert_eq!((retained.markdown.heading)("x"), "x");
    assert_eq!((retained.markdown.bold)("x"), "\x1b[1mx\x1b[22m");
    assert_eq!(
        (retained.settings.value)("x", true),
        "\x1b[38;5;9mx\x1b[39m"
    );
    assert_eq!((retained.settings.value)("x", false), "x");
}

#[test]
fn retained_style_handles_release_after_last_owner() {
    let first = theme(0);
    let (first_gone, second) = (Rc::downgrade(&first), theme(100));
    let second_gone = Rc::downgrade(&second);
    let (state, live) = published(&first);
    let border = first.get_bash_mode_border_color();
    let retained = helpers(&live);
    let markdown = Rc::new(Markdown::new(
        String::new(),
        MarkdownOptions {
            padding_x: 0,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(get_markdown_theme(live.clone(), no_syntax(), true)),
        TerminalImage::new(|_| None, || 1),
    ));
    state.on_theme_change(Rc::new(move || {
        markdown.invalidate();
        Ok(())
    }));
    drop(first);
    state.set_theme_instance(second).unwrap();
    assert_eq!(border("x"), "\x1b[38;5;45mx\x1b[39m");
    assert!(first_gone.upgrade().is_some());
    drop(border);
    assert!(first_gone.upgrade().is_none());
    assert!(second_gone.upgrade().is_some());
    drop((retained, live, state));
    assert!(second_gone.upgrade().is_none());
}

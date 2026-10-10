//! Component style records whose color callbacks read the published theme when invoked.
use super::code::{colored_rows, highlight_rows};
use super::styles::paint;
use super::{LiveTheme, SyntaxHighlighter, Theme, ThemeColor};
use maestro_tui::{EditorTheme, MarkdownTheme, SelectListTheme, SettingsListTheme};
use std::rc::Rc;

/// Callback coloring text with `key` of the theme published at call time.
///
/// Strikethrough does no lookup and unselected settings labels are never colored.
fn foreground(live: LiveTheme, key: ThemeColor) -> impl Fn(&str) -> String + Clone {
    move |text| paint(&live, &key, text)
}

/// Shared form of [`foreground`].
fn shared(live: LiveTheme, key: ThemeColor) -> Rc<dyn Fn(&str) -> String> {
    Rc::new(foreground(live, key))
}

/// Markdown styles; decorations apply when `styles_enabled`, after a theme lookup.
///
/// Code blocks use `highlighter`; an engine failure colors their rows as code.
#[must_use]
pub fn get_markdown_theme(
    theme: LiveTheme,
    highlighter: Rc<dyn SyntaxHighlighter>,
    styles_enabled: bool,
) -> MarkdownTheme {
    let fg = |key| -> Box<dyn Fn(&str) -> String> { Box::new(foreground(theme.clone(), key)) };
    let decoration = |apply: fn(&str, bool) -> String| -> Box<dyn Fn(&str) -> String> {
        let live = theme.clone();
        Box::new(move |text| match live.get() {
            Ok(_) => apply(text, styles_enabled),
            Err(_) => text.to_owned(),
        })
    };
    MarkdownTheme {
        heading: fg(ThemeColor::MdHeading),
        link: fg(ThemeColor::MdLink),
        link_url: fg(ThemeColor::MdLinkUrl),
        code: fg(ThemeColor::MdCode),
        code_block: fg(ThemeColor::MdCodeBlock),
        code_block_border: fg(ThemeColor::MdCodeBlockBorder),
        quote: fg(ThemeColor::MdQuote),
        quote_border: fg(ThemeColor::MdQuoteBorder),
        hr: fg(ThemeColor::MdHr),
        list_bullet: fg(ThemeColor::MdListBullet),
        bold: decoration(Theme::bold),
        italic: decoration(Theme::italic),
        underline: decoration(Theme::underline),
        strikethrough: Box::new(move |text| Theme::strikethrough(text, styles_enabled)),
        highlight_code: Some(Box::new(move |code, lang| {
            highlight_rows(&theme, &*highlighter, (code, lang), colored_rows)
        })),
        code_block_indent: None,
    }
}

/// Select-list styles.
#[must_use]
pub fn get_select_list_theme(theme: LiveTheme) -> SelectListTheme {
    let muted = shared(theme.clone(), ThemeColor::Muted);
    let accent = shared(theme, ThemeColor::Accent);
    SelectListTheme {
        selected_prefix: Rc::clone(&accent),
        selected_text: accent,
        description: Rc::clone(&muted),
        scroll_info: Rc::clone(&muted),
        no_match: muted,
    }
}

/// Editor border style and select-list styles.
#[must_use]
pub fn get_editor_theme(theme: LiveTheme) -> EditorTheme {
    EditorTheme {
        border_color: shared(theme.clone(), ThemeColor::BorderMuted),
        select_list: get_select_list_theme(theme),
    }
}

/// Settings-list styles; the cursor is colored once, with the theme published now.
#[must_use]
pub fn get_settings_list_theme(theme: LiveTheme) -> SettingsListTheme {
    let cursor = paint(&theme, &ThemeColor::Accent, "\u{2192} ");
    let (label, value) = (theme.clone(), theme.clone());
    let dim = shared(theme, ThemeColor::Dim);
    SettingsListTheme {
        label: Rc::new(move |text, selected| {
            if selected {
                paint(&label, &ThemeColor::Accent, text)
            } else {
                text.to_owned()
            }
        }),
        value: Rc::new(move |text, selected| {
            let key = if selected {
                ThemeColor::Accent
            } else {
                ThemeColor::Muted
            };
            paint(&value, &key, text)
        }),
        description: Rc::clone(&dim),
        cursor,
        hint: dim,
    }
}

//! Markdown styles for demonstration callers.
use super::color::{color, decorate};
use maestro_tui::MarkdownTheme;
use std::io;
/// Owned foreground callback in the toolkit theme record.
type Foreground = Box<dyn Fn(&str) -> String>;
/// Markdown styles retain decorations while selecting semantic brand roles.
pub(super) fn default_markdown_theme() -> io::Result<MarkdownTheme> {
    let foreground = |role| -> io::Result<Foreground> {
        let callback = color(role)?;
        Ok(Box::new(move |text| callback(text)))
    };
    let faint = |role| -> io::Result<Foreground> {
        let callback = color(role)?;
        Ok(Box::new(move |text| {
            decorate(&callback(text), "\x1b[2m", "\x1b[22m")
        }))
    };
    let heading = color("accent")?;
    let quote = color("muted")?;
    Ok(MarkdownTheme {
        heading: Box::new(move |text| decorate(&heading(text), "\x1b[1m", "\x1b[22m")),
        link: foreground("accent")?,
        link_url: faint("muted")?,
        code: foreground("accent")?,
        code_block: foreground("text")?,
        code_block_border: faint("border")?,
        quote: Box::new(move |text| decorate(&quote(text), "\x1b[3m", "\x1b[23m")),
        quote_border: faint("border")?,
        hr: faint("border")?,
        list_bullet: foreground("accent")?,
        bold: Box::new(|text| decorate(text, "\x1b[1m", "\x1b[22m")),
        italic: Box::new(|text| decorate(text, "\x1b[3m", "\x1b[23m")),
        strikethrough: Box::new(|text| decorate(text, "\x1b[9m", "\x1b[29m")),
        underline: Box::new(|text| decorate(text, "\x1b[4m", "\x1b[24m")),
        highlight_code: None,
        code_block_indent: None,
    })
}

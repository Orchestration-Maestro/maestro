//! Supplied styles for Markdown component tests.
use maestro_tui::MarkdownTheme;
use std::fmt::Write;

/// A theme that leaves every input unchanged.
pub fn plain() -> MarkdownTheme {
    MarkdownTheme {
        heading: Box::new(str::to_owned),
        link: Box::new(str::to_owned),
        link_url: Box::new(str::to_owned),
        code: Box::new(str::to_owned),
        code_block: Box::new(str::to_owned),
        code_block_border: Box::new(str::to_owned),
        quote: Box::new(str::to_owned),
        quote_border: Box::new(str::to_owned),
        hr: Box::new(str::to_owned),
        list_bullet: Box::new(str::to_owned),
        bold: Box::new(str::to_owned),
        italic: Box::new(str::to_owned),
        strikethrough: Box::new(str::to_owned),
        underline: Box::new(str::to_owned),
        highlight_code: None,
        code_block_indent: None,
    }
}

/// Applies terminal styles with nested closing-code restoration.
pub fn paint(text: &str, codes: &[(u8, u8)]) -> String {
    if text.is_empty() {
        return String::new();
    }
    let opening = codes.iter().fold(String::new(), |mut output, (open, _)| {
        let _ = write!(output, "\x1b[{open}m");
        output
    });
    let closing = codes
        .iter()
        .rev()
        .fold(String::new(), |mut output, (_, close)| {
            let _ = write!(output, "\x1b[{close}m");
            output
        });
    let mut body = text.to_owned();
    for (open, close) in codes {
        body = body.replace(
            &format!("\x1b[{close}m"),
            &format!("\x1b[{close}m\x1b[{open}m"),
        );
    }
    body = body.replace('\n', &format!("{closing}\n{opening}"));
    format!("{opening}{body}{closing}")
}

/// The supplied ANSI theme used by layout witnesses.
pub fn ansi() -> MarkdownTheme {
    MarkdownTheme {
        heading: Box::new(|s| paint(s, &[(1, 22), (36, 39)])),
        link: Box::new(|s| paint(s, &[(34, 39)])),
        link_url: Box::new(|s| paint(s, &[(2, 22)])),
        code: Box::new(|s| paint(s, &[(33, 39)])),
        code_block: Box::new(|s| paint(s, &[(32, 39)])),
        code_block_border: Box::new(|s| paint(s, &[(2, 22)])),
        quote: Box::new(|s| paint(s, &[(3, 23)])),
        quote_border: Box::new(|s| paint(s, &[(2, 22)])),
        hr: Box::new(|s| paint(s, &[(2, 22)])),
        list_bullet: Box::new(|s| paint(s, &[(36, 39)])),
        bold: Box::new(|s| paint(s, &[(1, 22)])),
        italic: Box::new(|s| paint(s, &[(3, 23)])),
        strikethrough: Box::new(|s| paint(s, &[(9, 29)])),
        underline: Box::new(|s| paint(s, &[(4, 24)])),
        highlight_code: None,
        code_block_indent: None,
    }
}

/// One component invocation and its recorded rows.
#[derive(serde::Deserialize)]
pub struct Case {
    /// Authored Markdown.
    pub text: String,
    /// Requested viewport width.
    pub width: usize,
    /// Horizontal margin.
    pub padding_x: usize,
    /// Vertical padding.
    pub padding_y: usize,
    /// Supplied callback profile.
    pub profile: String,
    /// Hyperlink capability.
    pub hyperlinks: bool,
    /// Optional code indentation.
    pub indent: Option<String>,
    /// Expected ordered rows.
    pub lines: Vec<String>,
}

/// Recorded component invocations for a named behavior, from either corpus.
pub fn corpus(name: &str) -> Vec<Case> {
    let mut found = None;
    for json in [
        include_str!("../fixtures/markdown_core.json"),
        include_str!("../fixtures/markdown_tables.json"),
    ] {
        let decoded: Result<std::collections::BTreeMap<String, Vec<Case>>, _> =
            serde_json::from_str(json);
        assert!(
            decoded.is_ok(),
            "invalid fixture: {:?}",
            decoded.as_ref().err()
        );
        found = found.or_else(|| decoded.unwrap_or_default().remove(name));
    }
    assert!(found.is_some(), "no recorded cases for {name}");
    found.unwrap_or_default()
}

/// Builds the component a recorded case describes.
pub fn component(case: &Case) -> maestro_tui::Markdown {
    use maestro_tui::{Markdown, MarkdownOptions, TerminalCapabilities, TerminalImage};
    let mut theme = if case.profile == "plain" {
        plain()
    } else {
        ansi()
    };
    theme.code_block_indent.clone_from(&case.indent);
    let terminal = TerminalImage::new(|_| None, || 1);
    terminal.set_capabilities(TerminalCapabilities {
        hyperlinks: case.hyperlinks,
        ..TerminalCapabilities::default()
    });
    Markdown::new(
        case.text.clone(),
        MarkdownOptions {
            padding_x: case.padding_x,
            padding_y: case.padding_y,
            default_text_style: default_style(&case.profile),
        },
        std::rc::Rc::new(theme),
        terminal,
    )
}

/// Asserts every recorded component invocation for a named behavior.
pub fn cases(name: &str) {
    use maestro_tui::Component;
    for case in corpus(name) {
        let rows = component(&case).render(case.width);
        let suppressed = match case.profile.as_str() {
            "quote-magenta" => Some("\x1b[35m"),
            "quote-cyan" => Some("\x1b[36m"),
            "quote-yellow" => Some("\x1b[33m"),
            _ => None,
        };
        if let Some(color) = suppressed {
            assert!(
                rows.iter().all(|row| !row.contains(color)),
                "{name}: base foreground leaked"
            );
        }
        assert_eq!(
            rows, case.lines,
            "{name}: {:?}, width {}",
            case.text, case.width
        );
    }
}

/// Supplies the base style carried by a recorded callback profile.
fn default_style(profile: &str) -> Option<maestro_tui::DefaultTextStyle> {
    if profile == "thinking" {
        Some(maestro_tui::DefaultTextStyle {
            color: Some(Box::new(|s| paint(s, &[(90, 39)]))),
            decorations: vec![maestro_tui::TextDecoration::Italic],
            ..maestro_tui::DefaultTextStyle::default()
        })
    } else if profile == "default-style" {
        Some(maestro_tui::DefaultTextStyle {
            color: Some(Box::new(|s| format!("\x1b[90m{s}\x1b[39m"))),
            bg_color: Some(Box::new(|s| format!("\x1b[44m{s}\x1b[49m"))),
            decorations: vec![
                maestro_tui::TextDecoration::Bold,
                maestro_tui::TextDecoration::Italic,
                maestro_tui::TextDecoration::Strikethrough,
                maestro_tui::TextDecoration::Underline,
            ],
        })
    } else {
        let code = match profile {
            "quote-magenta" => Some(35),
            "quote-cyan" => Some(36),
            "quote-yellow" => Some(33),
            _ => None,
        };
        code.map(|code| maestro_tui::DefaultTextStyle {
            color: Some(Box::new(move |s| paint(s, &[(code, 39)]))),
            decorations: if code == 33 {
                vec![maestro_tui::TextDecoration::Italic]
            } else {
                vec![]
            },
            ..maestro_tui::DefaultTextStyle::default()
        })
    }
}

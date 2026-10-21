//! Parses Markdown with every caller grammar, then renders it as an assistant reply.
#![no_main]

use libfuzzer_sys::fuzz_target;
use maestro_markdown::{MarkedOptions, parse_with_options};
use maestro_tui::{Component, Markdown, MarkdownOptions, MarkdownTheme, TerminalImage};
use std::hint::black_box;
use std::rc::Rc;

/// The grammars the parser's callers select.
const GRAMMARS: [MarkedOptions; 5] = [
    MarkedOptions::Terminal,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Browser { escape_html: true },
    MarkedOptions::Export,
    MarkedOptions::Documentation,
];

/// A styling callback that leaves its text unchanged.
fn unstyled() -> Box<dyn Fn(&str) -> String> {
    Box::new(str::to_owned)
}

/// Callbacks that add no escapes, so rendering cost comes from layout alone.
fn theme() -> MarkdownTheme {
    MarkdownTheme {
        heading: unstyled(),
        link: unstyled(),
        link_url: unstyled(),
        code: unstyled(),
        code_block: unstyled(),
        code_block_border: unstyled(),
        quote: unstyled(),
        quote_border: unstyled(),
        hr: unstyled(),
        list_bullet: unstyled(),
        bold: unstyled(),
        italic: unstyled(),
        strikethrough: unstyled(),
        underline: unstyled(),
        highlight_code: None,
        code_block_indent: None,
    }
}

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    for options in GRAMMARS {
        black_box(parse_with_options(source.to_owned(), options));
    }
    // The assistant reply's settings; no environment, so links render without terminal hyperlinks.
    let reply = Markdown::new(
        source.to_owned(),
        MarkdownOptions {
            clean_content: true,
            padding_x: 1,
            padding_y: 0,
            default_text_style: None,
        },
        Rc::new(theme()),
        TerminalImage::new(|_| None, || 1),
    );
    black_box(reply.render(80));
});

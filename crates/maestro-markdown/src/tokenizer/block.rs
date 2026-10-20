//! Leaf block recognizers with authored ranges.
use super::{Block, capture};
use crate::{Kind, MarkedOptions, helpers::rtrim, rules, source::Source};
use maestro_text::is_whitespace_scalar;

/// Admit a non-container block in grammar order.
pub(crate) fn leaf(source: &Source<'_>) -> Option<Block> {
    fences(source)
        .or_else(|| heading(source))
        .or_else(|| rule(source))
}

/// Remove one indentation unit on every indented-code line.
pub(crate) fn code(source: &Source<'_>) -> Option<Block> {
    let matched = rules::BLOCK_CODE.as_ref()?.find(source.text)?;
    let pieces = crate::lexer::lines(&source.text[matched.range()]).map(|range| {
        let line = &source.text[range.clone()];
        let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(4);
        let prefix = if spaces < 4 && line.as_bytes().get(spaces) == Some(&b'\t') {
            spaces + 1
        } else {
            spaces
        };
        range.start + prefix..range.end
    });
    let body = source.select(pieces);
    Some(Block {
        consumed: matched.end(),
        node: source.node(
            matched.range(),
            Kind::CodeBlock(
                code_body(
                    rtrim(&body.text, '\n').to_owned(),
                    body.text.ends_with('\n'),
                ),
                None,
            ),
        ),
        inline: Some(body),
    })
}

/// Retain full fence information and compensate only the opening indentation.
fn fences(source: &Source<'_>) -> Option<Block> {
    let matched = rules::BLOCK_FENCES.as_ref()?.find(source.text)?;
    let indent = source.text.bytes().take_while(|byte| *byte == b' ').count();
    let body = capture(source.text, &matched, 3);
    let body: String = body
        .split_inclusive('\n')
        .map(|line| {
            let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
            &line[if spaces >= indent { indent } else { 0 }..]
        })
        .collect();
    let info = capture(source.text, &matched, 2).trim_matches(is_whitespace_scalar);
    Some(Block {
        consumed: matched.end(),
        node: source.node(
            matched.range(),
            Kind::CodeBlock(
                code_body(
                    body,
                    matched
                        .group(3)
                        .is_some_and(|range| source.text.as_bytes().get(range.end) == Some(&b'\n')),
                ),
                Some(super::inline::decode_operand(info)),
            ),
        ),
        inline: None,
    })
}

/// Select ATX content, removing only an eligible trailing hash run.
fn heading(source: &Source<'_>) -> Option<Block> {
    let matched = rules::BLOCK_HEADING.as_ref()?.find(source.text)?;
    let body = capture(source.text, &matched, 2);
    let mut text = body.trim_matches(is_whitespace_scalar);
    let hashes = rtrim(text, '#');
    if hashes.is_empty() || hashes.ends_with(' ') {
        text = hashes.trim_matches(is_whitespace_scalar);
    }
    let start = matched.group(2)?.start + body.find(text)?;
    Some(Block {
        consumed: matched.end(),
        node: source.node(
            matched.range(),
            Kind::Heading(
                capture(source.text, &matched, 1).len(),
                Vec::new(),
                text.to_owned(),
            ),
        ),
        inline: Some(source.select(std::iter::once(start..start + text.len()))),
    })
}

/// Keep horizontal-rule terminators available to block spacing.
fn rule(source: &Source<'_>) -> Option<Block> {
    let matched = rules::BLOCK_HR.as_ref()?.find(source.text)?;
    let consumed = rtrim(&source.text[matched.range()], '\n').len();
    Some(Block {
        consumed,
        node: source.node(0..consumed, Kind::Rule),
        inline: None,
    })
}

/// Raw HTML remains a block only for callers that permit it.
pub(crate) fn html(source: &Source<'_>, options: MarkedOptions) -> Option<Block> {
    if options == MarkedOptions::Export {
        return None;
    }
    if options == (MarkedOptions::Browser { escape_html: true }) {
        let html = super::html::block(source.text)?;
        return Some(Block {
            consumed: html.consumed,
            node: source.node(0..html.consumed, Kind::HtmlBlock(html.value)),
            inline: None,
        });
    }
    let matched = rules::BLOCK_HTML.as_ref()?.find(source.text)?;
    Some(Block {
        consumed: matched.end(),
        node: source.node(
            matched.range(),
            Kind::HtmlBlock(super::html::block_value(
                source.text[matched.range()].to_owned(),
            )),
        ),
        inline: None,
    })
}

mod list;

/// Preserve a physical code terminator without retaining following blank separators.
fn code_body(mut body: String, terminated: bool) -> String {
    if terminated {
        body.push('\n');
    }
    body
}

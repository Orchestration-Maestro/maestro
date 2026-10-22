//! Leaf block recognizers with authored ranges.
use super::{Block, capture};
use crate::{
    Kind, MarkedOptions,
    helpers::rtrim,
    rules,
    source::{Segmented, Source},
};
use maestro_text::is_whitespace_scalar;

/// Admit a non-container block in grammar order.
pub(crate) fn leaf(source: &Segmented, first: &Source<'_>) -> Option<Block> {
    fences(source)
        .or_else(|| heading(first))
        .or_else(|| rule(first))
}

/// Remove one indentation unit on every indented-code line.
pub(crate) fn code(source: &Segmented) -> Option<Block> {
    let consumed = rules::code::Recognizer::default().finish(source)?;
    let raw = source.select(0..consumed).materialize();
    let source = &raw.view(0..consumed);
    let pieces = crate::lexer::lines(&source.text[..consumed]).map(|range| {
        let prefix = rules::code::indentation(&source.text[range.clone()]);
        range.start + prefix..range.end
    });
    let body = source.select(pieces);
    Some(Block {
        consumed,
        node: source.node(
            0..consumed,
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
fn fences(source: &Segmented) -> Option<Block> {
    let matched = rules::fence::Recognizer::default().finish(source)?;
    let raw = source.select(0..matched.consumed).materialize();
    let source = &raw.view(0..raw.text.len());
    let indent = matched.indent;
    let body = matched
        .body
        .as_ref()
        .map_or("", |range| &source.text[range.clone()]);
    let body: String = body
        .split_inclusive('\n')
        .map(|line| {
            let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
            &line[if spaces >= indent { indent } else { 0 }..]
        })
        .collect();
    let info = source.text[matched.info.clone()].trim_matches(is_whitespace_scalar);
    Some(Block {
        consumed: matched.consumed,
        node: source.node(
            0..matched.consumed,
            Kind::CodeBlock(
                code_body(
                    body,
                    matched
                        .body
                        .as_ref()
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
    #[cfg(test)]
    crate::tests::block_operand(source.text);
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
    #[cfg(test)]
    crate::tests::block_operand(source.text);
    let matched = rules::BLOCK_HR.as_ref()?.find(source.text)?;
    let consumed = rtrim(&source.text[matched.range()], '\n').len();
    Some(Block {
        consumed,
        node: source.node(0..consumed, Kind::Rule),
        inline: None,
    })
}

/// Raw HTML remains a block only for callers that permit it.
pub(crate) fn html(source: &Segmented, options: MarkedOptions) -> Option<Block> {
    if options == MarkedOptions::Export {
        return None;
    }
    let (consumed, value) = if options == (MarkedOptions::Browser { escape_html: true }) {
        let html = super::html::BlockRecognizer::default().finish(source)?;
        (html.consumed, Some(html.value))
    } else {
        (rules::html::Recognizer::default().finish(source)?, None)
    };
    let raw = source.select(0..consumed).materialize();
    let source = &raw.view(0..consumed);
    let value = value.unwrap_or_else(|| super::html::block_value(source.text.to_owned()));
    Some(Block {
        consumed,
        node: source.node(0..consumed, Kind::HtmlBlock(value)),
        inline: None,
    })
}

mod list;
pub(crate) use list::ListState;

/// Preserve a physical code terminator without retaining following blank separators.
fn code_body(mut body: String, terminated: bool) -> String {
    if terminated {
        body.push('\n');
    }
    body
}

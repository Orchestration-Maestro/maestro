//! Leaf block recognizers with authored ranges.
use super::{Block, capture};
use crate::{Kind, MarkedOptions, helpers::rtrim, rules, source::Source};
use maestro_text::is_whitespace_scalar;
use std::ops::Range;

/// Admit a non-container block in grammar order.
pub(crate) fn leaf(source: &Source<'_>, options: MarkedOptions) -> Option<Block> {
    code(source)
        .or_else(|| fences(source))
        .or_else(|| heading(source))
        .or_else(|| rule(source))
        .or_else(|| html(source, options))
}

/// Continue an open paragraph with indented code or a definition, which cannot interrupt it.
///
/// Code lines keep their text without one indentation unit. Trailing blank lines are consumed
/// but stay out of the text, and they close the paragraph; the flag says it stays open.
pub(crate) fn continuation(source: &Source<'_>, options: MarkedOptions) -> Option<(Block, bool)> {
    let code = rules::BLOCK_CODE
        .as_ref()
        .and_then(|rule| rule.find(source.text));
    let matched = match &code {
        Some(matched) => matched.clone(),
        None => rules::block_def(options)?.find(source.text)?,
    };
    let content = crate::lexer::lines(&source.text[..matched.end()])
        .filter(|line| {
            !source.text[line.clone()]
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t' | b'\n'))
        })
        .last()?
        .end;
    let pieces = crate::lexer::lines(&source.text[..content]).map(|line| {
        let unit = if code.is_some() {
            indent(&source.text[line.clone()])
        } else {
            0
        };
        line.start + unit..line.end
    });
    let consumed = matched.end();
    let block = Block {
        consumed,
        node: source.node(0..consumed, Kind::Paragraph(Vec::new())),
        inline: Some(source.select(pieces)),
    };
    Some((block, content == consumed))
}

/// Inline pieces of list item paragraph text: a continuation line shaped like indented code
/// loses one indentation unit, as when item text continues with code line by line.
pub(crate) fn item_text(text: &str, range: Range<usize>) -> Vec<Range<usize>> {
    crate::lexer::lines(&text[range.clone()])
        .enumerate()
        .map(|(index, line)| {
            let line = range.start + line.start..range.start + line.end;
            let code = index > 0
                && rules::BLOCK_CODE
                    .as_ref()
                    .is_some_and(|rule| rule.find(&text[line.clone()]).is_some());
            line.start + if code { indent(&text[line.clone()]) } else { 0 }..line.end
        })
        .collect()
}

/// Bytes of one indentation unit: up to four spaces, or fewer spaces and a tab.
fn indent(line: &str) -> usize {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(4);
    if spaces < 4 && line.as_bytes().get(spaces) == Some(&b'\t') {
        spaces + 1
    } else {
        spaces
    }
}

/// Remove one indentation unit on every indented-code line.
fn code(source: &Source<'_>) -> Option<Block> {
    let matched = rules::BLOCK_CODE.as_ref()?.find(source.text)?;
    let body: String = source.text[matched.range()]
        .split_inclusive('\n')
        .map(|line| &line[indent(line)..])
        .collect();
    Some(Block {
        consumed: matched.end(),
        node: source.node(
            matched.range(),
            Kind::CodeBlock(
                code_body(rtrim(&body, '\n').to_owned(), body.ends_with('\n')),
                None,
            ),
        ),
        inline: None,
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
fn html(source: &Source<'_>, options: MarkedOptions) -> Option<Block> {
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

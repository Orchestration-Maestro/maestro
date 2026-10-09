//! Typed native Markdown nodes with authored source ranges.
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Options, Parser, Tag};
use std::ops::Range;

/// A native node together with its source extent.
pub(super) struct Node {
    /// Semantic content.
    pub kind: Kind,
    /// Extent in the normalized source.
    pub range: Range<usize>,
}
/// Block and inline structures admitted by the configured parser.
pub(super) enum Kind {
    /// Ordinary paragraph children.
    Paragraph(Vec<Node>),
    /// Heading level and inline children.
    Heading(usize, Vec<Node>, String),
    /// Ordered start or unordered list with its item containers.
    List(Option<u64>, Vec<Node>),
    /// Ordered blocks belonging to an item.
    Item(Vec<Node>),
    /// Literal HTML block contents.
    HtmlBlock(String),
    /// Literal inline HTML.
    Html(String),
    /// Horizontal rule.
    Rule,
    /// Authored blank gap.
    Gap,
    /// Quote children and authored content without quote markers.
    Quote(Vec<Node>, String),
    /// Decoded text.
    Text(String),
    /// Code text and optional full fence information.
    CodeBlock(String, Option<String>),
    /// Styled children, authored label and renderer-facing destination.
    Link(Vec<Node>, String, String),
    /// Authored image label, retained without inline formatting.
    Image(String),
    /// Strong child span.
    Strong(Vec<Node>),
    /// Deleted child span.
    Strike(Vec<Node>),
    /// Emphasized child span.
    Emphasis(Vec<Node>),
    /// Inline code text.
    Code(String),
    /// A visible line break.
    Break,
}
/// Normalizes text before native parsing.
pub(super) fn parse(text: &str) -> Vec<Node> {
    let source = text
        .replace('\t', "   ")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "\u{fffd}");
    let mut events = Parser::new_ext(
        &source,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
    .into_offset_iter();
    gaps(
        children(&mut events, &source, false),
        &source,
        0..source.len(),
    )
}
/// Consumes the current native container without rebuilding its source.
fn children<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &str,
    blocked: bool,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    while let Some((event, range)) = events.next() {
        let kind = match event {
            Event::Start(tag) => container(tag, events, source, blocked, &range),
            Event::Html(text) | Event::InlineHtml(text) => Kind::Html(text.into_string()),
            Event::Rule => Kind::Rule,
            Event::Text(text) => Kind::Text(text.into_string()),
            Event::Code(text) => Kind::Code(text.into_string()),
            Event::SoftBreak => Kind::Text("\n".to_owned()),
            Event::HardBreak => Kind::Break,
            Event::End(_) => break,
            _ => continue,
        };
        nodes.push(Node { kind, range });
    }
    if blocked { nodes } else { runs(nodes, source) }
}
/// Retains blank source gaps around top-level blocks.
fn gaps(nodes: Vec<Node>, source: &str, extent: Range<usize>) -> Vec<Node> {
    let mut result = Vec::new();
    let mut end = extent.start;
    for node in nodes {
        if source[end..node.range.start].contains('\n') {
            result.push(Node {
                kind: Kind::Gap,
                range: end..node.range.start,
            });
        }
        end = node.range.end;
        if matches!(node.kind, Kind::List(..)) {
            let trimmed = source[node.range.clone()].trim_end_matches(['\n', ' ', '\t']);
            end = (node.range.start + trimmed.len() + 1).min(end);
        }
        result.push(node);
    }
    if source[end..extent.end].contains('\n') {
        result.push(Node {
            kind: Kind::Gap,
            range: end..extent.end,
        });
    }
    result
}

/// Retrieves authored label markup from the native child extents.
fn label(nodes: &[Node], source: &str) -> String {
    match (nodes.first(), nodes.last()) {
        (Some(first), Some(last)) => source[first.range.start..last.range.end]
            .replace("\\[", "[")
            .replace("\\]", "]"),
        _ => String::new(),
    }
}

/// Scans only contiguous eligible text children of the same container.
fn runs(nodes: Vec<Node>, source: &str) -> Vec<Node> {
    let mut result = Vec::new();
    let mut parts = Vec::new();
    for node in nodes {
        if matches!(node.kind, Kind::Text(_)) {
            parts.push(node);
        } else {
            result.extend(super::autolinks::extend(std::mem::take(&mut parts), source));
            result.push(node);
        }
    }
    result.extend(super::autolinks::extend(parts, source));
    result
}

/// Consumes one structured native tag.
fn container<'a>(
    tag: Tag<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &str,
    blocked: bool,
    range: &Range<usize>,
) -> Kind {
    match tag {
        Tag::Link {
            link_type,
            dest_url,
            ..
        } => {
            let children = children(events, source, true);
            let href = if link_type == LinkType::Email {
                format!("mailto:{dest_url}")
            } else {
                dest_url.into_string()
            };
            let authored = label(&children, source);
            Kind::Link(children, authored, href)
        }
        Tag::Image { .. } => Kind::Image(label(&children(events, source, true), source)),
        Tag::Strong => Kind::Strong(children(events, source, blocked)),
        Tag::Strikethrough => Kind::Strike(children(events, source, blocked)),
        Tag::Emphasis => Kind::Emphasis(children(events, source, blocked)),
        Tag::BlockQuote(_) => {
            let children = children(events, source, blocked);
            let extent = children
                .first()
                .zip(children.last())
                .map_or(range.start..range.start, |(first, last)| {
                    first.range.start..last.range.end
                });
            let children = gaps(children, source, extent);
            let authored = source[range.clone()]
                .lines()
                .map(|line| {
                    let line = line.trim_start_matches(' ');
                    line.strip_prefix('>')
                        .map_or(line, |line| line.strip_prefix(' ').unwrap_or(line))
                })
                .collect::<Vec<_>>()
                .join("\n");
            Kind::Quote(children, authored)
        }
        Tag::Heading { level, .. } => {
            let children = children(events, source, blocked);
            let authored = label(&children, source);
            Kind::Heading(level as usize, children, authored)
        }
        Tag::CodeBlock(info) => code_block(info, events, source),
        Tag::HtmlBlock => Kind::HtmlBlock(
            children(events, source, blocked)
                .into_iter()
                .filter_map(|node| match node.kind {
                    Kind::Html(text) => Some(text),
                    _ => None,
                })
                .collect(),
        ),
        Tag::List(start) => Kind::List(start, children(events, source, blocked)),
        Tag::Item => Kind::Item(children(events, source, blocked)),
        _ => Kind::Paragraph(children(events, source, blocked)),
    }
}

/// Retains native code information and removes the parser's terminal newline.
fn code_block<'a>(
    info: CodeBlockKind<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &str,
) -> Kind {
    let text: String = children(events, source, true)
        .into_iter()
        .filter_map(|node| match node.kind {
            Kind::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    Kind::CodeBlock(
        text.strip_suffix('\n').unwrap_or(&text).to_owned(),
        match info {
            CodeBlockKind::Indented => None,
            CodeBlockKind::Fenced(info) => Some(info.into_string()),
        },
    )
}

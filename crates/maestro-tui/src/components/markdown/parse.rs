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
/// Authored source and the native containers enclosing the current events.
struct Source<'a> {
    /// Normalized original input.
    text: &'a str,
    /// Container prefixes removed by the native parser on continuation lines.
    frame: Option<&'a Frame<'a>>,
}
/// One enclosing native block's continuation prefix.
struct Frame<'a> {
    /// Outer block, stripped before this block.
    parent: Option<&'a Frame<'a>>,
    /// Quote marker or list continuation indentation.
    kind: Prefix,
}
/// Native block framing relevant to authored inline spans.
enum Prefix {
    /// One quote marker with its optional following space.
    Quote,
    /// List marker width on the item's first line.
    Item(usize),
}
impl Frame<'_> {
    /// Removes only prefixes belonging to the captured container path.
    fn strip<'a>(&self, line: &'a str) -> &'a str {
        let line = self.parent.map_or(line, |parent| parent.strip(line));
        match self.kind {
            Prefix::Quote => strip_quote(line),
            Prefix::Item(width) => {
                let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
                &line[spaces.min(width)..]
            }
        }
    }
}
impl Source<'_> {
    /// Retains authored bytes while removing enclosing continuation prefixes.
    fn authored(&self, range: Range<usize>) -> String {
        self.text[range]
            .split('\n')
            .enumerate()
            .map(|(index, line)| {
                if index == 0 {
                    line
                } else {
                    self.frame.map_or(line, |frame| frame.strip(line))
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
/// Removes a native quote prefix, leaving lazy continuation text intact.
fn strip_quote(line: &str) -> &str {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count().min(3);
    line[spaces..]
        .strip_prefix('>')
        .map_or(line, |line| line.strip_prefix(' ').unwrap_or(line))
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
        children(
            &mut events,
            &Source {
                text: &source,
                frame: None,
            },
            false,
        ),
        &source,
        0..source.len(),
    )
}
/// Consumes the current native container without rebuilding its source.
fn children<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
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
    if blocked {
        nodes
    } else {
        runs(nodes, source.text)
    }
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

/// Retrieves authored labels using native delimiters or heading child bounds.
fn label(nodes: &[Node], source: &Source<'_>, range: &Range<usize>, opener: usize) -> String {
    let Some(last) = nodes.last() else {
        return String::new();
    };
    let start = if opener > 0 {
        range.start + opener
    } else {
        let mut start = nodes[0].range.start;
        while start > range.start && source.text.as_bytes()[start - 1] == b'\\' {
            start -= 1;
        }
        start
    };
    source
        .authored(start..last.range.end)
        .replace("\\[", "[")
        .replace("\\]", "]")
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
    source: &Source<'_>,
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
            let authored = label(&children, source, range, 1);
            Kind::Link(children, authored, href)
        }
        Tag::Image { .. } => Kind::Image(label(&children(events, source, true), source, range, 2)),
        Tag::Strong => Kind::Strong(children(events, source, blocked)),
        Tag::Strikethrough => Kind::Strike(children(events, source, blocked)),
        Tag::Emphasis => Kind::Emphasis(children(events, source, blocked)),
        Tag::BlockQuote(_) => quote(events, source, blocked, range),
        Tag::Heading { level, .. } => {
            let children = children(events, source, blocked);
            let authored = label(&children, source, range, 0);
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
        Tag::Item => item(events, source, blocked, range),
        _ => Kind::Paragraph(children(events, source, blocked)),
    }
}

/// Captures quote framing for child labels and the literal item fallback.
fn quote<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    blocked: bool,
    range: &Range<usize>,
) -> Kind {
    let frame = Frame {
        parent: source.frame,
        kind: Prefix::Quote,
    };
    let nested = Source {
        text: source.text,
        frame: Some(&frame),
    };
    let children = children(events, &nested, blocked);
    let extent = children
        .first()
        .zip(children.last())
        .map_or(range.start..range.start, |(first, last)| {
            first.range.start..last.range.end
        });
    let children = gaps(children, source.text, extent);
    let authored = source
        .authored(range.clone())
        .lines()
        .map(strip_quote)
        .collect::<Vec<_>>()
        .join("\n");
    Kind::Quote(children, authored)
}

/// Captures list continuation framing from the native item marker.
fn item<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    blocked: bool,
    range: &Range<usize>,
) -> Kind {
    let raw = &source.text[range.clone()];
    let leading = raw.bytes().take_while(|byte| *byte == b' ').count();
    let marker = leading
        + raw[leading..]
            .bytes()
            .take_while(|byte| !byte.is_ascii_whitespace())
            .count();
    let spaces = raw[marker..]
        .bytes()
        .take_while(|byte| *byte == b' ')
        .count();
    let frame = Frame {
        parent: source.frame,
        kind: Prefix::Item(marker + if (1..=4).contains(&spaces) { spaces } else { 1 }),
    };
    let nested = Source {
        text: source.text,
        frame: Some(&frame),
    };
    Kind::Item(children(events, &nested, blocked))
}

/// Retains native code information and removes the parser's terminal newline.
fn code_block<'a>(
    info: CodeBlockKind<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
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

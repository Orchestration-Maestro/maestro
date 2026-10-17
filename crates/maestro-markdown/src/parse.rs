//! Native event adaptation with retained source operands.
use crate::{
    source::{Frame, Prefix, Source, strip_quote},
    types::{Cell, Cells, Document, ImageMetadata, Kind, LinkMetadata, Node, Table, Text},
};
use pulldown_cmark::{
    BrokenLink, CodeBlockKind, CowStr, Event, LinkType, Options, Parser, Tag, TagEnd,
};
use std::ops::Range;
/// Parse caller-normalized owned source into native structure.
#[must_use]
pub fn parse(source: String) -> Document {
    parse_using(source, None::<fn(&str) -> Option<(String, String)>>)
}
/// Parse a fragment using the complete document's resolved reference metadata.
#[must_use]
pub fn parse_with_references(source: String, document: &Document) -> Document {
    let references = crate::references::collect(&document.nodes);
    parse_using(source, Some(move |id: &str| references.get(id).cloned()))
}
/// Adapt the one native parser, optionally resolving document-wide references.
fn parse_using(
    source: String,
    mut callback: Option<impl FnMut(&str) -> Option<(String, String)>>,
) -> Document {
    let parser = Parser::new_with_broken_link_callback(
        &source,
        Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES,
        Some(move |link: BrokenLink<'_>| {
            callback.as_mut()?(link.reference.as_ref())
                .map(|(href, title)| (CowStr::from(href), CowStr::from(title)))
        }),
    );
    let mut events = parser.into_offset_iter();
    let nodes = gaps(
        children(
            &mut events,
            &Source {
                text: &source,
                frame: None,
                cell: false,
            },
        ),
        &Source {
            text: &source,
            frame: None,
            cell: false,
        },
        0..source.len(),
    );
    Document { source, nodes }
}
/// Consumes the current native container without rebuilding its source.
fn children<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    while let Some((event, mut range)) = events.next() {
        let kind = match event {
            Event::Start(tag) => container(tag, events, source, &range),
            Event::Html(text) | Event::InlineHtml(text) => {
                Kind::Html(source.spelling(text.into_string()))
            }
            Event::Rule => Kind::Rule,
            Event::Text(text) => {
                if text
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_punctuation)
                    && source.text[..range.start]
                        .bytes()
                        .rev()
                        .take_while(|byte| *byte == b'\\')
                        .count()
                        % 2
                        == 1
                {
                    range.start -= 1;
                }
                Kind::Text(Text {
                    decoded: text.into_string(),
                    authored: source.authored(range.clone()),
                })
            }
            Event::Code(text) => Kind::Code(text.into_string()),
            Event::SoftBreak => Kind::Text(Text {
                decoded: "\n".to_owned(),
                authored: "\n".to_owned(),
            }),
            Event::HardBreak => Kind::Break,
            Event::TaskListMarker(checked) => Kind::TaskListMarker(checked),
            Event::End(_) => break,
            _ => continue,
        };
        nodes.push(Node {
            kind,
            authored: source.authored(range.clone()),
            range,
        });
    }
    nodes
}
/// Retains blank source gaps around top-level blocks.
fn gaps(nodes: Vec<Node>, source: &Source<'_>, extent: Range<usize>) -> Vec<Node> {
    if nodes.is_empty() {
        return nodes;
    }
    let mut result = Vec::new();
    let mut end = extent.start;
    for node in nodes {
        if blank_gap(source, end..node.range.start) {
            result.push(Node {
                kind: Kind::Gap,
                authored: source.authored(end..node.range.start),
                range: end..node.range.start,
            });
        }
        end = node.range.end;
        if matches!(node.kind, Kind::Table(_)) {
            end += source.text[end..extent.end]
                .bytes()
                .take_while(|byte| *byte == b'\n')
                .count();
        }
        if matches!(node.kind, Kind::List(..)) {
            let trimmed = source.text[node.range.clone()].trim_end_matches(['\n', ' ', '\t']);
            end = (node.range.start + trimmed.len() + 1).min(end);
        }
        result.push(node);
    }
    if blank_gap(source, end..extent.end) {
        result.push(Node {
            kind: Kind::Gap,
            authored: source.authored(end..extent.end),
            range: end..extent.end,
        });
    }
    result
}

/// Classifies uncovered native lines after removing their enclosing prefixes.
fn blank_gap(source: &Source<'_>, range: Range<usize>) -> bool {
    source.text[range].split_inclusive('\n').any(|line| {
        let authored = source.frame.map_or(line, |frame| frame.strip(line));
        authored.ends_with('\n') && authored.chars().all(char::is_whitespace)
    })
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
    let authored = source.authored(start..last.range.end);
    if opener == 0 {
        authored
    } else {
        authored.replace("\\[", "[").replace("\\]", "]")
    }
}

/// Spells a table cell's `\|` inside an autolink's displayed text, which the parser leaves raw.
fn spell_text(node: &mut Node, source: &Source<'_>) {
    if let Kind::Text(text) = &mut node.kind {
        text.decoded = source.spelling(std::mem::take(&mut text.decoded));
    }
}

/// Consumes one structured native tag.
fn container<'a>(
    tag: Tag<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    range: &Range<usize>,
) -> Kind {
    match tag {
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => link(
            LinkMetadata {
                title: title.into_string(),
                link_type,
                id: id.into_string(),
            },
            dest_url,
            events,
            source,
            range,
        ),
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => image(
            ImageMetadata {
                destination: dest_url.into_string(),
                title: title.into_string(),
                link_type,
                id: id.into_string(),
                children: Vec::new(),
            },
            events,
            source,
            range,
        ),
        Tag::Strong => Kind::Strong(children(events, source)),
        Tag::Strikethrough => Kind::Strike(children(events, source)),
        Tag::Emphasis => Kind::Emphasis(children(events, source)),
        Tag::BlockQuote(_) => quote(events, source, range),
        Tag::Heading { level, .. } => {
            let children = children(events, source);
            let authored = label(&children, source, range, 0);
            Kind::Heading(level as usize, children, authored)
        }
        Tag::CodeBlock(info) => code_block(info, events, source),
        Tag::HtmlBlock => Kind::HtmlBlock(
            children(events, source)
                .into_iter()
                .filter_map(|node| match node.kind {
                    Kind::Html(text) => Some(text),
                    _ => None,
                })
                .collect(),
        ),
        Tag::List(start) => Kind::List(start, children(events, source)),
        Tag::Item => item(events, source, range),
        Tag::Table(alignments) => table(events, source, range, alignments),
        _ => Kind::Paragraph(children(events, source)),
    }
}

/// Captures quote framing for child labels and the literal item fallback.
fn quote<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    range: &Range<usize>,
) -> Kind {
    let frame = Frame {
        parent: source.frame,
        kind: Prefix::Quote,
    };
    let nested = Source {
        text: source.text,
        frame: Some(&frame),
        cell: false,
    };
    let children = children(events, &nested);
    let extent = children
        .first()
        .zip(children.last())
        .map_or(range.start..range.start, |(first, last)| {
            first.range.start..last.range.end
        });
    let children = gaps(children, &nested, extent);
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
        cell: false,
    };
    Kind::Item(tight(children(events, &nested), &nested))
}

/// Retains complete native code information and body.
fn code_block<'a>(
    info: CodeBlockKind<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
) -> Kind {
    let text: String = children(events, source)
        .into_iter()
        .filter_map(|node| match node.kind {
            Kind::Text(text) => Some(text.decoded),
            _ => None,
        })
        .collect();
    Kind::CodeBlock(
        text,
        match info {
            CodeBlockKind::Indented => None,
            CodeBlockKind::Fenced(info) => Some(info.into_string()),
        },
    )
}

/// Collects header and body cells with native alignment and authored ranges.
fn table<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    range: &Range<usize>,
    alignments: Vec<pulldown_cmark::Alignment>,
) -> Kind {
    let mut rows: Vec<Cells> = Vec::new();
    while let Some((event, range)) = events.next() {
        match event {
            Event::Start(Tag::TableHead | Tag::TableRow) => rows.push(Vec::new()),
            Event::Start(Tag::TableCell) => {
                let cell = children(
                    events,
                    &Source {
                        cell: true,
                        ..*source
                    },
                );
                if let Some(row) = rows.last_mut() {
                    row.push(Cell {
                        children: cell,
                        authored: Source {
                            cell: true,
                            ..*source
                        }
                        .authored(range.clone()),
                        range,
                    });
                }
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    let mut rows = rows.into_iter();
    let header = rows.next().unwrap_or_default();
    let line = &source.text[source.text[..range.start]
        .rfind('\n')
        .map_or(0, |index| index + 1)..range.start];
    let lead = source.frame.map_or(line, |frame| frame.strip(line));
    let indent = if lead.bytes().all(|byte| byte == b' ') {
        lead
    } else {
        ""
    };
    let authored = source.authored(range.clone());
    Kind::Table(Table {
        header,
        alignments,
        rows: rows.collect(),
        authored: format!(
            "{indent}{}",
            authored.strip_suffix('\n').unwrap_or(&authored)
        ),
    })
}

/// Retain complete link metadata while preserving native destination decoding.
fn link<'a>(
    meta: LinkMetadata,
    dest_url: pulldown_cmark::CowStr<'a>,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    range: &Range<usize>,
) -> Kind {
    let mut children = children(events, source);
    let mut href = if meta.link_type == LinkType::Email {
        format!("mailto:{dest_url}")
    } else {
        dest_url.into_string()
    };
    if meta.link_type == LinkType::Autolink {
        for node in &mut children {
            spell_text(node, source);
        }
        href = source.spelling(href);
    }
    let authored = label(&children, source, range, 1);
    Kind::Link(children, authored, href, meta)
}
/// Retain all image operands beside the authored terminal fallback.
fn image<'a>(
    mut meta: ImageMetadata,
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    source: &Source<'_>,
    range: &Range<usize>,
) -> Kind {
    meta.children = children(events, source);
    let authored = label(&meta.children, source, range, 2);
    Kind::Image(authored, meta)
}

/// Group tight item spans once so other renderers never rebuild container framing.
fn tight(nodes: Vec<Node>, source: &Source<'_>) -> Vec<Node> {
    let mut output = Vec::new();
    let mut pending = Vec::new();
    for node in nodes {
        if matches!(
            node.kind,
            Kind::Text(_)
                | Kind::Code(_)
                | Kind::Break
                | Kind::Strong(_)
                | Kind::Strike(_)
                | Kind::Emphasis(_)
                | Kind::Link(..)
                | Kind::Image(..)
                | Kind::Html(_)
        ) {
            pending.push(node);
        } else {
            flush(&mut output, &mut pending, source);
            output.push(node);
        }
    }
    flush(&mut output, &mut pending, source);
    output
}
/// Store the whole authored inline range, including formatting delimiters.
fn flush(output: &mut Vec<Node>, pending: &mut Vec<Node>, source: &Source<'_>) {
    let Some(first) = pending.first() else {
        return;
    };
    let Some(last) = pending.last() else {
        return;
    };
    let range = first.range.start..last.range.end;
    output.push(Node {
        authored: source.authored(range.clone()),
        range,
        kind: Kind::Inline(std::mem::take(pending)),
    });
}

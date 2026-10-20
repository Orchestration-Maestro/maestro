//! Structured destinations and original-document reference lookup.
use super::{Tokenizer, decode_operand};
use crate::{
    ImageMetadata, Kind, LinkMetadata, LinkType, Text,
    helpers::{ClosingBracket, find_closing_bracket},
    lexer::{reference_id, reference_key},
    rules,
    tokenizer::capture,
};
use std::ops::Range;

/// Source-local operands selected before recursive label interpretation.
struct Link {
    /// Complete authored construct length.
    consumed: usize,
    /// Label, excluding enclosing brackets.
    label: Range<usize>,
    /// Decoded destination.
    destination: String,
    /// Decoded tooltip.
    title: String,
    /// Selected reference class.
    kind: LinkType,
    /// Original reference identity.
    id: String,
}
impl Tokenizer<'_> {
    /// Resolve a link/image without rebuilding its document or definition index.
    pub(super) fn link(
        &mut self,
        range: Range<usize>,
        atoms: &[Range<usize>],
    ) -> Option<(usize, Kind)> {
        let text = &self.source.text[range.clone()];
        let link = self.inline_link(text).or_else(|| self.reference(text))?;
        let label = range.start + link.label.start..range.start + link.label.end;
        let label_source = self.source.select(std::iter::once(label));
        let source = label_source.brackets();
        let atoms: Vec<_> = atoms
            .iter()
            .filter_map(|atom| source.local(&self.source.extent(atom.clone())))
            .collect();
        let children = Tokenizer::new(
            &source,
            self.options,
            self.definitions,
            self.scopes.as_deref_mut(),
            true,
        )
        .parse(&atoms);
        if !text.starts_with('!') && contains_link(&children) {
            return None;
        }
        let authored = source.text;
        let kind = if text.starts_with('!') {
            Kind::Image(
                authored,
                ImageMetadata {
                    destination: link.destination,
                    title: link.title,
                    link_type: link.kind,
                    id: link.id,
                    children,
                },
            )
        } else {
            let native = self.source.extent(range.start..range.start + link.consumed);
            if let Some(scopes) = self.scopes.as_mut() {
                scopes.insert((native.start, native.end, true), label_source);
            }
            Kind::Link(
                children,
                authored,
                link.destination,
                LinkMetadata {
                    title: link.title,
                    link_type: link.kind,
                    id: link.id,
                },
            )
        };
        Some((link.consumed, kind))
    }

    /// Balance a direct destination and retain the selected title boundary.
    fn inline_link(&self, text: &str) -> Option<Link> {
        let matched = rules::inline_link(self.options)?.find(text)?;
        let mut href = capture(text, &matched, 2).trim_matches(maestro_text::is_whitespace_scalar);
        let mut consumed = matched.end();
        let mut title = capture(text, &matched, 3);
        if href.starts_with('<') {
            href = href.strip_prefix('<')?.strip_suffix('>')?;
            if href.bytes().rev().take_while(|byte| *byte == b'\\').count() % 2 != 0 {
                return None;
            }
        } else {
            match find_closing_bracket(href, ('(', ')'), self.options) {
                ClosingBracket::Unclosed => return None,
                ClosingBracket::Found(end) => {
                    consumed = matched.group(2)?.start + end + 1;
                    href = &href[..end];
                    title = "";
                }
                ClosingBracket::Missing => {}
            }
        }
        Some(Link {
            consumed,
            label: matched.group(1)?,
            destination: decode_operand(href),
            title: decode_operand(title.get(1..title.len().saturating_sub(1)).unwrap_or("")),
            kind: LinkType::Inline,
            id: String::new(),
        })
    }

    /// Resolve explicit, collapsed and shortcut references using the original index.
    fn reference(&self, text: &str) -> Option<Link> {
        let explicit = rules::inline_reflink(self.options)?.find(text);
        let matched = explicit
            .clone()
            .or_else(|| rules::inline_nolink(self.options)?.find(text))?;
        let label = capture(text, &matched, 1);
        let identity = if explicit.is_some() {
            capture(text, &matched, 2)
        } else {
            label
        };
        let id = reference_id(identity);
        let definition = self.definitions.get(&reference_key(identity))?;
        let kind = if explicit.is_some() {
            LinkType::Reference
        } else if text[..matched.end()].ends_with("[]") {
            LinkType::Collapsed
        } else {
            LinkType::Shortcut
        };
        Some(Link {
            consumed: matched.end(),
            label: matched.group(1)?,
            destination: decode_operand(&definition.destination),
            title: decode_operand(&definition.title),
            kind,
            id,
        })
    }

    /// Angle autolinks retain literal operands, including entity spellings.
    pub(super) fn angle(&self, range: Range<usize>) -> Option<(usize, Kind)> {
        let text = &self.source.text[range.clone()];
        let matched = rules::INLINE_AUTOLINK.as_ref()?.find(text)?;
        let label = capture(text, &matched, 1);
        let email = matched.group(2).is_some();
        let destination = if email {
            format!("mailto:{label}")
        } else {
            label.to_owned()
        };
        let child = crate::Node {
            range: self
                .source
                .extent(range.start + 1..range.start + 1 + label.len()),
            authored: label.to_owned(),
            kind: Kind::Text(Text {
                authored: label.to_owned(),
                decoded: label.to_owned(),
            }),
        };
        Some((
            matched.end(),
            Kind::Link(
                vec![child],
                label.to_owned(),
                destination,
                LinkMetadata {
                    title: String::new(),
                    link_type: if email {
                        LinkType::Email
                    } else {
                        LinkType::Autolink
                    },
                    id: String::new(),
                },
            ),
        ))
    }
}

/// Ordinary link labels cannot publish nested anchors; images and code stay opaque.
fn contains_link(nodes: &[crate::Node]) -> bool {
    nodes.iter().any(|node| match &node.kind {
        Kind::Link(_, _, _, metadata) => {
            !matches!(metadata.link_type, LinkType::Autolink | LinkType::Email)
        }
        Kind::Strong(children) | Kind::Emphasis(children) | Kind::Strike(children) => {
            contains_link(children)
        }
        _ => false,
    })
}

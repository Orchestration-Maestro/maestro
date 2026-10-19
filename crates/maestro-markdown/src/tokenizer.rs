//! Source-backed tokens for block dispatch and inline interpretation.
pub(crate) mod block;
mod html;
pub(crate) mod inline;

use crate::{Kind, Node, source::ScopeSource};
use std::ops::Range;

/// Build a semantic node without losing its original source extent.
pub(crate) fn node(source: &ScopeSource, range: Range<usize>, kind: Kind) -> Node {
    source.view(0..source.text.len()).node(range, kind)
}

/// Captured block syntax with an optional independently interpreted inline body.
pub(crate) struct Block {
    /// Number of authored bytes consumed by this block.
    pub(crate) consumed: usize,
    /// Owned semantic node.
    pub(crate) node: Node,
    /// Inline source retained until all document definitions are known.
    pub(crate) inline: Option<ScopeSource>,
}

/// Access a capture as a borrowed string without assuming optional groups exist.
pub(crate) fn capture<'a>(text: &'a str, matched: &regress::Match, index: usize) -> &'a str {
    matched.group(index).map_or("", |range| &text[range])
}

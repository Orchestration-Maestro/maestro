//! Lexical tokens and owned concrete syntax trees.

use super::composer::Composer;
use std::ops::Range;

/// Lexical categories independent of decoded scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TokenKind {
    /// An unrecognized lexical unit.
    Error,
    /// Leading Unicode marker.
    ByteOrderMark,
    /// Beginning of an implicit document.
    DocMode,
    /// Explicit document start.
    DocStart,
    /// Unexpected end of a flow collection.
    FlowErrorEnd,
    /// Explicit document end.
    DocEnd,
    /// Line break.
    Newline,
    /// Horizontal whitespace.
    Space,
    /// Comment through the end of its line.
    Comment,
    /// Directive line.
    Directive,
    /// Block sequence indicator.
    SeqItemInd,
    /// Explicit mapping key indicator.
    ExplicitKeyInd,
    /// Mapping value indicator.
    MapValueInd,
    /// Flow mapping opening delimiter.
    FlowMapStart,
    /// Flow mapping closing delimiter.
    FlowMapEnd,
    /// Flow sequence opening delimiter.
    FlowSeqStart,
    /// Flow sequence closing delimiter.
    FlowSeqEnd,
    /// Flow member separator.
    Comma,
    /// Anchor reference.
    Alias,
    /// Anchor declaration.
    Anchor,
    /// Explicit node tag.
    Tag,
    /// Unquoted scalar text.
    Scalar,
    /// Single-quoted scalar text.
    SingleQuotedScalar,
    /// Double-quoted scalar text.
    DoubleQuotedScalar,
    /// Quoted text whose closing delimiter was not consumed.
    UnclosedQuotedScalar,
    /// Block scalar header.
    BlockScalarHeader,
}

impl TokenKind {
    /// Whether a token carries no node or indicator.
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Space | Self::Newline | Self::Comment)
    }

    /// Whether this token starts a flow scalar node.
    pub fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::Alias | Self::Scalar | Self::SingleQuotedScalar | Self::DoubleQuotedScalar
        )
    }
}

/// One complete lexeme with byte coordinates.
#[derive(Debug, Clone)]
pub(super) struct SourceToken {
    /// Lexical category.
    pub kind: TokenKind,
    /// Byte offset at the beginning.
    pub offset: usize,
    /// Byte offset immediately after the token.
    pub end: usize,
    /// Indentation tracked by the parser.
    pub indent: usize,
}

impl SourceToken {
    /// Construct a lexical token before parser indentation is assigned.
    pub fn new(kind: TokenKind, range: Range<usize>) -> Self {
        Self {
            kind,
            offset: range.start,
            end: range.end,
            indent: 0,
        }
    }

    /// Source range wholly owned by this token.
    pub fn range(&self) -> Range<usize> {
        self.offset..self.end
    }
}

/// Tokens and nodes of one collection entry.
#[derive(Debug, Default)]
pub(super) struct CollectionItem {
    /// Properties and trivia before the key or sequence value.
    pub start: Vec<SourceToken>,
    /// Mapping key, absent for an empty key.
    pub key: Option<Box<Node>>,
    /// Separator and value properties, present even when empty.
    pub sep: Option<Vec<SourceToken>>,
    /// Mapping or sequence value.
    pub value: Option<Box<Node>>,
    /// Whether an explicit key indicator was seen.
    pub explicit_key: bool,
}

/// Syntax node kind.
#[derive(Debug)]
pub(super) enum NodeKind {
    /// Alias or flow scalar token.
    Scalar(SourceToken),
    /// Block scalar properties and content token.
    BlockScalar {
        /// Header and its trailing lexical tokens.
        props: Vec<SourceToken>,
        /// Complete scalar content, including retained newlines.
        source: Option<SourceToken>,
    },
    /// Block mapping entries.
    BlockMap(Vec<CollectionItem>),
    /// Block sequence entries.
    BlockSequence(Vec<CollectionItem>),
    /// Flow entries and opening delimiter.
    FlowCollection {
        /// Opening flow delimiter.
        start: SourceToken,
        /// Authored collection entries.
        items: Vec<CollectionItem>,
    },
    /// Document properties and optional value.
    Document {
        /// Leading document properties and trivia.
        start: Vec<SourceToken>,
        /// Optional document content.
        value: Option<Box<Node>>,
    },
    /// Explicit document end.
    DocEnd,
    /// Invalid syntax retained for composition.
    Error(&'static str),
}

/// Concrete syntax node with trailing lexical tokens.
#[derive(Debug)]
pub(super) struct Node {
    /// Node-specific content.
    pub kind: NodeKind,
    /// Beginning of the node.
    pub offset: usize,
    /// Block indentation.
    pub indent: usize,
    /// Trailing trivia or flow closing delimiter.
    pub end: Vec<SourceToken>,
}

impl Node {
    /// Construct a node without trailing tokens.
    pub fn new(kind: NodeKind, token: &SourceToken) -> Self {
        Self {
            kind,
            offset: token.offset,
            indent: token.indent,
            end: Vec::new(),
        }
    }

    /// Whether this is a flow-style node.
    pub fn is_flow(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::Scalar(_) | NodeKind::FlowCollection { .. }
        )
    }

    /// Whether this node is an open collection.
    pub fn is_collection(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::BlockMap(_) | NodeKind::BlockSequence(_) | NodeKind::FlowCollection { .. }
        )
    }
}

/// Whether properties contain the selected token kind.
pub(super) fn includes_token(tokens: &[SourceToken], kind: TokenKind) -> bool {
    tokens.iter().any(|token| token.kind == kind)
}

/// Remove the final key properties after the last structural separator.
pub(super) fn get_first_key_start_props(tokens: &mut Vec<SourceToken>) -> Vec<SourceToken> {
    let start = tokens
        .iter()
        .rposition(|token| {
            matches!(
                token.kind,
                TokenKind::DocStart
                    | TokenKind::ExplicitKeyInd
                    | TokenKind::MapValueInd
                    | TokenKind::SeqItemInd
                    | TokenKind::Newline
            )
        })
        .map_or(0, |i| i + 1);
    let spaces = tokens[start..]
        .iter()
        .take_while(|token| token.kind == TokenKind::Space)
        .count();
    tokens.split_off(start + spaces)
}

impl Composer<'_> {
    /// Observe newlines only in a node's retained token ranges.
    pub fn contains_newline(&self, node: &Node) -> bool {
        match &node.kind {
            NodeKind::Scalar(token) => {
                self.text(token).contains('\n') || includes_token(&node.end, TokenKind::Newline)
            }
            NodeKind::FlowCollection { items, .. } => items.iter().any(|item| {
                includes_token(&item.start, TokenKind::Newline)
                    || item
                        .sep
                        .as_ref()
                        .is_some_and(|sep| includes_token(sep, TokenKind::Newline))
                    || item
                        .key
                        .as_deref()
                        .is_some_and(|node| self.contains_newline(node))
                    || item
                        .value
                        .as_deref()
                        .is_some_and(|node| self.contains_newline(node))
            }),
            _ => true,
        }
    }
}

//! Owned native source records.
use std::ops::Range;

/// Alignment of a table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    /// No authored alignment.
    None,
    /// Left alignment.
    Left,
    /// Center alignment.
    Center,
    /// Right alignment.
    Right,
}

/// Source classification of a link or image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkType {
    /// Inline destination.
    Inline,
    /// Explicit reference definition.
    Reference,
    /// Unresolved explicit reference supplied by a caller.
    ReferenceUnknown,
    /// Collapsed reference definition.
    Collapsed,
    /// Unresolved collapsed reference supplied by a caller.
    CollapsedUnknown,
    /// Shortcut reference definition.
    Shortcut,
    /// Unresolved shortcut reference supplied by a caller.
    ShortcutUnknown,
    /// Angle-enclosed URI or mapped bare destination.
    Autolink,
    /// Angle-enclosed email address.
    Email,
    /// Caller-supplied wiki classification; parsing does not enable wiki syntax.
    WikiLink {
        /// Whether an explicit label separates the destination from its display text.
        has_pothole: bool,
    },
}

/// A native node together with its source extent.
pub struct Node {
    /// Semantic content.
    pub kind: Kind,
    /// Authored content with enclosing native continuation prefixes removed.
    pub authored: String,
    /// Extent in the normalized source.
    pub range: Range<usize>,
}
/// Block and inline structures admitted by the configured parser.
pub enum Kind {
    /// Opaque atom in a locally interpreted inline run, indexed by that run.
    Atom(usize),
    /// Ordinary paragraph children.
    Paragraph(Vec<Node>),
    /// Tight-item inline container with its full authored source.
    Inline(Vec<Node>),
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
    Text(Text),
    /// Code text and optional full fence information.
    CodeBlock(String, Option<String>),
    /// Styled children, authored label and renderer-facing destination.
    Link(Vec<Node>, String, String, LinkMetadata),
    /// Authored image label, retained without inline formatting.
    Image(String, ImageMetadata),
    /// Native checked or unchecked task control.
    TaskListMarker(bool),
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
    /// Header and body cells with the authored rows.
    Table(Table),
}
/// Inline children of each cell in one table row.
pub type Cells = Vec<Cell>;
/// One table cell with its original inline source and native extent.
pub struct Cell {
    /// Complete inline operands.
    pub children: Vec<Node>,
    /// Container-local authored bytes, not reconstructed decoded text.
    pub authored: String,
    /// Native source coordinates.
    pub range: Range<usize>,
}
/// A native table; the parser supplies one body cell per header column.
pub struct Table {
    /// Header cells in column order.
    pub header: Cells,
    /// Per-column native alignment.
    pub alignments: Vec<Alignment>,
    /// Body rows in authored order.
    pub rows: Vec<Cells>,
    /// Authored rows without container prefixes or the final line terminator.
    pub authored: String,
}
/// One native text unit with separate display and authored spellings.
pub struct Text {
    /// Native-decoded display content.
    pub decoded: String,
    /// Authored content with enclosing continuation prefixes removed.
    pub authored: String,
}
/// Owned link metadata; no input lifetime reaches a retained renderer.
pub struct LinkMetadata {
    /// Optional tooltip; empty means absent.
    pub title: String,
    /// Native source classification.
    pub link_type: LinkType,
    /// Native reference identity.
    pub id: String,
}
/// Complete image operands retained beside the authored terminal label.
pub struct ImageMetadata {
    /// Decoded destination.
    pub destination: String,
    /// Decoded tooltip.
    pub title: String,
    /// Native link classification.
    pub link_type: LinkType,
    /// Native reference identity.
    pub id: String,
    /// Alt-label inline structure.
    pub children: Vec<Node>,
}
/// Owned shared document; source coordinates name this exact string.
pub struct Document {
    /// Captured inline source provenance, owned by this document.
    pub(crate) scopes:
        std::collections::HashMap<crate::inline::ScopeKey, crate::source::ScopeSource>,
    /// Input supplied by the caller's normalization policy.
    pub source: String,
    /// Complete native semantic content.
    pub nodes: Vec<Node>,
}

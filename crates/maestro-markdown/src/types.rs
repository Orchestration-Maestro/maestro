//! Owned native source records.
use std::ops::Range;

/// Immutable grammar selection for an existing Markdown consumer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkedOptions {
    /// Terminal grammar and strict double-tilde deletion.
    #[default]
    Terminal,
    /// Browser grammar with caller-selected authored HTML handling.
    Browser {
        /// Show authored tags as text instead of live markup.
        escape_html: bool,
    },
    /// Standalone document grammar and strict double-tilde deletion.
    Export,
    /// Code-block source ranges for documentation checks.
    Documentation,
}

/// A caller-admitted block extension token.
pub struct BlockToken {
    /// Authored input prefix; admission requires it to be nonempty.
    pub raw: String,
    /// Caller-owned token classification.
    pub kind: String,
    /// Parsed content retained for the caller's renderer.
    pub text: String,
}

/// A resolved, first-wins document reference definition.
#[derive(Clone)]
pub(crate) struct Definition {
    /// Authored destination, decoded only when resolving an operand.
    pub(crate) destination: String,
    /// Authored optional title, decoded only when resolving an operand.
    pub(crate) title: String,
}

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

/// Handling class of a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkType {
    /// Authored link label, including resolved document references.
    Inline,
    /// Automatic URI or email destination, angle-enclosed or bare.
    Autolink,
}

/// A native node together with its source extent.
pub struct Node {
    /// Semantic content.
    pub kind: Kind,
    /// Authored content with enclosing native continuation prefixes removed.
    pub authored: crate::MappedText,
    /// Extent in the normalized source.
    pub range: Range<usize>,
}
/// Block and inline structures admitted by the configured parser.
pub enum Kind {
    /// Caller-owned block classification and parsed content.
    CustomBlock(String, String),
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
    Quote(Vec<Node>, crate::MappedText),
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
/// One table cell with its inline children and original source extent.
pub struct Cell {
    /// Complete inline operands.
    pub children: Vec<Node>,
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
    /// Link handling class.
    pub link_type: LinkType,
}
/// Complete image operands retained beside the authored terminal label.
pub struct ImageMetadata {
    /// Decoded destination.
    pub destination: String,
    /// Decoded tooltip.
    pub title: String,
    /// Alt-label inline structure.
    pub children: Vec<Node>,
}
/// Owned shared document; source coordinates name this exact string.
pub struct Document {
    /// Grammar retained for local inline interpretation.
    pub(crate) options: MarkedOptions,
    /// Definitions shared by all inline scopes, including forward references.
    pub(crate) definitions: std::collections::HashMap<String, Definition>,
    /// Captured inline source provenance, owned by this document.
    pub(crate) scopes:
        std::collections::HashMap<crate::inline::ScopeKey, crate::source::ScopeSource>,
    /// Input supplied by the caller's normalization policy.
    pub source: String,
    /// Complete native semantic content.
    pub nodes: Vec<Node>,
}

//! Child nodes of one parsed node, for tests that walk whole documents.
use crate::{Kind, Node};

/// Children of one node in authored order, including table cells and image labels.
#[must_use]
pub fn children(kind: &Kind) -> Box<dyn Iterator<Item = &Node> + '_> {
    match kind {
        Kind::Paragraph(nodes)
        | Kind::Inline(nodes)
        | Kind::Heading(_, nodes, _)
        | Kind::Quote(nodes, _)
        | Kind::Item(nodes)
        | Kind::List(_, nodes)
        | Kind::Strong(nodes)
        | Kind::Strike(nodes)
        | Kind::Emphasis(nodes)
        | Kind::Link(nodes, _, _, _) => Box::new(nodes.iter()),
        Kind::Image(_, metadata) => Box::new(metadata.children.iter()),
        Kind::Table(table) => Box::new(
            table
                .header
                .iter()
                .chain(table.rows.iter().flatten())
                .flat_map(|cell| cell.children.iter()),
        ),
        _ => Box::new(std::iter::empty()),
    }
}

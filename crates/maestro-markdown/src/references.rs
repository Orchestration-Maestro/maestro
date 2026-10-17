//! Resolved document references shared with fragment parsing.
use crate::{Kind, Node};
use std::collections::HashMap;

/// Collect resolved operands under the reference identity of each authored use.
pub(crate) fn collect(nodes: &[Node]) -> HashMap<String, (String, String)> {
    let mut references = HashMap::new();
    visit(nodes, &mut references);
    references
}
/// Include references in nested containers, image labels and table cells.
fn visit(nodes: &[Node], references: &mut HashMap<String, (String, String)>) {
    for node in nodes {
        match &node.kind {
            Kind::Link(children, _, href, meta) => {
                references.insert(meta.id.clone(), (href.clone(), meta.title.clone()));
                visit(children, references);
            }
            Kind::Image(_, meta) => {
                references.insert(
                    meta.id.clone(),
                    (meta.destination.clone(), meta.title.clone()),
                );
                visit(&meta.children, references);
            }
            Kind::Paragraph(c)
            | Kind::Inline(c)
            | Kind::Heading(_, c, _)
            | Kind::List(_, c)
            | Kind::Item(c)
            | Kind::Quote(c, _)
            | Kind::Strong(c)
            | Kind::Strike(c)
            | Kind::Emphasis(c) => visit(c, references),
            Kind::Table(table) => {
                for cell in std::iter::once(&table.header).chain(&table.rows).flatten() {
                    visit(&cell.children, references);
                }
            }
            _ => {}
        }
    }
}

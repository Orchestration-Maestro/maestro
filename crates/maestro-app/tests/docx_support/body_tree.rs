//! Lossless observations of owned Word trees.
#![cfg(test)]
use maestro_app::browser::docx_preview::{
    DomType, OpenXmlElementData as Data, OpenXmlElementTree, OwnedXmlElement,
};
use serde_json::{Value, json};

/// Observe the retained XML selection, including mixed element/text order.
pub fn raw(input: &OwnedXmlElement) -> Value {
    let parser = roxmltree::Document::parse(input.source()).unwrap();
    let element = parser
        .descendants()
        .find(|n| n.is_element() && n.range() == input.source_range())
        .unwrap();
    xml(element)
}
/// Project one XML element without discarding text or namespace identity.
fn xml(element: roxmltree::Node<'_, '_>) -> Value {
    json!({
        "local_name": element.tag_name().name(), "namespace": element.tag_name().namespace(),
        "attributes": element.attributes().map(|a| json!({"local_name": a.name(), "namespace": a.namespace(), "value": a.value()})).collect::<Vec<_>>(),
        "children": element.children().filter_map(|n| if n.is_element() { Some(xml(n)) } else { n.text().map(|t| json!(t)) }).collect::<Vec<_>>()
    })
}
/// Scalar payload observation; common links are observed separately.
fn payload(data: &Data) -> Value {
    match data {
        Data::Run { field_run: true } => json!({"fieldRun":true}),
        Data::Text { text } | Data::DeletedText { text } | Data::Instruction { text } => {
            json!({"text":text})
        }
        Data::Break { r#break } => json!({"break":r#break}),
        Data::Symbol { font, char } => json!({"font":font,"char":char}),
        Data::FootnoteReference { id } | Data::EndnoteReference { id } | Data::AltChunk { id } => {
            json!({"id":id})
        }
        Data::SimpleField {
            instruction,
            lock,
            dirty,
        } => json!({"instruction":instruction,"lock":lock,"dirty":dirty}),
        Data::ComplexField {
            char_type,
            lock,
            dirty,
        } => json!({"charType":char_type,"lock":lock,"dirty":dirty}),
        _ => json!({}),
    }
}
/// Assign graph positions by children first, then parent-only identities.
fn visit(
    tree: &OpenXmlElementTree,
    id: usize,
    positions: &mut [Option<usize>],
    nodes: &mut Vec<Value>,
) -> usize {
    if let Some(index) = positions[id] {
        return index;
    }
    let index = nodes.len();
    positions[id] = Some(index);
    nodes.push(Value::Null);
    let node = &tree.nodes[id];
    let children: Vec<_> = node
        .base
        .children
        .iter()
        .map(|&child| visit(tree, child, positions, nodes))
        .collect();
    let parent = node
        .base
        .parent
        .map(|parent| visit(tree, parent, positions, nodes));
    let mut value = payload(&node.data);
    value["type"] = json!(node.dom_type().map(DomType::as_str));
    if matches!(
        node.data,
        Data::Paragraph
            | Data::Run { .. }
            | Data::SimpleField { .. }
            | Data::Inserted
            | Data::Deleted
            | Data::AltChunk { .. }
    ) {
        value["children"] = json!(children);
    } else {
        assert!(children.is_empty());
    }
    if matches!(node.data, Data::Run { .. }) {
        value["parent"] = json!(parent);
    } else {
        assert!(parent.is_none());
    }
    nodes[index] = value;
    index
}
/// Observe the complete graph, including repeated links and hidden parents.
pub fn graph(tree: &OpenXmlElementTree) -> Value {
    let mut positions = vec![None; tree.nodes.len()];
    let mut nodes = Vec::new();
    let roots: Vec<_> = tree
        .roots
        .iter()
        .map(|&root| visit(tree, root, &mut positions, &mut nodes))
        .collect();
    json!({"roots":roots,"nodes":nodes})
}
/// Observe property slots in encounter order within each owning element.
pub fn properties(tree: &OpenXmlElementTree) -> Value {
    json!(
        tree.nodes
            .iter()
            .flat_map(|node| node
                .base
                .properties
                .iter()
                .map(|slot| json!({"context":node.dom_type().unwrap().as_str(),"xml":raw(slot)})))
            .collect::<Vec<_>>()
    )
}
/// Observe every foreign slot with its actual kind and complete retained XML.
pub fn foreign(tree: &OpenXmlElementTree, context: &str) -> Value {
    json!(tree.nodes.iter().filter_map(|node| if let Data::Unparsed { element, .. } = &node.data {
        let xml = raw(element);
        Some(json!({"context":context,"name":xml["local_name"],"kind":node.dom_type().map(DomType::as_str),"xml":xml}))
    } else { None }).collect::<Vec<_>>())
}

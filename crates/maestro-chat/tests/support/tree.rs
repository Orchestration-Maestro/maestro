//! Shared borrowed session-tree fixture construction.
use maestro_app::SessionTreeNode;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
/// One borrowed node's recorded fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// Original entry passed intact to the existing owner type.
    pub entry: Value,
    /// Child occurrence indices.
    pub children: Vec<usize>,
    /// Present labels retain explicit null.
    #[serde(default, deserialize_with = "present_value")]
    pub label: Option<Value>,
    /// Present timestamps retain explicit null.
    #[serde(default, rename = "labelTimestamp", deserialize_with = "present_value")]
    pub label_timestamp: Option<Value>,
}
/// Distinguishes an explicitly null label from an omitted label.
pub fn present_value<'de, D: Deserializer<'de>>(decoder: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(decoder).map(Some)
}
/// Builds owned child containers iteratively over borrowed raw entries.
pub fn tree<'a>(nodes: &'a [Node], roots: &[usize]) -> Vec<SessionTreeNode<'a>> {
    let mut order = Vec::new();
    let mut pending = roots.to_vec();
    while let Some(index) = pending.pop() {
        order.push(index);
        pending.extend(&nodes[index].children);
    }
    assert_eq!(order.len(), nodes.len());
    let mut built: Vec<Option<SessionTreeNode<'_>>> = (0..nodes.len()).map(|_| None).collect();
    for index in order.into_iter().rev() {
        let node = &nodes[index];
        built[index] = Some(SessionTreeNode {
            entry: &node.entry,
            children: node
                .children
                .iter()
                .map(|&child| built[child].take().unwrap())
                .collect(),
            label: node.label.as_ref(),
            label_timestamp: node.label_timestamp.as_ref(),
        });
    }
    roots
        .iter()
        .map(|&index| built[index].take().unwrap())
        .collect()
}

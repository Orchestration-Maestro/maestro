//! Intersection evaluation for declared conversion.
//!
//! A declared intersection converts through the members its declaration narrows to. Members of
//! the same shape merge their declared members; disjoint members accept nothing; alternatives
//! distribute over the remaining members.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::SchemaKind;

use super::Context;

/// A schema node with the kinds its author declared, rooted at the empty pointer.
#[derive(Clone)]
pub(super) struct Node {
    /// Node schema.
    pub(super) schema: Value,
    /// Declared kinds below this node, keyed by a pointer from its root.
    pub(super) kinds: HashMap<String, SchemaKind>,
}

impl Node {
    /// The kind declared for this node itself.
    fn kind(&self) -> Option<&SchemaKind> {
        self.kinds.get("")
    }

    /// The declared alternatives of this node, when it is one alternative list.
    fn alternatives(&self) -> Option<Vec<Node>> {
        let members = self.schema.get("anyOf").and_then(Value::as_array)?;
        Some(
            members
                .iter()
                .enumerate()
                .map(|(index, member)| self.child(member, &format!("/anyOf/{index}")))
                .collect(),
        )
    }

    /// Extract the node a nested member schema stands for.
    fn child(&self, schema: &Value, pointer: &str) -> Self {
        let mut kinds = HashMap::new();
        for (key, kind) in &self.kinds {
            let nested = if key.is_empty() {
                // The enclosing node's own kind belongs to the enclosing node, not to the child.
                None
            } else if key == pointer {
                Some(String::new())
            } else {
                key.strip_prefix(pointer).map(ToOwned::to_owned)
            };
            if let Some(nested) = nested {
                kinds.insert(nested, kind.clone());
            }
        }
        Self {
            schema: schema.clone(),
            kinds,
        }
    }

    /// Re-root an extracted node's kinds below one pointer segment.
    fn re_root(&self, pointer: &str, schema: &Value) -> Self {
        let kinds = self
            .kinds
            .iter()
            .map(|(key, kind)| {
                let nested = if key.is_empty() {
                    pointer.to_owned()
                } else {
                    format!("{pointer}/{key}")
                };
                (nested, kind.clone())
            })
            .collect();
        Self {
            schema: schema.clone(),
            kinds,
        }
    }
}

/// Evaluate one declared intersection into the node conversion uses.
pub(super) fn evaluate(context: &Context<'_>, pointer: &str, members: &[Value]) -> Option<Node> {
    let root = Node {
        schema: context.schema().clone(),
        kinds: context.kinds().clone(),
    };
    if members.is_empty() {
        return None;
    }
    let nodes: Vec<Node> = members
        .iter()
        .enumerate()
        .map(|(index, member)| root.child(member, &format!("{pointer}/allOf/{index}")))
        .collect();
    broaden(&distribute(&nodes))
}

/// Combine the supplied nodes pairwise, distributing alternatives.
fn distribute(nodes: &[Node]) -> Vec<Node> {
    let mut accumulated = vec![nodes[0].clone()];
    for node in &nodes[1..] {
        accumulated = accumulated
            .into_iter()
            .flat_map(|left| pair(left, node.clone()))
            .collect();
    }
    accumulated
}

/// Combine two evaluated members into the nodes their intersection accepts.
fn pair(left: Node, right: Node) -> Vec<Node> {
    if let Some(alternatives) = left.alternatives() {
        return alternatives
            .into_iter()
            .flat_map(|alternative| pair(alternative, right.clone()))
            .collect();
    }
    if let Some(alternatives) = right.alternatives() {
        return alternatives
            .into_iter()
            .flat_map(|alternative| pair(left.clone(), alternative))
            .collect();
    }
    match (object_like(&left), object_like(&right)) {
        (true, true) => vec![composite(&left, &right)],
        (true, false) => vec![left],
        (false, true) => vec![right],
        (false, false) => vec![narrow(left, right)],
    }
}

/// Whether the node declares named members.
fn object_like(node: &Node) -> bool {
    matches!(node.kind(), Some(SchemaKind::Object))
}

/// Merge two nodes that both declare named members.
fn composite(left: &Node, right: &Node) -> Node {
    let left_properties = properties(left);
    let right_properties = properties(right);
    let mut merged = Map::new();
    let mut kinds = HashMap::new();
    let mut keys: Vec<&String> = left_properties
        .keys()
        .chain(right_properties.keys())
        .collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let member = match (left_properties.get(key), right_properties.get(key)) {
            (Some(one), Some(other)) => pair(one.clone(), other.clone())
                .into_iter()
                .next()
                .unwrap_or_else(never),
            (Some(one), None) => one.clone(),
            (None, Some(other)) => other.clone(),
            (None, None) => continue,
        };
        let pointer = format!("/properties/{}", key.replace('~', "~0").replace('/', "~1"));
        let member = member.re_root(&pointer, &member.schema.clone());
        kinds.extend(member.kinds.iter().map(|(k, v)| (k.clone(), v.clone())));
        merged.insert(key.clone(), member.schema);
    }
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(merged));
    let mut node_kinds = HashMap::new();
    node_kinds.insert(String::new(), SchemaKind::Object);
    node_kinds.extend(kinds);
    Node {
        schema: Value::Object(schema),
        kinds: node_kinds,
    }
}

/// Read the declared members of an object node.
fn properties(node: &Node) -> HashMap<String, Node> {
    node.schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .map(|(key, schema)| {
                    (
                        key.clone(),
                        node.child(schema, &format!("/properties/{key}")),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Keep the narrower of two members, or nothing when they are disjoint.
fn narrow(left: Node, right: Node) -> Node {
    match relation(&left, &right) {
        Relation::Equal | Relation::RightInside => right,
        Relation::LeftInside => left,
        Relation::Disjoint => never(),
    }
}

/// How two declared members relate.
enum Relation {
    /// The members accept the same values.
    Equal,
    /// The left member accepts only values of the right member.
    LeftInside,
    /// The right member accepts only values of the left member.
    RightInside,
    /// The members accept no common value.
    Disjoint,
}

/// Compare two declared members.
fn relation(left: &Node, right: &Node) -> Relation {
    match (left.kind(), right.kind()) {
        (Some(SchemaKind::Never) | None, _) | (_, Some(SchemaKind::Never) | None) => {
            Relation::Disjoint
        }
        (Some(SchemaKind::Any | SchemaKind::Unknown), _)
        | (Some(SchemaKind::Integer), Some(SchemaKind::Number)) => Relation::LeftInside,
        (Some(SchemaKind::Number), Some(SchemaKind::Integer))
        | (_, Some(SchemaKind::Any | SchemaKind::Unknown)) => Relation::RightInside,
        (Some(SchemaKind::Literal), Some(SchemaKind::Literal)) => {
            if left.schema.get("const") == right.schema.get("const") {
                Relation::Equal
            } else {
                Relation::Disjoint
            }
        }
        (Some(SchemaKind::Literal), Some(kind)) => {
            if literal_fits(left, kind) {
                Relation::LeftInside
            } else {
                Relation::Disjoint
            }
        }
        (Some(kind), Some(SchemaKind::Literal)) => {
            if literal_fits(right, kind) {
                Relation::RightInside
            } else {
                Relation::Disjoint
            }
        }
        (Some(SchemaKind::Array), Some(SchemaKind::Array)) => {
            let (Some(one), Some(other)) = (items(left), items(right)) else {
                return Relation::Equal;
            };
            match relation(&one, &other) {
                Relation::Equal => Relation::Equal,
                Relation::LeftInside => Relation::LeftInside,
                Relation::RightInside => Relation::RightInside,
                Relation::Disjoint => Relation::Disjoint,
            }
        }
        (Some(one), Some(other)) if one == other => Relation::Equal,
        _ => Relation::Disjoint,
    }
}

/// Whether a declared literal accepts a value of the supplied kind.
fn literal_fits(literal: &Node, kind: &SchemaKind) -> bool {
    let constant = literal.schema.get("const");
    match (constant, kind) {
        (Some(Value::String(_)), SchemaKind::String)
        | (Some(Value::Bool(_)), SchemaKind::Boolean)
        | (Some(Value::Null), SchemaKind::Null)
        | (Some(Value::Number(_)), SchemaKind::Number) => true,
        (Some(Value::Number(number)), SchemaKind::Integer) => {
            number.as_f64().is_some_and(|n| n.fract() == 0.0)
        }
        _ => false,
    }
}

/// Extract a sequence node's item node.
fn items(node: &Node) -> Option<Node> {
    let schema = node.schema.get("items")?;
    Some(node.child(schema, "/items"))
}

/// Reduce the distributed members to the one node conversion uses.
fn broaden(nodes: &[Node]) -> Option<Node> {
    let mut accepted: Vec<Node> = nodes
        .iter()
        .filter(|node| node.kind() != Some(&SchemaKind::Never))
        .cloned()
        .collect();
    match accepted.len() {
        0 => None,
        1 => accepted.pop(),
        _ => Some(union(&accepted)),
    }
}

/// Build the alternative node a set of accepted members stands for.
fn union(nodes: &[Node]) -> Node {
    let members: Vec<Value> = nodes.iter().map(|node| node.schema.clone()).collect();
    let mut kinds = HashMap::new();
    kinds.insert(String::new(), SchemaKind::Union);
    for (index, node) in nodes.iter().enumerate() {
        let member = node.re_root(&format!("/anyOf/{index}"), &node.schema.clone());
        kinds.extend(member.kinds);
    }
    Node {
        schema: serde_json::json!({ "anyOf": members }),
        kinds,
    }
}

/// The node that accepts no value.
fn never() -> Node {
    Node {
        schema: serde_json::json!({ "not": {} }),
        kinds: HashMap::from([(String::new(), SchemaKind::Never)]),
    }
}

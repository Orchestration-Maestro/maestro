//! Intersection evaluation for declared conversion.
//!
//! A declared intersection converts through the members its declaration narrows to. Members of
//! the same shape merge their declared members; disjoint members accept nothing; alternatives
//! distribute over the remaining members.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::SchemaKind;

use super::{Context, collections};

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
        Self {
            schema: schema.clone(),
            kinds: nested_kinds(&self.kinds, pointer),
        }
    }

    /// Re-root this node's kinds below one pointer segment.
    fn re_rooted_kinds(&self, pointer: &str) -> HashMap<String, SchemaKind> {
        self.kinds
            .iter()
            .map(|(key, kind)| (nested(pointer, key), kind.clone()))
            .collect()
    }
}

/// Evaluate one declared intersection into the node conversion uses.
pub(super) fn evaluate(context: &Context<'_>, pointer: &str, members: &[Value]) -> Option<Node> {
    if members.is_empty() {
        return None;
    }
    let nodes: Vec<Node> = members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let address = format!("{pointer}/allOf/{index}");
            Node {
                schema: member.clone(),
                kinds: nested_kinds(context.kinds(), &address),
            }
        })
        .collect();
    broaden(&distribute(&nodes))
}

/// The kinds declared below one pointer, keyed from that pointer's node.
fn nested_kinds(kinds: &HashMap<String, SchemaKind>, pointer: &str) -> HashMap<String, SchemaKind> {
    kinds
        .iter()
        .filter_map(|(key, kind)| {
            let nested = if key.is_empty() {
                None
            } else if key == pointer {
                Some(String::new())
            } else {
                key.strip_prefix(pointer).map(ToOwned::to_owned)
            };
            nested.map(|nested| (nested, kind.clone()))
        })
        .collect()
}

/// Address a node's kinds below one pointer, keeping the pointer's own boundaries.
fn nested(pointer: &str, key: &str) -> String {
    match key.strip_prefix('/') {
        None => pointer.to_owned(),
        Some(rest) => format!("{pointer}/{rest}"),
    }
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
            (Some(one), Some(other)) => {
                broaden(&pair(one.clone(), other.clone())).unwrap_or_else(never)
            }
            (Some(one), None) => one.clone(),
            (None, Some(other)) => other.clone(),
            (None, None) => continue,
        };
        let pointer = format!("/properties/{}", collections::escape(key));
        kinds.extend(member.re_rooted_kinds(&pointer));
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
                        node.child(schema, &format!("/properties/{}", collections::escape(key))),
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
        (
            Some(SchemaKind::Any | SchemaKind::Unknown),
            Some(SchemaKind::Any | SchemaKind::Unknown),
        ) => Relation::Equal,
        (Some(SchemaKind::Any | SchemaKind::Unknown), _)
        | (Some(SchemaKind::Number), Some(SchemaKind::Integer)) => Relation::RightInside,
        (Some(SchemaKind::Integer), Some(SchemaKind::Number))
        | (_, Some(SchemaKind::Any | SchemaKind::Unknown)) => Relation::LeftInside,
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
    let mut kinds = HashMap::new();
    kinds.insert(String::new(), SchemaKind::Union);
    let mut members = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        kinds.extend(node.re_rooted_kinds(&format!("/anyOf/{index}")));
        members.push(node.schema.clone());
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

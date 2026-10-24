//! Explicit composition work and ordered collection assembly.

use super::{FrontmatterError, FrontmatterValue, Node, Properties};
use crate::metadata::values::Mapping;

/// A child node paired with its validated properties.
pub(in crate::metadata) struct Child<'a> {
    /// Optional syntax value; absence denotes an empty scalar.
    pub node: Option<&'a Node>,
    /// Properties belonging to that child.
    pub props: Properties,
}

/// Collection assembly after its children complete.
pub(in crate::metadata) enum CollectionKind {
    /// Alternating keys and values.
    Mapping,
    /// One child per sequence member.
    Sequence,
    /// Ordinary values and two-child mapping pairs.
    FlowSequence(Vec<bool>),
}

/// Validated children in source order, ready for composition.
pub(in crate::metadata) struct CollectionPlan<'a> {
    /// How completed children form the collection.
    pub kind: CollectionKind,
    /// Each child exactly once in source order.
    pub children: Vec<Child<'a>>,
}

/// Heap-owned continuation for nested composition.
pub(super) enum Work<'a> {
    /// Resolve a node with its parent-owned properties.
    Node(Option<&'a Node>, Properties),
    /// Assemble a collection after its children finish.
    Finish {
        /// Required assembly shape.
        kind: CollectionKind,
        /// Anchor published only after assembly.
        anchor: Option<String>,
        /// Start of this subtree's alias terms.
        mark: usize,
        /// Beginning of this collection's completed child values.
        base: usize,
    },
}

impl CollectionKind {
    /// Build the ordered collection from completed values without recursive traversal.
    pub(super) fn finish(
        self,
        children: Vec<FrontmatterValue>,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        if matches!(self, Self::Sequence) {
            return Ok(FrontmatterValue::Sequence(children));
        }
        let mut children = children.into_iter();
        if let Self::FlowSequence(pairs) = self {
            return Self::flow_sequence(pairs, &mut children);
        }
        let mut mapping = Mapping::default();
        while let Some(key) = children.next() {
            mapping.insert(key, Self::child(&mut children)?)?;
        }
        Ok(mapping.finish())
    }

    /// Assemble ordinary sequence values and two-child mapping pairs.
    fn flow_sequence(
        pairs: Vec<bool>,
        children: &mut std::vec::IntoIter<FrontmatterValue>,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        let mut sequence = Vec::new();
        for pair in pairs {
            let value = Self::child(children)?;
            if pair {
                let mut entry = Mapping::default();
                entry.insert(value, Self::child(children)?)?;
                sequence.push(entry.finish());
            } else {
                sequence.push(value);
            }
        }
        Ok(FrontmatterValue::Sequence(sequence))
    }

    /// Consume the next completed child, rejecting an incomplete construction.
    fn child(
        children: &mut std::vec::IntoIter<FrontmatterValue>,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        children
            .next()
            .ok_or_else(|| FrontmatterError::new("Missing collection child"))
    }
}

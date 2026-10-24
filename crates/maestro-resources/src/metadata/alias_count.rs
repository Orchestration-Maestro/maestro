//! Completed anchors and the alias-expansion limit.

use super::{FrontmatterError, FrontmatterValue};
use std::{collections::HashMap, ops::Range};

/// Largest accepted product of an anchor's uses and its alias weight.
const MAX_ALIAS_COUNT: usize = 100;

/// One scalar or alias in document order.
#[derive(Clone, Copy)]
enum Term {
    /// A scalar, weighing one.
    Leaf,
    /// A use of the anchor with this index.
    Alias(usize),
}

/// A published anchor with its expansion bookkeeping.
struct Anchor {
    /// The completed value aliases receive.
    value: FrontmatterValue,
    /// Uses so far, counting the anchored node itself.
    count: usize,
    /// Weight of the anchored subtree, computed at the first use.
    weight: usize,
    /// The anchored subtree's terms.
    terms: Range<usize>,
}

/// Completed anchors, latest binding per name, and the terms they cover.
#[derive(Default)]
pub(super) struct Anchors {
    /// Latest published anchor index per name.
    names: HashMap<String, usize>,
    /// Every published anchor.
    published: Vec<Anchor>,
    /// Scalars and aliases in document order.
    log: Vec<Term>,
}

impl Anchors {
    /// Position in the term log, where a node's terms begin.
    pub fn mark(&self) -> usize {
        self.log.len()
    }

    /// Record a scalar.
    pub fn leaf(&mut self) {
        self.log.push(Term::Leaf);
    }

    /// Publish a completed node whose terms start at `start`.
    pub fn publish(&mut self, name: String, value: FrontmatterValue, start: usize) {
        self.names.insert(name, self.published.len());
        self.published.push(Anchor {
            value,
            count: 1,
            weight: 0,
            terms: start..self.log.len(),
        });
    }

    /// Resolve the latest completed binding, rejecting excessive expansion.
    pub fn resolve(&mut self, name: &str) -> Result<FrontmatterValue, FrontmatterError> {
        let index = *self
            .names
            .get(name)
            .ok_or_else(|| FrontmatterError::new("Unresolved YAML alias"))?;
        self.log.push(Term::Alias(index));
        self.published[index].count += 1;
        if self.published[index].weight == 0 {
            self.published[index].weight = self.weight(index);
        }
        let anchor = &self.published[index];
        if anchor.count * anchor.weight > MAX_ALIAS_COUNT {
            return Err(FrontmatterError::new(
                "Excessive alias count indicates a resource exhaustion attack",
            ));
        }
        Ok(anchor.value.clone())
    }

    /// Heaviest scalar or nested alias use inside an anchored subtree.
    fn weight(&self, index: usize) -> usize {
        self.log[self.published[index].terms.clone()]
            .iter()
            .map(|term| match *term {
                Term::Leaf => 1,
                Term::Alias(nested) => self.published[nested].count * self.published[nested].weight,
            })
            .max()
            .unwrap_or(0)
    }
}

//! Publication of a list's items, including the nested lists they contain.
use super::BlockTokens;
use crate::{
    Kind, Node,
    tokenizer::block::{ListState, loosen},
};

/// One list whose item nodes are being filled.
///
/// Each item slot travels with the output that fills it, so an output can never
/// outlive the slot it belongs to.
struct Listing {
    /// Output owning the list node whose items are being filled.
    output: BlockTokens,
    /// The list node, held here until its filled items are restored to it.
    node: Node,
    /// Ordered start the list node carries.
    start: Option<u64>,
    /// Item slots paired with the output that fills each, in authored order.
    pending: std::vec::IntoIter<(Node, BlockTokens)>,
    /// Item nodes whose nested lists are published, in authored order.
    filled: Vec<Node>,
    /// Whether every item of this list is loose.
    loose: bool,
}

impl BlockTokens {
    /// Publish the trailing list's items once no later step can change them.
    pub(crate) fn complete(&mut self) {
        let Some(state) = self.take_list() else {
            return;
        };
        let taken = std::mem::take(self);
        *self = match Listing::start(taken, state) {
            Ok(listing) => listing.publish(),
            Err(output) => *output,
        };
    }
}

impl Listing {
    /// Take a list's item slots, pairing each with the output that fills it.
    ///
    /// A list's item history is installed only together with its list node, so a
    /// published list always ends with the node carrying its item slots. Failing to
    /// find one returns the whole output, so nothing is dropped.
    fn start(mut output: BlockTokens, state: Box<ListState>) -> Result<Self, Box<BlockTokens>> {
        debug_assert!(
            output
                .nodes
                .last()
                .is_some_and(|node| matches!(node.kind, Kind::List(..))),
            "a published list ends with the node carrying its item slots"
        );
        let Some(mut node) = output.nodes.pop() else {
            return Err(Box::new(output));
        };
        let kind = std::mem::replace(&mut node.kind, Kind::Gap);
        let (start, items) = match kind {
            Kind::List(start, items) => (start, items),
            kind => {
                node.kind = kind;
                output.nodes.push(node);
                return Err(Box::new(output));
            }
        };
        let (outputs, loose) = state.outputs();
        debug_assert_eq!(
            items.len(),
            outputs.len(),
            "every item slot of a list is filled by exactly one output"
        );
        let pending: Vec<(Node, BlockTokens)> = items.into_iter().zip(outputs).collect();
        Ok(Self {
            output,
            node,
            start,
            pending: pending.into_iter(),
            filled: Vec::new(),
            loose,
        })
    }

    /// Publish every item of this list, including the nested lists it holds.
    fn publish(mut self) -> BlockTokens {
        let mut parents: Vec<(Self, Node)> = Vec::new();
        loop {
            match self.step(&mut parents) {
                Ok(next) => self = next,
                Err(output) => return *output,
            }
        }
    }

    /// Descend into the nested list a slot holds, place a plain item, or close this level.
    fn step(mut self, parents: &mut Vec<(Self, Node)>) -> Result<Self, Box<BlockTokens>> {
        let Some((node, output, state)) = self.next() else {
            return self.close(parents);
        };
        let nested = match Self::start(output, state) {
            Ok(nested) => nested,
            Err(output) => {
                self.place(node, *output);
                return Ok(self);
            }
        };
        parents.push((self, node));
        Ok(nested)
    }

    /// Close this level into the level that suspended for it, or finish publication.
    fn close(self, parents: &mut Vec<(Self, Node)>) -> Result<Self, Box<BlockTokens>> {
        let Some((mut parent, node)) = parents.pop() else {
            return Err(Box::new(self.finish()));
        };
        parent.place(node, self.finish());
        Ok(parent)
    }

    /// Place item outputs until one needs its own nested publication.
    fn next(&mut self) -> Option<(Node, BlockTokens, Box<ListState>)> {
        while let Some((node, mut output)) = self.pending.next() {
            match output.take_list() {
                Some(state) => return Some((node, output, state)),
                None => self.place(node, output),
            }
        }
        None
    }

    /// Fill one item node and merge the output's retained inline operands.
    fn place(&mut self, mut node: Node, mut output: BlockTokens) {
        self.output
            .scopes
            .extend(std::mem::take(&mut output.scopes));
        debug_assert!(
            matches!(node.kind, Kind::Item(_)),
            "a list's item slot holds an item node"
        );
        if let Kind::Item(children) = &mut node.kind {
            *children = std::mem::take(&mut output.nodes);
        }
        if self.loose {
            loosen(&mut node);
        }
        self.filled.push(node);
    }

    /// Restore the filled item nodes into their list node.
    fn finish(mut self) -> BlockTokens {
        self.node.kind = Kind::List(self.start, std::mem::take(&mut self.filled));
        self.output.nodes.push(self.node);
        self.output
    }
}

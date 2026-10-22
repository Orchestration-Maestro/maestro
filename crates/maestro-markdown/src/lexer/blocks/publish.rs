//! Publication of a list's items, including the nested lists they contain.
use super::BlockTokens;
use crate::{Kind, Node, tokenizer::block::loosen};

/// One list whose item nodes are being filled.
struct Listing {
    /// Output owning the list node whose items are being filled.
    output: BlockTokens,
    /// Item nodes to fill, in order.
    items: Vec<Node>,
    /// Outputs of those items, in order.
    outputs: std::vec::IntoIter<BlockTokens>,
    /// Slot of the next output.
    next: usize,
    /// Whether every item of this list is loose.
    loose: bool,
}

impl BlockTokens {
    /// Publish the trailing list's items once no later step can change them.
    pub(crate) fn complete(&mut self) {
        if self.list.is_none() {
            return;
        }
        let mut listing = Listing::start(std::mem::take(self));
        let mut parents: Vec<(Listing, usize)> = Vec::new();
        loop {
            if let Some((index, output)) = listing.next() {
                parents.push((listing, index));
                listing = Listing::start(output);
                continue;
            }
            let Some((mut parent, index)) = parents.pop() else {
                *self = listing.finish();
                return;
            };
            parent.place(index, listing.finish());
            listing = parent;
        }
    }
}

impl Listing {
    /// Take a list's item nodes out of its own list node for filling.
    fn start(mut output: BlockTokens) -> Self {
        let items = match output.nodes.last_mut() {
            Some(Node {
                kind: Kind::List(_, items),
                ..
            }) => std::mem::take(items),
            _ => Vec::new(),
        };
        let (outputs, loose) = output
            .list
            .take()
            .map_or((Vec::new(), false), |list| list.outputs());
        Self {
            output,
            items,
            outputs: outputs.into_iter(),
            next: 0,
            loose,
        }
    }

    /// Place item outputs until one needs its own nested publication.
    fn next(&mut self) -> Option<(usize, BlockTokens)> {
        while let Some(output) = self.outputs.next() {
            let index = self.next;
            self.next += 1;
            if output.list.is_some() {
                return Some((index, output));
            }
            self.place(index, output);
        }
        None
    }

    /// Fill one item slot and merge the output's retained inline operands.
    fn place(&mut self, index: usize, mut output: BlockTokens) {
        self.output
            .scopes
            .extend(std::mem::take(&mut output.scopes));
        let Some(node) = self.items.get_mut(index) else {
            return;
        };
        if let Kind::Item(children) = &mut node.kind {
            *children = std::mem::take(&mut output.nodes);
        }
        if self.loose {
            loosen(node);
        }
    }

    /// Restore the filled item nodes into their list node.
    fn finish(mut self) -> BlockTokens {
        if let Some(Node {
            kind: Kind::List(_, items),
            ..
        }) = self.output.nodes.last_mut()
        {
            *items = std::mem::take(&mut self.items);
        }
        self.output
    }
}

//! Publication of a list's items, including the nested lists they contain.
use super::BlockTokens;
use crate::{Kind, Node, tokenizer::block::loosen};

/// One pending step of a publication, innermost last.
enum Frame {
    /// A list whose item nodes are filled in order.
    Listing(Box<Listing>),
    /// A published child output returning into its parent item slot.
    Child {
        /// Item slot awaiting the child.
        index: usize,
    },
}

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

/// Next action of one listing.
enum Progress {
    /// An item output whose own trailing list is published first.
    Nested {
        /// Item slot that output belongs to.
        index: usize,
        /// Output awaiting its nested publication.
        output: BlockTokens,
    },
    /// Every item output is placed.
    Done,
}

impl BlockTokens {
    /// Publish the trailing list's items once no later step can change them.
    pub(crate) fn complete(&mut self) {
        if self.list.is_none() {
            return;
        }
        let mut frames = vec![Frame::Listing(Box::new(Listing::start(std::mem::take(
            self,
        ))))];
        let mut published = None;
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Child { index } => place_child(&mut frames, &mut published, index),
                Frame::Listing(listing) => step(listing, &mut frames, &mut published),
            }
        }
        *self = published.unwrap_or_default();
    }
}

/// Hand a finished child output to its parent listing.
fn place_child(frames: &mut [Frame], published: &mut Option<BlockTokens>, index: usize) {
    let (Some(child), Some(Frame::Listing(parent))) = (published.take(), frames.last_mut()) else {
        return;
    };
    parent.place(index, child);
}

/// Advance one listing, queueing a nested publication when an item needs one.
fn step(listing: Box<Listing>, frames: &mut Vec<Frame>, published: &mut Option<BlockTokens>) {
    let mut listing = listing;
    match listing.next() {
        Progress::Done => *published = Some((*listing).finish()),
        Progress::Nested { index, output } => {
            frames.push(Frame::Listing(listing));
            frames.push(Frame::Child { index });
            frames.push(Frame::Listing(Box::new(Listing::start(output))));
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
    fn next(&mut self) -> Progress {
        while let Some(output) = self.outputs.next() {
            let index = self.next;
            self.next += 1;
            if output.list.is_some() {
                return Progress::Nested { index, output };
            }
            self.place(index, output);
        }
        Progress::Done
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

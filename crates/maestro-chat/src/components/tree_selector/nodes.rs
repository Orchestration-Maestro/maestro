//! Retained occurrence handles with flat, nonrecursive ownership.
use serde_json::Value;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
};

/// One occurrence's retained record, metadata and ordered children.
struct Slot {
    /// Shared immutable source record.
    entry: Rc<Value>,
    /// Raw optional label.
    label: RefCell<Option<Value>>,
    /// Raw optional timestamp.
    timestamp: RefCell<Option<Value>>,
    /// Child occurrence indices in supplied order.
    children: Vec<usize>,
}

/// A shared occurrence in a retained session-tree presentation.
#[derive(Clone)]
pub struct SessionTreeNode {
    /// Flat owner, without strong child-to-owner edges.
    arena: Rc<Vec<Slot>>,
    /// Physical occurrence identity.
    pub(super) index: usize,
}
impl SessionTreeNode {
    /// Borrows the shared immutable source record.
    #[must_use]
    pub fn entry(&self) -> &Rc<Value> {
        &self.arena[self.index].entry
    }
    /// Borrows the raw optional label.
    #[must_use]
    pub fn label(&self) -> Ref<'_, Option<Value>> {
        self.arena[self.index].label.borrow()
    }
    /// Mutably borrows this occurrence's raw optional label.
    #[must_use]
    pub fn label_mut(&self) -> RefMut<'_, Option<Value>> {
        self.arena[self.index].label.borrow_mut()
    }
    /// Borrows the raw optional label timestamp.
    #[must_use]
    pub fn label_timestamp(&self) -> Ref<'_, Option<Value>> {
        self.arena[self.index].timestamp.borrow()
    }
    /// Mutably borrows this occurrence's raw optional label timestamp.
    #[must_use]
    pub fn label_timestamp_mut(&self) -> RefMut<'_, Option<Value>> {
        self.arena[self.index].timestamp.borrow_mut()
    }
    /// Iterates children in supplied order, retaining the same flat owner.
    #[must_use]
    pub fn children(&self) -> impl ExactSizeIterator<Item = Self> + '_ {
        self.arena[self.index].children.iter().map(|&index| Self {
            arena: Rc::clone(&self.arena),
            index,
        })
    }
}
impl PartialEq for SessionTreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && Rc::ptr_eq(&self.arena, &other.arena)
    }
}
impl Eq for SessionTreeNode {}

/// Retains each source record once while keeping distinct physical occurrences.
pub(super) fn retain(nodes: &[&maestro_app::SessionTreeNode<'_>]) -> Vec<SessionTreeNode> {
    let indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (std::ptr::from_ref(*n), i))
        .collect();
    let mut entries = HashMap::new();
    let arena = Rc::new(
        nodes
            .iter()
            .map(|node| Slot {
                entry: Rc::clone(
                    entries
                        .entry(std::ptr::from_ref(node.entry))
                        .or_insert_with(|| Rc::new(node.entry.clone())),
                ),
                label: RefCell::new(node.label.cloned()),
                timestamp: RefCell::new(node.label_timestamp.cloned()),
                children: node
                    .children
                    .iter()
                    .map(|child| indices[&std::ptr::from_ref(child)])
                    .collect(),
            })
            .collect::<Vec<_>>(),
    );
    (0..nodes.len())
        .map(|index| SessionTreeNode {
            arena: Rc::clone(&arena),
            index,
        })
        .collect()
}

//! Clone-on-push snapshots for editable components.

/// A last-in, first-out stack of snapshots.
pub struct UndoStack<S> {
    /// Snapshots in capture order.
    snapshots: Vec<S>,
}

impl<S> Default for UndoStack<S> {
    fn default() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }
}

impl<S> UndoStack<S> {
    /// Creates an empty stack without requiring a default snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores the supplied type's clone.
    pub fn push(&mut self, state: &S)
    where
        S: Clone,
    {
        self.snapshots.push(state.clone());
    }

    /// Transfers the most recently captured snapshot.
    pub fn pop(&mut self) -> Option<S> {
        self.snapshots.pop()
    }

    /// Drops all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    /// Returns the snapshot count.
    #[must_use]
    pub fn length(&self) -> usize {
        self.snapshots.len()
    }
}

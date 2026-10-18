//! Identity-deduplicated callbacks traversed live in registration order.
use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
};

/// Callbacks in registration order with one admission index per live identity.
pub struct Listeners<P> {
    /// Identity of the next registration.
    next: u64,
    /// Current registrations by ordinal.
    entries: BTreeMap<u64, P>,
    /// Callback allocation addresses mapped to their live ordinal.
    identities: HashMap<usize, u64>,
}
impl<P> Default for Listeners<P> {
    fn default() -> Self {
        Self {
            next: 0,
            entries: BTreeMap::new(),
            identities: HashMap::new(),
        }
    }
}
impl<P: Deref> Listeners<P> {
    /// Appends a callback unless its allocation is already admitted.
    pub fn add(&mut self, listener: P) {
        let identity = std::ptr::from_ref(&*listener).cast::<()>().addr();
        if let std::collections::hash_map::Entry::Vacant(entry) = self.identities.entry(identity) {
            entry.insert(self.next);
            self.entries.insert(self.next, listener);
            self.next += 1;
        }
    }
    /// Removes the current identity, returning ownership for release outside guards.
    pub fn remove(&mut self, listener: *const P::Target) -> Option<P> {
        let key = self.identities.remove(&listener.cast::<()>().addr())?;
        self.entries.remove(&key)
    }
    /// Returns the first current registration at or after the ordinal cursor.
    #[must_use]
    pub fn next_from(&self, cursor: u64) -> Option<(u64, &P)> {
        self.entries
            .range(cursor..)
            .next()
            .map(|(id, listener)| (id + 1, listener))
    }
}

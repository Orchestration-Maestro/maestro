//! Identity-deduplicated callbacks traversed live in registration order.
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::Deref,
    rc::Rc,
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
impl<T: ?Sized> Listeners<Rc<T>> {
    /// Adds `listener` to the shared collection; the returned operation removes it and does
    /// nothing once the registration or the collection is gone.
    pub fn subscribe(listeners: &Rc<RefCell<Self>>, listener: Rc<T>) -> impl Fn() + use<T> {
        let identity = Rc::downgrade(&listener);
        let weak = Rc::downgrade(listeners);
        listeners.borrow_mut().add(listener);
        move || {
            if let Some(listeners) = weak.upgrade() {
                let removed = listeners.borrow_mut().remove(identity.as_ptr());
                drop(removed);
            }
        }
    }
    /// Calls `visit` with each listener in registration order, including ones added meanwhile,
    /// without holding the collection's borrow; stops at the first failure.
    ///
    /// # Errors
    /// Returns the first failure `visit` reports.
    pub fn try_for_each_live<F>(
        listeners: &RefCell<Self>,
        mut visit: impl FnMut(&T) -> Result<(), F>,
    ) -> Result<(), F> {
        let mut cursor = 0;
        loop {
            let next = listeners
                .borrow()
                .next_from(cursor)
                .map(|(next, listener)| (next, Rc::clone(listener)));
            let Some((next, listener)) = next else {
                return Ok(());
            };
            cursor = next;
            visit(&listener)?;
        }
    }
}

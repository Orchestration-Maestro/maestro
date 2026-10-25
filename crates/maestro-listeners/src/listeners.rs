//! Live registration storage with an identity index and traversable order.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::Deref,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// One traversal position over a [`Listeners`] collection.
///
/// A cursor holds bookkeeping only: not callbacks and not the registry. Keeping it
/// alive excludes slot compaction, so callbacks may run and the registry may change
/// while a traversal is between two positions, including across an await.
pub struct ListenerCursor {
    /// Index of the next slot this traversal inspects.
    next: usize,
    /// Traversals sharing the cursor's registry, used to reject a foreign owner.
    active: Arc<AtomicUsize>,
}

impl Drop for ListenerCursor {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Callbacks in registration order, deduplicated by identity.
///
/// A callback is admitted once per identity: adding a live one again keeps its first
/// position. Registration order is the traversal order, and a removed identity that is
/// added again takes the last position. Traversals see the collection as it is at each
/// step: callbacks added meanwhile are visited, callbacks removed before they are
/// reached are skipped.
pub struct Listeners<P> {
    /// Registrations in order; `None` marks a slot retired by removal.
    entries: Vec<Option<P>>,
    /// Callback identity of each live registration mapped to its slot.
    identities: HashMap<usize, usize>,
    /// Traversals sharing this collection.
    active: Arc<AtomicUsize>,
}

impl<P> Default for Listeners<P> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            identities: HashMap::new(),
            active: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl<P: Deref> Listeners<P> {
    /// Reclaims retired slots once no traversal runs and at least half are retired.
    ///
    /// A traversal in progress keeps every slot it may still reach, so compaction waits
    /// for it. Reclaiming inspects each retained slot once, which keeps the work spread
    /// over the registrations that retired them.
    fn compact(&mut self) {
        let live = self.identities.len();
        if self.active.load(Ordering::Relaxed) != 0 || live > self.entries.len() - live {
            return;
        }
        let identities = &mut self.identities;
        let mut slot = 0;
        self.entries.retain(|entry| {
            visit();
            let Some(listener) = entry else {
                return false;
            };
            identities.insert(identity(listener), slot);
            slot += 1;
            true
        });
    }

    /// Admits `listener` unless its identity is already registered.
    pub fn add(&mut self, listener: P) {
        self.compact();
        visit();
        if let std::collections::hash_map::Entry::Vacant(entry) =
            self.identities.entry(identity(&listener))
        {
            entry.insert(self.entries.len());
            self.entries.push(Some(listener));
        }
    }

    /// Removes the registration of `listener` and returns what it owned.
    ///
    /// The caller receives the callback so it is released outside any borrow of the
    /// collection. An identity that is not registered yields `None`.
    pub fn remove(&mut self, listener: *const P::Target) -> Option<P> {
        visit();
        let slot = self.identities.remove(&listener.cast::<()>().addr())?;
        let removed = self.entries[slot].take();
        self.compact();
        removed
    }

    /// Clones the registered handles in registration order.
    ///
    /// The returned handles retain their values independently of later registration
    /// edits or release of the collection. Payloads are not cloned separately.
    pub fn snapshot(&mut self) -> Vec<P>
    where
        P: Clone,
    {
        let mut cursor = self.cursor();
        std::iter::from_fn(|| self.next_from(&mut cursor).cloned()).collect()
    }

    /// Starts a traversal at the first registration.
    #[must_use]
    pub fn cursor(&mut self) -> ListenerCursor {
        self.compact();
        self.active.fetch_add(1, Ordering::Relaxed);
        ListenerCursor {
            next: 0,
            active: Arc::clone(&self.active),
        }
    }

    /// The first live registration at or after `cursor`, advancing that traversal.
    ///
    /// A cursor taken from another collection finds nothing.
    #[must_use]
    pub fn next_from(&self, cursor: &mut ListenerCursor) -> Option<&P> {
        if !Arc::ptr_eq(&self.active, &cursor.active) || self.identities.is_empty() {
            return None;
        }
        while let Some(entry) = self.entries.get(cursor.next) {
            cursor.next += 1;
            visit();
            if let Some(listener) = entry {
                return Some(listener);
            }
        }
        None
    }
}

impl<T: ?Sized> Listeners<Rc<T>> {
    /// Adds `listener` to the shared collection.
    ///
    /// The returned operation removes that identity and does nothing once the
    /// registration or the collection is gone.
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

    /// Calls `visit` with each registration in order without holding the borrow,
    /// including ones added meanwhile, and stops at the first failure.
    ///
    /// # Errors
    /// Returns the first failure `visit` reports.
    pub fn try_for_each_live<F>(
        listeners: &RefCell<Self>,
        mut visit: impl FnMut(&T) -> Result<(), F>,
    ) -> Result<(), F> {
        let mut cursor = listeners.borrow_mut().cursor();
        loop {
            let next = listeners.borrow().next_from(&mut cursor).map(Rc::clone);
            let Some(listener) = next else {
                return Ok(());
            };
            visit(&listener)?;
        }
    }
}

/// The allocation address that identifies one callback value.
fn identity<P: Deref>(listener: &P) -> usize {
    std::ptr::from_ref(&**listener).cast::<()>().addr()
}

#[cfg(test)]
thread_local! {
    /// Collection operations counted by traversal and removal tests.
    static VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Count one inspected slot or index operation.
fn visit() {
    #[cfg(test)]
    VISITS.set(VISITS.get() + 1);
}

#[cfg(test)]
/// Operations counted since the last reset.
pub(crate) fn visits() -> usize {
    VISITS.get()
}

#[cfg(test)]
/// Restart the operation count.
pub(crate) fn reset_visits() {
    VISITS.set(0);
}

#[cfg(test)]
mod tests;

//! Live callback registrations under a caller-chosen admission policy.

use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::Deref,
    rc::Rc,
};

/// How a collection treats a callback it already holds an allocation for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Admission {
    /// A callback whose allocation is already registered is not registered again.
    #[default]
    Once,
    /// Every registration is its own entry, as an event-emitter listener list keeps them.
    Every,
}

/// One admitted callback.
struct Registration<P> {
    /// Allocation address of the callback, kept only for [`Admission::Once`].
    identity: Option<usize>,
    /// The callback.
    listener: P,
}

/// Callbacks in registration order, each under the ordinal it was admitted with.
pub struct Listeners<P> {
    /// How a repeated callback is treated.
    admission: Admission,
    /// Ordinal the next registration takes.
    next: u64,
    /// Current registrations by ordinal.
    entries: BTreeMap<u64, Registration<P>>,
    /// Callback allocation addresses mapped to the one live ordinal that holds them.
    identities: HashMap<usize, u64>,
}

impl<P> Default for Listeners<P> {
    fn default() -> Self {
        Self::new(Admission::Once)
    }
}

impl<P> Listeners<P> {
    /// Creates a collection that treats repeated callbacks by `admission`.
    #[must_use]
    pub fn new(admission: Admission) -> Self {
        Self {
            admission,
            next: 0,
            entries: BTreeMap::new(),
            identities: HashMap::new(),
        }
    }
}

impl<P: Deref> Listeners<P> {
    /// Appends `listener` and returns the ordinal it was admitted under, or `None` when the
    /// collection already holds that callback.
    pub fn add(&mut self, listener: P) -> Option<u64> {
        let identity = std::ptr::from_ref(&*listener).cast::<()>().addr();
        let deduplicated = self.admission == Admission::Once;
        if deduplicated && self.identities.contains_key(&identity) {
            return None;
        }
        let ordinal = self.next;
        self.next += 1;
        if deduplicated {
            self.identities.insert(identity, ordinal);
        }
        self.entries.insert(
            ordinal,
            Registration {
                identity: deduplicated.then_some(identity),
                listener,
            },
        );
        Some(ordinal)
    }

    /// Removes the registration holding the callback at `listener`, returning its ownership.
    pub fn remove(&mut self, listener: *const P::Target) -> Option<P> {
        let ordinal = self.identities.remove(&listener.cast::<()>().addr())?;
        self.entries
            .remove(&ordinal)
            .map(|registration| registration.listener)
    }

    /// Removes the registration admitted as `ordinal`, returning its ownership; a repeated
    /// or unknown ordinal removes nothing.
    pub fn remove_at(&mut self, ordinal: u64) -> Option<P> {
        let registration = self.entries.remove(&ordinal)?;
        if let Some(identity) = registration.identity {
            self.identities.remove(&identity);
        }
        Some(registration.listener)
    }

    /// Returns the first current registration at or after the ordinal cursor.
    #[must_use]
    pub fn next_from(&self, cursor: u64) -> Option<(u64, &P)> {
        self.entries
            .range(cursor..)
            .next()
            .map(|(id, registration)| (id + 1, &registration.listener))
    }
}

impl<T: ?Sized> Listeners<Rc<T>> {
    /// Adds `listener` to the shared collection; the returned operation removes that
    /// registration and does nothing once it is gone or the collection is.
    pub fn subscribe(listeners: &Rc<RefCell<Self>>, listener: Rc<T>) -> impl Fn() + use<T> {
        let admitted = listeners.borrow_mut().add(listener);
        let weak = Rc::downgrade(listeners);
        move || {
            if let (Some(ordinal), Some(listeners)) = (admitted, weak.upgrade()) {
                drop(listeners.borrow_mut().remove_at(ordinal));
            }
        }
    }

    /// Calls `visit` with each live callback in registration order, including ones added
    /// meanwhile, without holding the collection's borrow.
    pub fn for_each_live<F: FnMut(&T)>(listeners: &RefCell<Self>, mut visit: F) {
        let mut cursor = 0;
        while let Some((next, listener)) = live(listeners, cursor) {
            cursor = next;
            visit(&listener);
        }
    }

    /// Calls `visit` with each live callback in registration order, including ones added
    /// meanwhile, without holding the collection's borrow; stops at the first failure.
    ///
    /// # Errors
    /// Returns the first failure `visit` reports.
    pub fn try_for_each_live<F>(
        listeners: &RefCell<Self>,
        mut visit: impl FnMut(&T) -> Result<(), F>,
    ) -> Result<(), F> {
        let mut cursor = 0;
        while let Some((next, listener)) = live(listeners, cursor) {
            cursor = next;
            visit(&listener)?;
        }
        Ok(())
    }
}

/// The next live registration at or after `cursor`, cloned so no borrow is held while it runs.
fn live<T: ?Sized>(listeners: &RefCell<Listeners<Rc<T>>>, cursor: u64) -> Option<(u64, Rc<T>)> {
    listeners
        .borrow()
        .next_from(cursor)
        .map(|(next, listener)| (next, Rc::clone(listener)))
}

#[cfg(test)]
mod tests;

//! Insertion-ordered map whose clones and walks share one live collection.

use maestro_listeners::{ListenerCursor, Listeners};
use std::{cell::RefCell, collections::HashMap, hash::Hash, iter::FusedIterator, rc::Rc};

/// One indexed value, shared with the ordered traversal owner.
struct Entry<K, V> {
    /// The original key.
    key: K,
    /// A snapshot can outlive the borrow while its value is cloned.
    value: RefCell<Rc<V>>,
}

/// The key index and the shared live-order owner.
struct Entries<K, V> {
    /// Current entries by key.
    index: HashMap<K, Rc<Entry<K, V>>>,
    /// Current entries in insertion order.
    order: Listeners<Rc<Entry<K, V>>>,
}

/// Entries in insertion order, shared by every clone and every walk.
///
/// Replacement keeps its position; removal followed by insertion appends.
/// A walk sees edits until it ends, then remains exhausted.
#[derive(Clone)]
pub struct Live<K, V>(Rc<RefCell<Entries<K, V>>>);

impl<K: Hash + Eq + Clone, V: Clone> Live<K, V> {
    /// An empty collection.
    #[must_use]
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Entries {
            index: HashMap::new(),
            order: Listeners::default(),
        })))
    }

    /// Sets `key`, releasing any replaced value outside collection borrows.
    pub fn insert(&self, key: K, value: V) {
        drop(self.replace(key, value));
    }

    /// Sets `key` without moving its position, returning its former value.
    pub fn replace(&self, key: K, value: V) -> Option<V> {
        let existing = self.0.borrow().index.get(&key).cloned();
        if let Some(entry) = existing {
            let old = entry.value.replace(Rc::new(value));
            return Some(Rc::unwrap_or_clone(old));
        }
        let entry = Rc::new(Entry {
            key: key.clone(),
            value: RefCell::new(Rc::new(value)),
        });
        let mut entries = self.0.borrow_mut();
        entries.order.add(Rc::clone(&entry));
        entries.index.insert(key, entry);
        None
    }

    /// Removes `key`, returning its value outside collection borrows.
    pub fn remove<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q>,
    {
        let (key, entry) = {
            let mut entries = self.0.borrow_mut();
            let removed = entries.index.remove_entry(key)?;
            drop(entries.order.remove(Rc::as_ptr(&removed.1)));
            removed
        };
        drop(key);
        let value = match Rc::try_unwrap(entry) {
            Ok(entry) => entry.value.into_inner(),
            Err(entry) => Rc::clone(&entry.value.borrow()),
        };
        Some(Rc::unwrap_or_clone(value))
    }

    /// Removes all current entries, releasing values outside collection borrows.
    pub fn clear(&self) {
        let removed = {
            let mut entries = self.0.borrow_mut();
            let removed = std::mem::take(&mut entries.index);
            for entry in removed.values() {
                drop(entries.order.remove(Rc::as_ptr(entry)));
            }
            removed
        };
        drop(removed);
    }

    /// The number of current entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.borrow().index.len()
    }

    /// Whether there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Starts a live insertion-ordered walk.
    #[must_use]
    pub fn iter(&self) -> LiveIter<K, V> {
        let cursor = self.0.borrow_mut().order.cursor();
        LiveIter {
            live: self.clone(),
            cursor,
            ended: false,
        }
    }

    /// The current value of `key`.
    #[must_use]
    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q>,
    {
        let entry = self.0.borrow().index.get(key).cloned()?;
        let value = Rc::clone(&entry.value.borrow());
        Some((*value).clone())
    }

    /// Whether `key` has a value.
    #[must_use]
    pub fn contains_key<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
    {
        self.0.borrow().index.contains_key(key)
    }
}

/// A walk over one shared map, retaining no removed values between steps.
pub struct LiveIter<K, V> {
    /// The shared collection.
    live: Live<K, V>,
    /// The ordered owner's next slot.
    cursor: ListenerCursor,
    /// Whether this iterator has returned `None`.
    ended: bool,
}

impl<K: Hash + Eq + Clone, V: Clone> Iterator for LiveIter<K, V> {
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let next = self
            .live
            .0
            .borrow()
            .order
            .next_from(&mut self.cursor)
            .cloned();
        let Some(entry) = next else {
            self.ended = true;
            return None;
        };
        let value = Rc::clone(&entry.value.borrow());
        Some((entry.key.clone(), (*value).clone()))
    }
}
impl<K: Hash + Eq + Clone, V: Clone> FusedIterator for LiveIter<K, V> {}
impl<K: Hash + Eq + Clone, V: Clone> IntoIterator for &Live<K, V> {
    type Item = (K, V);
    type IntoIter = LiveIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<K: Hash + Eq + Clone, V: Clone> Default for Live<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

//! Insertion-ordered map whose clones and walks share one live collection.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::iter::FusedIterator;
use std::rc::Rc;

/// The entries and the positions that order them.
struct Entries<K, V> {
    /// The position the next new key receives; positions are never reused.
    next_position: u64,
    /// The position of each key.
    positions: HashMap<K, u64>,
    /// The entries by position.
    by_position: BTreeMap<u64, (K, V)>,
}

/// Entries in insertion order, shared by every clone and every walk.
///
/// Replacing a value keeps its position; removing a key and inserting it again
/// moves it to the end. A walk reads the collection at each step: it skips an
/// entry removed before its turn, yields a replaced value, reaches entries
/// added before it ends and stays ended once it returns `None`.
#[derive(Clone)]
pub struct Live<K, V>(Rc<RefCell<Entries<K, V>>>);

impl<K: Hash + Eq + Clone, V: Clone> Live<K, V> {
    /// An empty collection.
    pub(super) fn new() -> Self {
        Self(Rc::new(RefCell::new(Entries {
            next_position: 0,
            positions: HashMap::new(),
            by_position: BTreeMap::new(),
        })))
    }

    /// Set `key` to `value`, appending the entry when the key is new.
    pub(super) fn insert(&self, key: K, value: V) {
        let mut entries = self.0.borrow_mut();
        let entries = &mut *entries;
        let position = *entries
            .positions
            .entry(key.clone())
            .or_insert(entries.next_position);
        entries.next_position = entries.next_position.max(position + 1);
        entries.by_position.insert(position, (key, value));
    }

    /// Remove `key`.
    pub(super) fn remove<Q: Hash + Eq + ?Sized>(&self, key: &Q)
    where
        K: std::borrow::Borrow<Q>,
    {
        let mut entries = self.0.borrow_mut();
        if let Some(position) = entries.positions.remove(key) {
            entries.by_position.remove(&position);
        }
    }

    /// Remove every entry.
    pub(super) fn clear(&self) {
        let mut entries = self.0.borrow_mut();
        entries.positions.clear();
        entries.by_position.clear();
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.borrow().by_position.len()
    }

    /// Whether there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entries in order, as owned pairs.
    #[must_use]
    pub fn iter(&self) -> LiveIter<K, V> {
        LiveIter {
            live: self.clone(),
            from: Some(0),
        }
    }
}

/// A walk over [`Live`] entries that observes changes made while it runs.
pub struct LiveIter<K, V> {
    /// The collection being walked.
    live: Live<K, V>,
    /// The first position not yet visited; `None` once the walk has ended.
    from: Option<u64>,
}

impl<K: Hash + Eq + Clone, V: Clone> Iterator for LiveIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<(K, V)> {
        let from = self.from.take()?;
        let entries = self.live.0.borrow();
        let (position, (key, value)) = entries.by_position.range(from..).next()?;
        self.from = Some(position + 1);
        Some((key.clone(), value.clone()))
    }
}

impl<K: Hash + Eq + Clone, V: Clone> FusedIterator for LiveIter<K, V> {}

impl Live<String, String> {
    /// The value of `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        let entries = self.0.borrow();
        let (_, value) = entries.by_position.get(entries.positions.get(key)?)?;
        Some(value.clone())
    }

    /// Whether `key` has a value.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.borrow().positions.contains_key(key)
    }
}

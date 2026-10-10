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
pub struct Live<K, V> {
    /// The shared collection.
    entries: Rc<RefCell<Entries<K, V>>>,
}

impl<K, V> Clone for Live<K, V> {
    fn clone(&self) -> Self {
        Self {
            entries: Rc::clone(&self.entries),
        }
    }
}

impl<K: Hash + Eq + Clone, V: Clone> Live<K, V> {
    /// An empty collection.
    pub(super) fn new() -> Self {
        Self {
            entries: Rc::new(RefCell::new(Entries {
                next_position: 0,
                positions: HashMap::new(),
                by_position: BTreeMap::new(),
            })),
        }
    }

    /// Set `key` to `value`, appending the entry when the key is new.
    pub(super) fn insert(&self, key: K, value: V) {
        let mut entries = self.entries.borrow_mut();
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
        let mut entries = self.entries.borrow_mut();
        if let Some(position) = entries.positions.remove(key) {
            entries.by_position.remove(&position);
        }
    }

    /// Remove every entry.
    pub(super) fn clear(&self) {
        let mut entries = self.entries.borrow_mut();
        entries.positions.clear();
        entries.by_position.clear();
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.borrow().by_position.len()
    }

    /// Whether there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The value of `key`.
    #[must_use]
    pub fn get<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> Option<V>
    where
        K: std::borrow::Borrow<Q>,
    {
        let entries = self.entries.borrow();
        let (_, value) = entries.by_position.get(entries.positions.get(key)?)?;
        Some(value.clone())
    }

    /// Whether `key` has a value.
    #[must_use]
    pub fn contains_key<Q: Hash + Eq + ?Sized>(&self, key: &Q) -> bool
    where
        K: std::borrow::Borrow<Q>,
    {
        self.entries.borrow().positions.contains_key(key)
    }

    /// The entries in order, as owned pairs.
    #[must_use]
    pub fn iter(&self) -> LiveIter<K, V> {
        LiveIter {
            live: self.clone(),
            from: 0,
            ended: false,
        }
    }
}

impl<K: Hash + Eq + Clone, V: Clone> IntoIterator for &Live<K, V> {
    type Item = (K, V);
    type IntoIter = LiveIter<K, V>;

    fn into_iter(self) -> LiveIter<K, V> {
        self.iter()
    }
}

/// A walk over [`Live`] entries that observes changes made while it runs.
pub struct LiveIter<K, V> {
    /// The collection being walked.
    live: Live<K, V>,
    /// The first position not yet visited.
    from: u64,
    /// Whether the walk has ended.
    ended: bool,
}

impl<K: Hash + Eq + Clone, V: Clone> Iterator for LiveIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<(K, V)> {
        let entries = self.live.entries.borrow();
        let next = entries.by_position.range(self.from..).next();
        let Some((position, (key, value))) = next.filter(|_| !self.ended) else {
            self.ended = true;
            return None;
        };
        self.from = position + 1;
        Some((key.clone(), value.clone()))
    }
}

impl<K: Hash + Eq + Clone, V: Clone> FusedIterator for LiveIter<K, V> {}

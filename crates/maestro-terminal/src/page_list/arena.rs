//! Generation-checked owned slots shared by pages and retained positions.

use super::NodeId;
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes keys produced by independently owned tables.
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// One reusable slot with its current lifetime.
struct Slot<T> {
    /// Lifetime incremented before reuse.
    generation: u64,
    /// Occupied value, absent after release.
    value: Option<T>,
}

/// Private storage whose keys reject foreign owners and released lifetimes.
pub(super) struct Arena<T> {
    /// Identity shared by this table's keys.
    owner: u64,
    /// Stable indices, including vacant slots.
    slots: Vec<Slot<T>>,
    /// Vacant indices available for reuse.
    free: Vec<usize>,
}

impl<T> Arena<T> {
    /// Create an independently identified table.
    pub(super) fn new() -> Self {
        Self {
            owner: NEXT_OWNER.fetch_add(1, Ordering::Relaxed),
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Admit a value and return its current lifetime key.
    pub(super) fn insert(&mut self, value: T) -> NodeId {
        let slot = if let Some(index) = self.free.pop() {
            self.slots[index].value = Some(value);
            index
        } else {
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
            self.slots.len() - 1
        };
        NodeId {
            arena: self.owner,
            slot,
            generation: self.slots[slot].generation,
        }
    }

    /// Borrow a current value, rejecting foreign or obsolete keys.
    pub(super) fn get(&self, key: NodeId) -> Option<&T> {
        #[cfg(test)]
        crate::tests::WORK
            .with_borrow_mut(|work| *work.queries.entry(self.owner).or_default() += 1);
        if key.arena != self.owner {
            return None;
        }
        let slot = self.slots.get(key.slot)?;
        (slot.generation == key.generation)
            .then_some(slot.value.as_ref())
            .flatten()
    }

    /// Mutably borrow a current value.
    pub(super) fn get_mut(&mut self, key: NodeId) -> Option<&mut T> {
        if key.arena != self.owner {
            return None;
        }
        let slot = self.slots.get_mut(key.slot)?;
        (slot.generation == key.generation)
            .then_some(slot.value.as_mut())
            .flatten()
    }

    /// Mutably borrow two distinct live values without moving their ownership.
    pub(super) fn get_pair_mut(
        &mut self,
        first: NodeId,
        second: NodeId,
    ) -> Option<(&mut T, &mut T)> {
        self.get(first)?;
        self.get(second)?;
        if first.slot == second.slot {
            return None;
        }
        let (low, high, reverse) = if first.slot < second.slot {
            (first.slot, second.slot, false)
        } else {
            (second.slot, first.slot, true)
        };
        let (left, right) = self.slots.split_at_mut(high);
        let pair = (left[low].value.as_mut()?, right[0].value.as_mut()?);
        Some(if reverse { (pair.1, pair.0) } else { pair })
    }

    /// Release ownership and retire the key before the slot can be reused.
    pub(super) fn remove(&mut self, key: NodeId) -> Option<T> {
        if key.arena != self.owner {
            return None;
        }
        let slot = self.slots.get_mut(key.slot)?;
        if slot.generation != key.generation {
            return None;
        }
        let value = slot.value.take()?;
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.free.push(key.slot);
        }
        Some(value)
    }

    /// Enumerate occupied slots without changing their lifetimes.
    pub(super) fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            Some((
                NodeId {
                    arena: self.owner,
                    slot: index,
                    generation: slot.generation,
                },
                slot.value.as_ref()?,
            ))
        })
    }

    /// Mutably enumerate occupied slots while retaining their lifetime keys.
    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut T)> {
        let owner = self.owner;
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(move |(index, slot)| {
                Some((
                    NodeId {
                        arena: owner,
                        slot: index,
                        generation: slot.generation,
                    },
                    slot.value.as_mut()?,
                ))
            })
    }

    /// Mutably visit occupied values without exposing their storage.
    pub(super) fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.iter_mut().map(|(_, value)| value)
    }
}

//! Borrowed and owned operands selected from the active parent.

use serde_json::Value;

#[derive(Clone)]
/// A direct selection from the containing frame's current operand.
pub(in super::super) enum Instance {
    /// Apply a schema to the containing value itself.
    Same,
    /// Read the current member by its declared or captured key.
    Key(String),
    /// Check a required property even when an earlier callback deleted it.
    RequiredKey(String),
    /// Read the current array member at this position.
    Index(usize),
    /// Check a detached property-name string.
    Name(String),
}
/// The current value has one owner; read-only entry borrows the caller's value.
pub(super) enum Operand<'v> {
    /// A deleted required member reached by compiled checking, distinct from JSON null.
    Absent,
    /// Borrow the original value for read-only checking.
    Read(&'v Value),
    /// Own the current value exclusively while callbacks edit it.
    Write(Value),
}
impl<'v> Operand<'v> {
    /// Borrow the current value without materializing a subtree.
    pub(super) fn value(&self) -> Option<&Value> {
        match self {
            Self::Absent => None,
            Self::Read(v) => Some(v),
            Self::Write(v) => Some(v),
        }
    }
    /// Return an exclusive value, copying only if this was the borrowed entry.
    pub(super) fn into_owned(self) -> Option<Value> {
        match self {
            Self::Absent => None,
            Self::Read(value) => Some(value.clone()),
            Self::Write(value) => Some(value),
        }
    }
    /// Move a writable child out, or borrow a read-only child.
    pub(super) fn select(&mut self, selector: &Instance) -> Option<Self> {
        match self {
            Self::Absent => matches!(selector, Instance::Same).then_some(Self::Absent),
            Self::Read(value) => {
                let value: &'v Value = value;
                match selector {
                    Instance::Same => Some(Self::Read(value)),
                    Instance::Key(k) => value.get(k).map(Self::Read),
                    Instance::RequiredKey(k) => Some(value.get(k).map_or(Self::Absent, Self::Read)),
                    Instance::Index(i) => value.get(*i).map(Self::Read),
                    Instance::Name(k) => Some(Self::Write(Value::String(k.clone()))),
                }
            }
            Self::Write(value) => match selector {
                Instance::Same => Some(Self::Write(std::mem::take(value))),
                Instance::Key(k) => value.get_mut(k).map(|v| Self::Write(std::mem::take(v))),
                Instance::RequiredKey(k) => Some(
                    value
                        .get_mut(k)
                        .map_or(Self::Absent, |v| Self::Write(std::mem::take(v))),
                ),
                Instance::Index(i) => value.get_mut(*i).map(|v| Self::Write(std::mem::take(v))),
                Instance::Name(k) => Some(Self::Write(Value::String(k.clone()))),
            },
        }
    }
    /// Restore into the suspended, unmodified parent slot.
    pub(super) fn restore(&mut self, selector: &Instance, child: Self) {
        let (Self::Write(parent), Self::Write(child)) = (self, child) else {
            return;
        };
        let slot = match selector {
            Instance::Same => Some(parent),
            Instance::Key(key) | Instance::RequiredKey(key) => parent.get_mut(key),
            Instance::Index(index) => parent.get_mut(*index),
            Instance::Name(_) => None,
        };
        if let Some(slot) = slot {
            *slot = child;
        }
    }
}

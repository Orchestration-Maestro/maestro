//! Captured preference paths shared by cache publication and fresh-document merging.

use serde_json::{Map, Value};

/// One edited preference, from its top-level field through any nested keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Edit(Vec<String>);

impl Edit {
    /// Captures a dotted lock path supplied by a base-layer publisher.
    pub(super) fn dotted(path: &str) -> Self {
        Self(path.split('.').map(str::to_owned).collect())
    }

    /// Reads the captured preference from an accepted snapshot.
    pub(super) fn value_in<'a>(&self, document: &'a Map<String, Value>) -> Option<&'a Value> {
        let (field, keys) = self.0.split_first()?;
        keys.iter()
            .try_fold(document.get(field)?, |value, key| value.get(key))
    }

    /// Sets or removes precisely this preference without copying stale sibling values.
    pub(super) fn apply(&self, mut document: &mut Map<String, Value>, value: Option<Value>) {
        let Some((key, parents)) = self.0.split_last() else {
            return;
        };
        for parent in parents {
            if value.is_none() && !document.get(parent).is_some_and(Value::is_object) {
                return;
            }
            let slot = document
                .entry(parent.clone())
                .or_insert_with(|| Value::Object(Map::new()));
            if !slot.is_object() {
                *slot = Value::Object(Map::new());
            }
            let Value::Object(next) = slot else {
                return;
            };
            document = next;
        }
        if let Some(value) = value {
            document.insert(key.clone(), value);
        } else {
            document.shift_remove(key);
        }
    }
}

/// A new value or removal intent at one nonempty preference path.
pub(super) struct Change {
    /// Captured field and nested key identity.
    edit: Edit,
    /// Value to publish, or absence for removal.
    value: Option<Value>,
}

impl Change {
    /// Captures a public edit whose nonempty path was checked before admission.
    pub(super) fn from_requested(change: super::SettingsChange) -> Self {
        Self {
            edit: Edit(change.path),
            value: change.value,
        }
    }

    /// Captures precisely the paths a one-level runtime merge can replace.
    pub(super) fn overrides(document: &Map<String, Value>) -> Vec<Self> {
        document
            .iter()
            .flat_map(|(field, value)| {
                if let Value::Object(object) = value
                    && !object.is_empty()
                {
                    return object
                        .iter()
                        .map(|(key, value)| Self::key(field, key, value.clone()))
                        .collect();
                }
                vec![Self::field(field, Some(value.clone()))]
            })
            .collect()
    }

    /// Sets or removes a top-level member.
    pub(super) fn field(field: &str, value: Option<Value>) -> Self {
        Self::path(field, &[], value)
    }

    /// Sets one key of a top-level object member.
    pub(super) fn key(field: &str, key: &str, value: Value) -> Self {
        Self::path(field, &[key], Some(value))
    }

    /// Sets or removes a nested preference, keeping each path component literal.
    pub(super) fn path(field: &str, keys: &[&str], value: Option<Value>) -> Self {
        Self {
            edit: Edit(
                std::iter::once(field)
                    .chain(keys.iter().copied())
                    .map(str::to_owned)
                    .collect(),
            ),
            value,
        }
    }

    /// Borrows the literal components of this requested edit.
    pub(super) fn components(&self) -> Vec<&str> {
        self.edit.0.iter().map(String::as_str).collect()
    }

    /// Copies this edit for the publisher without exposing persistence internals.
    pub(super) fn requested(&self) -> super::SettingsChange {
        super::SettingsChange {
            path: self.edit.0.clone(),
            value: self.value.clone(),
        }
    }

    /// Publishes the value immediately and returns its captured persistence identity.
    pub(super) fn apply(self, document: &mut Map<String, Value>) -> Edit {
        self.edit.apply(document, self.value);
        self.edit
    }
}

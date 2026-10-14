//! Recorded keys and registry changes dispatched through public interfaces.
use super::bindings::{Binding, mutate, replace};
use maestro_chat::TreeList;
use serde::Deserialize;
use std::collections::BTreeMap;
/// A key event or an active manager change.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Operation {
    /// Raw protocol bytes.
    Key(String),
    /// Manager replacement or live configuration change.
    Registry(RegistryOperation),
}
/// The two recorded registry operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegistryOperation {
    /// Replacement manager's overrides.
    replace_bindings: Option<BTreeMap<String, Binding>>,
    /// Live manager's replacement overrides.
    set_bindings: Option<BTreeMap<String, Binding>>,
}
/// Drives a key event or caller configuration change through the existing manager.
pub fn apply_operation(list: &TreeList, operation: &Operation) {
    match operation {
        Operation::Key(data) => list.handle_input(data).unwrap(),
        Operation::Registry(change) => {
            assert_ne!(
                change.replace_bindings.is_some(),
                change.set_bindings.is_some()
            );
            if let Some(values) = &change.replace_bindings {
                replace(values);
            }
            if let Some(values) = &change.set_bindings {
                mutate(values);
            }
        }
    }
}

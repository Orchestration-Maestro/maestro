//! Original file-effect operands on the existing nested invocation route.
use wasmtime::component::{ComponentType, Lower};

/// Original operation operands of a nested file observation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalRead {
    /// Original guest operations identity.
    pub(super) target: u32,
    /// The requested observation.
    pub(super) operation: crate::guest_calls::read::Operation,
    /// Exact absolute authored path.
    pub(super) path: String,
}

/// Original operation operands of a nested file effect.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalWrite {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested effect and its original content.
    pub(super) operation: crate::guest_calls::write::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

/// Original operation operands of a nested edit effect.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalEdit {
    /// Original guest effect identity.
    pub(super) target: u32,
    /// Requested edit effect and its original content.
    pub(super) operation: crate::guest_calls::edit::Operation,
    /// Exact absolute target.
    pub(super) path: String,
}

//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type SessionCompactEvent =
    maestro_request::extension_events::session_events::SessionCompactEvent<crate::CompactionEntry>;
/// Shared schema specialized for the guest operands.
pub type TreePreparation =
    maestro_request::extension_events::session_events::TreePreparation<crate::SessionEntry>;
/// Shared schema specialized for the guest operands.
pub type SessionTreeEvent =
    maestro_request::extension_events::session_events::SessionTreeEvent<crate::BranchSummaryEntry>;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeTreeSummary =
    maestro_request::extension_events::session_events::SessionBeforeTreeSummary<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeTreeResult =
    maestro_request::extension_events::session_events::SessionBeforeTreeResult<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeTreeEvent =
    maestro_request::extension_events::session_events::SessionBeforeTreeEvent<
        TreePreparation,
        crate::AbortSignal,
    >;
/// Resource-free before-tree transport fields.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) struct SessionBeforeTreeEventData {
    /// Supplied navigation preparation.
    pub(crate) preparation: TreePreparation,
}

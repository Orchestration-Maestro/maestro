//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type ModelSelectSource = maestro_request::extension_events::selection_events::ModelSelectSource;
/// Shared schema specialized for the guest operands.
pub type ModelSelectEvent =
    maestro_request::extension_events::selection_events::ModelSelectEvent<crate::Model>;
/// Shared schema specialized for the guest operands.
pub type ThinkingLevelSelectEvent =
    maestro_request::extension_events::selection_events::ThinkingLevelSelectEvent;

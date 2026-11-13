//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type UserBashEvent = maestro_interchange::extension_events::tool_results::UserBashEvent;
/// Shared schema specialized for the guest operands.
pub type UserBashEventResult =
    maestro_interchange::extension_events::tool_results::UserBashEventResult<
        crate::BashOperations,
        crate::BashResult,
    >;
/// Shared schema specialized for the guest operands.
pub type ToolCallEventResult =
    maestro_interchange::extension_events::tool_results::ToolCallEventResult;
/// Shared schema specialized for the guest operands.
pub type ToolResultEventResult =
    maestro_interchange::extension_events::tool_results::ToolResultEventResult<crate::JsonText>;

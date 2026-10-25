//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type ContextEventResult =
    maestro_request::extension_events::agent_results::ContextEventResult<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type MessageEndEventResult =
    maestro_request::extension_events::agent_results::MessageEndEventResult<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type BeforeAgentStartEventResult =
    maestro_request::extension_events::agent_results::BeforeAgentStartEventResult<
        maestro_request::messages::CustomMessageInput<crate::JsonText>,
    >;
/// Custom message supplied before starting.
pub type BeforeAgentStartMessage = maestro_request::messages::CustomMessageInput<crate::JsonText>;

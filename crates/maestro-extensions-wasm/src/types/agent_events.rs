//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type ContextEvent =
    maestro_request::extension_events::agent_events::ContextEvent<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type BeforeAgentStartEvent =
    maestro_request::extension_events::agent_events::BeforeAgentStartEvent<
        maestro_request::system_prompt::BuildSystemPromptOptions,
    >;
/// Shared schema specialized for the guest operands.
pub type AgentStartEvent = maestro_request::extension_events::agent_events::AgentStartEvent;
/// Shared schema specialized for the guest operands.
pub type AgentEndEvent =
    maestro_request::extension_events::agent_events::AgentEndEvent<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type TurnStartEvent = maestro_request::extension_events::agent_events::TurnStartEvent;
/// Shared schema specialized for the guest operands.
pub type TurnEndEvent =
    maestro_request::extension_events::agent_events::TurnEndEvent<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type MessageStartEvent =
    maestro_request::extension_events::agent_events::MessageStartEvent<crate::AgentMessage>;
/// Shared schema specialized for the guest operands.
pub type MessageUpdateEvent = maestro_request::extension_events::agent_events::MessageUpdateEvent<
    crate::AgentMessage,
    crate::AssistantMessageEvent,
>;
/// Shared schema specialized for the guest operands.
pub type MessageEndEvent =
    maestro_request::extension_events::agent_events::MessageEndEvent<crate::AgentMessage>;

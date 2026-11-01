//! Native custom-message fixtures shared by registry and browser consumers.
#![cfg(test)]
use maestro_app::{AgentMessage, CustomAgentMessages};
use maestro_web::{MessageRenderer, tools::types::ToolUiError};
use std::{
    rc::Rc,
    sync::{Arc, RwLock, atomic::AtomicU32},
};

/// Open role carrying a non-serializable live handle.
pub struct NativeMessage {
    /// Caller-selected role.
    pub role: String,
    /// Original mutable native identity.
    pub handle: Arc<AtomicU32>,
}
impl CustomAgentMessages for NativeMessage {
    fn role(&self) -> &str {
        &self.role
    }
}
/// A supplied typed callback, without JSON conversion or retained frame operands.
pub type Callback = dyn Fn(
    &AgentMessage<NativeMessage>,
) -> Result<crate::tool_renderer_support::PreparedContent, ToolUiError>;
/// Ordinary user renderer for the native fixture.
pub struct Renderer(pub Rc<Callback>);
impl MessageRenderer<NativeMessage> for Renderer {
    fn render(
        &self,
        message: &AgentMessage<NativeMessage>,
    ) -> Result<crate::tool_renderer_support::PreparedContent, ToolUiError> {
        (self.0)(message)
    }
}
/// Wrap the original native handle in the shared message owner.
pub fn message(role: &str, handle: Arc<AtomicU32>) -> AgentMessage<NativeMessage> {
    AgentMessage::Custom(Arc::new(RwLock::new(NativeMessage {
        role: role.into(),
        handle,
    })))
}

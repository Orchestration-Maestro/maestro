//! Role-keyed custom content with typed retained callback ownership.
use crate::tools::types::{ToolContent, ToolUiError};
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages, ToolRendererState};
use std::{collections::HashMap, rc::Rc};

/// Caller-defined message-role vocabulary.
pub type MessageRole = String;
/// Prepare owned content from the caller's original shared message.
pub trait MessageRenderer<C: CustomAgentMessages = NoCustomMessages> {
    /// Prepare a recipe without retaining the message borrow or a drawing frame.
    /// # Errors
    /// Returns the supplied renderer's failure.
    fn render(&self, message: &AgentMessage<C>) -> Result<Box<dyn ToolContent>, ToolUiError>;
}
/// One typed table per custom-message representation.
type Renderers<C> = HashMap<MessageRole, Rc<dyn MessageRenderer<C>>>;
thread_local! {
    /// Existing typed erasure owner, local to the browser's rendering thread.
    static MESSAGE_RENDERERS: ToolRendererState = ToolRendererState::default();
}
/// Replace the current role callback without invalidating retained handles.
pub fn register_message_renderer<C: CustomAgentMessages + 'static>(
    role: &str,
    renderer: Rc<dyn MessageRenderer<C>>,
) {
    let table = MESSAGE_RENDERERS.with(ToolRendererState::state::<Renderers<C>>);
    let previous = table.borrow_mut().insert(role.into(), renderer);
    drop(previous);
}
/// Retain the current role callback for this message representation.
#[must_use]
pub fn get_message_renderer<C: CustomAgentMessages + 'static>(
    role: &str,
) -> Option<Rc<dyn MessageRenderer<C>>> {
    let table = MESSAGE_RENDERERS.with(ToolRendererState::state::<Renderers<C>>);
    table.borrow().get(role).cloned()
}
/// Prepare registered content, or no content for an unregistered role.
/// # Errors
/// Returns the reached renderer's original error without converting it to empty content.
pub fn render_message<C: CustomAgentMessages + 'static>(
    message: &AgentMessage<C>,
) -> Result<Option<Box<dyn ToolContent>>, ToolUiError> {
    let role = match message {
        AgentMessage::User(_) => "user".into(),
        AgentMessage::Assistant(_) => "assistant".into(),
        AgentMessage::ToolResult(_) => "toolResult".into(),
        AgentMessage::Custom(value) => value
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .role()
            .to_owned(),
    };
    get_message_renderer(&role)
        .map(|renderer| renderer.render(message))
        .transpose()
}

//! Role-keyed custom content with typed retained callback ownership.
use crate::{dom::PreparedContent, tools::types::ToolUiError};
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages, ToolRendererState};
use std::{collections::HashMap, rc::Rc};

/// Caller-defined message-role vocabulary.
pub type MessageRole = String;
/// Prepare owned content from the caller's original shared message.
///
/// `T` selects the content representation. By default it is page content
/// ([`crate::dom::DomContent`]) in the browser and the owned prepared record on native targets.
pub trait MessageRenderer<C: CustomAgentMessages = NoCustomMessages, T = PreparedContent> {
    /// Prepare content without retaining the message borrow.
    /// # Errors
    /// Returns the supplied renderer's failure.
    fn render(&self, message: &AgentMessage<C>) -> Result<T, ToolUiError>;
}
/// One typed table per message and content representation.
type Renderers<C, T> = HashMap<MessageRole, Rc<dyn MessageRenderer<C, T>>>;
thread_local! {
    /// Existing typed erasure owner, local to the browser's rendering thread.
    static MESSAGE_RENDERERS: ToolRendererState = ToolRendererState::default();
}
/// Replace the current role callback without invalidating retained handles.
pub fn register_message_renderer<C: CustomAgentMessages + 'static, T: 'static>(
    role: &str,
    renderer: Rc<dyn MessageRenderer<C, T>>,
) {
    let table = MESSAGE_RENDERERS.with(ToolRendererState::state::<Renderers<C, T>>);
    let previous = table.borrow_mut().insert(role.into(), renderer);
    drop(previous);
}
/// Retain the current role callback for the default content representation.
#[must_use]
pub fn get_message_renderer<C: CustomAgentMessages + 'static>(
    role: &str,
) -> Option<Rc<dyn MessageRenderer<C>>> {
    retain::<C, PreparedContent>(role)
}
/// Retain the callback registered for this role and content representation.
fn retain<C: CustomAgentMessages + 'static, T: 'static>(
    role: &str,
) -> Option<Rc<dyn MessageRenderer<C, T>>> {
    let table = MESSAGE_RENDERERS.with(ToolRendererState::state::<Renderers<C, T>>);
    table.borrow().get(role).cloned()
}
/// Remove a role's callback only while `renderer` is still the registered one.
#[cfg(target_arch = "wasm32")]
pub(crate) fn unregister_message_renderer<C: CustomAgentMessages + 'static, T: 'static>(
    role: &str,
    renderer: &Rc<dyn MessageRenderer<C, T>>,
) -> bool {
    let table = MESSAGE_RENDERERS.with(ToolRendererState::state::<Renderers<C, T>>);
    let mut table = table.borrow_mut();
    let owned = table
        .get(role)
        .is_some_and(|current| Rc::ptr_eq(current, renderer));
    let removed = owned.then(|| table.remove(role));
    drop(table);
    drop(removed);
    owned
}
/// Prepare registered content, or none for an unregistered role.
/// # Errors
/// Returns the reached renderer's original error without converting it to empty content.
pub fn render_message<C: CustomAgentMessages + 'static>(
    message: &AgentMessage<C>,
) -> Result<Option<PreparedContent>, ToolUiError> {
    render::<C, PreparedContent>(message)
}
/// Prepare the content its role's registered renderer produces.
fn render<C: CustomAgentMessages + 'static, T: 'static>(
    message: &AgentMessage<C>,
) -> Result<Option<T>, ToolUiError> {
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
    retain::<C, T>(&role)
        .map(|renderer| renderer.render(message))
        .transpose()
}
/// Page-content message lookups, the same role map as the root.
#[cfg(target_arch = "wasm32")]
pub mod dom {
    pub use super::{get_message_renderer, render_message};
}

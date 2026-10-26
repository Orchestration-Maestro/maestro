//! Completed transcript history with one result index and rows placed by emitted position.
#[cfg(any(target_arch = "wasm32", test))]
use crate::{
    components::{
        message_renderer_registry::render_message,
        messages::{AssistantMessage, ToolResults, UserMessage},
    },
    dom::PreparedContent,
    tools::types::ToolUiError,
};
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::ToolResultMessage;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::browser::components::messages::{
    is_artifact_message, is_user_message_with_attachments,
};
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages};
#[cfg(any(target_arch = "wasm32", test))]
use std::{collections::HashMap, sync::PoisonError};
use std::{
    collections::HashSet,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The live local tools forwarded to each assistant row.
type Tools = Rc<RwLock<Vec<maestro_app::SharedAgentTool<maestro_app::Local>>>>;
/// One shared transcript entry.
type Message<C> = AgentMessage<BrowserMessages<C>>;
/// The shared transcript the list reads.
type History<C> = Arc<RwLock<Vec<Message<C>>>>;
/// Original result envelopes by exact call ID.
#[cfg(any(target_arch = "wasm32", test))]
type Results = HashMap<String, Arc<RwLock<ToolResultMessage>>>;

/// Completed messages of a session, each shown by a registered role renderer or a built-in row.
///
/// Artifact entries are kept for session restore only and never shown. Rows are placed by their
/// emitted position, so an unchanged position keeps its root when entries before it are filtered.
pub struct MessageList<C: CustomAgentMessages = NoCustomMessages> {
    /// The shared history, read in order without holding it across renderer callbacks.
    pub messages: History<C>,
    /// Local tools forwarded to each assistant row.
    pub tools: Tools,
    /// Calls still waiting for their results.
    pub pending_tool_calls: Option<Rc<HashSet<String>>>,
    /// Whether a response is streaming; assistant rows then hide their pending calls.
    pub is_streaming: bool,
    /// Usage action forwarded to each assistant row.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
}
impl<C: CustomAgentMessages> Default for MessageList<C> {
    fn default() -> Self {
        Self {
            messages: Arc::default(),
            tools: Rc::default(),
            pending_tool_calls: None,
            is_streaming: false,
            on_cost_click: None,
        }
    }
}
impl<C: CustomAgentMessages + 'static> MessageList<C> {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
#[cfg(any(target_arch = "wasm32", test))]
impl<C: CustomAgentMessages + 'static> MessageList<C> {
    /// Prepare every shown row in emitted order: one pass indexes the results, a second reads
    /// the live history one entry at a time.
    /// # Errors
    /// Returns the first error a registered role renderer reports.
    pub(in crate::components) fn prepare(&self) -> Result<Vec<PreparedContent>, ToolUiError> {
        let results = {
            let history = self.messages.read().unwrap_or_else(PoisonError::into_inner);
            Rc::new(index(history.iter()))
        };
        let mut position = 0;
        self.emit(
            || {
                let history = self.messages.read().unwrap_or_else(PoisonError::into_inner);
                let next = history.get(position).cloned();
                position += 1;
                next
            },
            &results,
        )
    }
    /// Prepare the rows of each entry `next` yields until it yields none.
    fn emit(
        &self,
        mut next: impl FnMut() -> Option<Message<C>>,
        results: &Rc<Results>,
    ) -> Result<Vec<PreparedContent>, ToolUiError> {
        let mut rows = Vec::new();
        while let Some(message) = next() {
            if let Some(row) = self.row(message, results)? {
                rows.push(row);
            }
        }
        Ok(rows)
    }
    /// The row for one entry: an artifact shows nothing, a registered role renderer comes
    /// first, then the built-in user and assistant rows; other entries show nothing.
    fn row(
        &self,
        message: Message<C>,
        results: &Rc<Results>,
    ) -> Result<Option<PreparedContent>, ToolUiError> {
        if is_artifact_message(&message) {
            return Ok(None);
        }
        if let Some(custom) = render_message(&message)? {
            return Ok(Some(custom));
        }
        Ok(match &message {
            AgentMessage::Assistant(assistant) => {
                Some(Box::new(AssistantMessage::<serde_json::Value> {
                    message: Some(Arc::clone(assistant)),
                    tools: Some(Rc::clone(&self.tools)),
                    pending_tool_calls: self.pending_tool_calls.clone(),
                    hide_tool_calls: false,
                    tool_results_by_id: Some(ToolResults::Live(Rc::clone(results))),
                    is_streaming: false,
                    hide_pending_tool_calls: self.is_streaming,
                    on_cost_click: self.on_cost_click.clone(),
                }))
            }
            AgentMessage::User(_) => Some(Box::new(UserMessage {
                message: Some(message),
            })),
            AgentMessage::Custom(_) if is_user_message_with_attachments(&message) => {
                Some(Box::new(UserMessage {
                    message: Some(message),
                }))
            }
            _ => None,
        })
    }
}
/// Every result envelope by its exact call ID; a later result for the same call replaces an
/// earlier one.
#[cfg(any(target_arch = "wasm32", test))]
fn index<'a, C: CustomAgentMessages + 'a>(
    messages: impl Iterator<Item = &'a Message<C>>,
) -> Results {
    messages
        .filter_map(|message| match message {
            AgentMessage::ToolResult(result) => {
                let id = result
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .tool_call_id
                    .clone();
                Some((id, Arc::clone(result)))
            }
            _ => None,
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;

//! User text with caller-owned attachment handles.
#[cfg(any(target_arch = "wasm32", test))]
use crate::{SharedAttachment, tools::types::ToolUiError};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages};
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::{UserBlock, UserContent};

/// Native user row retaining the application's original message owner.
pub struct UserMessage<C: CustomAgentMessages = NoCustomMessages> {
    /// Ordinary or attachment-bearing application message, unset by default.
    pub message: Option<AgentMessage<BrowserMessages<C>>>,
}
impl<C: CustomAgentMessages> Default for UserMessage<C> {
    fn default() -> Self {
        Self { message: None }
    }
}
impl<C: CustomAgentMessages + 'static> UserMessage<C> {
    /// Construct a row with its required message unset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Copy only the selected source and original attachment handles outside the transient borrow.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(in crate::components::messages) fn operand(&self) -> Result<Operand, ToolUiError> {
        match self.message.as_ref() {
            Some(AgentMessage::User(message)) => {
                let message = message.try_read().map_err(|_| borrow_error())?;
                Ok(Operand::new(&message.content, &[]))
            }
            Some(AgentMessage::Custom(message)) => {
                let message = message.try_read().map_err(|_| borrow_error())?;
                if let BrowserMessages::UserWithAttachments(message) = &*message {
                    return Ok(Operand::new(
                        &message.content,
                        message.attachments.as_deref().unwrap_or_default(),
                    ));
                }
                Err(ToolUiError::Presentation("Expected user message".into()))
            }
            _ => Err(ToolUiError::Presentation("Missing user message".into())),
        }
    }
}
/// Operation-local selected source, never a copied attachment record.
#[cfg(any(target_arch = "wasm32", test))]
pub(in crate::components::messages) struct Operand {
    /// First text, including authored whitespace.
    pub(in crate::components::messages) text: String,
    /// Original shared attachment handles, including duplicates.
    pub(in crate::components::messages) attachments: Vec<SharedAttachment>,
}
#[cfg(any(target_arch = "wasm32", test))]
impl Operand {
    /// Leave the message borrow with only the operands consumed by the row.
    fn new(content: &UserContent, attachments: &[SharedAttachment]) -> Self {
        Self {
            text: user_source(content).into(),
            attachments: attachments.to_vec(),
        }
    }
}
/// Report a reached message borrow conflict without replacing the message owner.
#[cfg(any(target_arch = "wasm32", test))]
fn borrow_error() -> ToolUiError {
    ToolUiError::Presentation("Message borrow conflict".into())
}
/// Select the first text block without trimming or consulting later blocks.
#[cfg(any(target_arch = "wasm32", test))]
fn user_source(content: &UserContent) -> &str {
    match content {
        UserContent::Text(text) => text.as_str(),
        UserContent::Blocks(blocks) => blocks
            .iter()
            .find_map(|block| match block {
                UserBlock::Text(text) => Some(text.text.as_str()),
                UserBlock::Image(_) => None,
            })
            .unwrap_or_default(),
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;

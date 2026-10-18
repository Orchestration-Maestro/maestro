//! Custom messages and invocation-owned effective prompt chaining.
use super::{ExtensionRunner, dispatch::HandlerCursor};
use crate::{
    BuildSystemPromptOptions, EventError, ExtensionExecution,
    types::events::{BeforeAgentStartEvent, BeforeAgentStartEventResult},
};
use maestro_models::ImageContent;
use maestro_session::{Presence, messages::CustomMessageInput};
use std::{cell::RefCell, rc::Rc};

/// Accumulated messages and the last explicitly replaced system prompt.
pub struct BeforeAgentStartCombinedResult {
    /// Custom messages contributed by callbacks.
    pub messages: Option<Vec<CustomMessageInput>>,
    /// Optional replacement prompt, including empty text.
    pub system_prompt: Option<String>,
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Accumulates custom messages and chains a prompt reader private to this invocation.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_before_agent_start(
        &self,
        prompt: String,
        images: &mut Presence<Vec<ImageContent>>,
        system_prompt: String,
        system_prompt_options: BuildSystemPromptOptions,
    ) -> Result<Option<BeforeAgentStartCombinedResult>, EventError> {
        let current = Rc::new(RefCell::new(system_prompt));
        let mut context = self.create_context();
        context.invocation_system_prompt = Some(current.clone());
        let mut cursor = HandlerCursor::default();
        let mut messages = Vec::new();
        let mut modified = false;
        let mut event = BeforeAgentStartEvent {
            prompt: prompt.clone(),
            images: Presence::Missing,
            system_prompt: String::new(),
            system_prompt_options,
        };
        while let Some(invocation) =
            cursor.next::<_, BeforeAgentStartEventResult>(self, "before_agent_start")
        {
            event.prompt.clone_from(&prompt);
            event.system_prompt.clone_from(&current.borrow());
            event.images = std::mem::take(images);
            let result = invocation.call(self, &mut event, context.clone()).await;
            *images = std::mem::take(&mut event.images);
            let Some(result) = result? else {
                continue;
            };
            if let Presence::Present(message) = result.message {
                messages.push(message);
            }
            if let Presence::Present(prompt) = result.system_prompt {
                *current.borrow_mut() = prompt;
                modified = true;
            }
        }
        Ok(
            (!messages.is_empty() || modified).then(|| BeforeAgentStartCombinedResult {
                messages: (!messages.is_empty()).then_some(messages),
                system_prompt: modified.then(|| current.borrow().clone()),
            }),
        )
    }
}

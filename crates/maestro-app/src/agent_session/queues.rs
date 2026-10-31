//! Steering and follow-up queues with their text mirrors, custom messages and user
//! messages.
use super::prompting::{custom_message, split_command, user_message};
use super::state::thrown;
use super::{AgentSession, AgentSessionEvent, PromptOptions, QueueTexts};
use crate::utils::lock::{read, write};
use maestro_agent::{AgentEvent, AgentMessage, PromptInput};
use maestro_extensions::{ExtensionExecution, InputSource, NativeAgentMessage};
use maestro_models::{
    DiagnosticErrorInfo, ImageContent, SharedAssistantMessage, ToolResultMessage, UserBlock,
    UserContent,
};
use maestro_session::messages::{
    CustomAgentMessages, CustomMessageInput, MessageDelivery, SendMessageOptions,
    SendUserMessageOptions,
};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> AgentSession<E> {
    /// Queues a steering message, delivered after the running turn's tool calls and before
    /// the next model call. Skill commands and templates expand.
    ///
    /// # Errors
    /// Refuses the text of a registered command; returns an error listener's failure.
    pub fn steer(&self, text: &str, images: &[ImageContent]) -> Result<(), DiagnosticErrorInfo> {
        self.refuse_extension_command(text)?;
        let text = self.expand_text(text)?;
        self.queue_steer(text, images);
        Ok(())
    }

    /// Queues a follow-up message, delivered once the agent has no tool calls or steering
    /// messages left. Skill commands and templates expand.
    ///
    /// # Errors
    /// Refuses the text of a registered command; returns an error listener's failure.
    pub fn follow_up(
        &self,
        text: &str,
        images: &[ImageContent],
    ) -> Result<(), DiagnosticErrorInfo> {
        self.refuse_extension_command(text)?;
        let text = self.expand_text(text)?;
        self.queue_follow_up(text, images);
        Ok(())
    }

    /// Sends a custom message: kept for the next prompt when `deliver_as` asks for it,
    /// otherwise, while a run is active, queued (steering unless follow-up is asked for) or,
    /// when `trigger_turn` is false, kept until the current turn ends; otherwise sent as a new
    /// run when `trigger_turn` is set, otherwise appended to the history and the session with
    /// its own start and end announced.
    ///
    /// # Errors
    /// Returns the failure of a triggered run or the session history's failure.
    pub async fn send_custom_message(
        self: &Rc<Self>,
        message: CustomMessageInput,
        options: SendMessageOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let message = custom_message(message);
        if matches!(options.deliver_as, Some(MessageDelivery::NextTurn)) {
            self.state
                .borrow_mut()
                .pending_next_turn_messages
                .push(message);
        } else if self.is_streaming() && options.trigger_turn == Some(false) {
            self.state
                .borrow_mut()
                .pending_custom_messages
                .push(message);
        } else if self.is_streaming() {
            if matches!(options.deliver_as, Some(MessageDelivery::FollowUp)) {
                self.agent.follow_up(message);
            } else {
                self.agent.steer(message);
            }
        } else if options.trigger_turn == Some(true) {
            let session = Rc::clone(self);
            self.start_operation(
                async move { session.run_agent(PromptInput::Message(message)).await },
            )
            .await
            .map_err(thrown)??;
        } else {
            self.append_custom_message(&message, None)?;
        }
        Ok(())
    }

    /// Appends a custom message to the history, at `index` or at the end, and to the session,
    /// then announces its start and end.
    fn append_custom_message(
        &self,
        message: &NativeAgentMessage,
        index: Option<usize>,
    ) -> Result<(), DiagnosticErrorInfo> {
        {
            let history = self.messages();
            let mut history = write(&history);
            let at = index.map_or(history.len(), |index| index.min(history.len()));
            history.insert(at, message.clone());
        }
        self.store_custom_message(message)?;
        for event in [
            AgentEvent::MessageStart {
                message: message.clone(),
            },
            AgentEvent::MessageEnd {
                message: message.clone(),
            },
        ] {
            self.emit_agent_event(Arc::new(RwLock::new(event)));
        }
        Ok(())
    }

    /// Records the custom messages kept while a run was active, in the order they arrived:
    /// right after the tool results of the turn `turn` names, or at the end of the history.
    pub(super) fn flush_pending_custom_messages(
        &self,
        turn: Option<(&SharedAssistantMessage, &[ToolResultMessage])>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_custom_messages);
        let first = turn.and_then(|(response, results)| {
            let history = self.messages();
            turn_end_index(&read(&history), response, results)
        });
        pending
            .iter()
            .enumerate()
            .try_for_each(|(offset, message)| {
                self.append_custom_message(message, first.map(|index| index + offset))
            })
    }

    /// Stores a custom message as a session entry; any other message is left alone.
    pub(super) fn store_custom_message(
        &self,
        message: &NativeAgentMessage,
    ) -> Result<(), DiagnosticErrorInfo> {
        let AgentMessage::Custom(custom) = message else {
            return Ok(());
        };
        let CustomAgentMessages::Custom(custom) = &*read(custom) else {
            return Ok(());
        };
        self.session_manager
            .borrow_mut()
            .append_custom_message_entry(
                &custom.custom_type,
                custom.content.clone(),
                custom.display,
                custom.details.clone(),
            )
            .map(drop)
            .map_err(thrown)
    }

    /// Sends a user message, which always starts a turn: block content becomes its text
    /// blocks joined with newlines followed by its images. Commands, skills and templates do
    /// not expand, and the input handlers see the source as an extension.
    ///
    /// # Errors
    /// Returns the prompt's failure.
    pub async fn send_user_message(
        self: &Rc<Self>,
        content: UserContent,
        options: SendUserMessageOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let (text, images) = match content {
            UserContent::Text(text) => (text, None),
            UserContent::Blocks(blocks) => {
                let mut texts = Vec::new();
                let mut images = Vec::new();
                for block in blocks {
                    match block {
                        UserBlock::Text(text) => texts.push(text.text),
                        UserBlock::Image(image) => images.push(image),
                    }
                }
                (texts.join("\n"), (!images.is_empty()).then_some(images))
            }
        };
        let options = PromptOptions {
            expand_prompt_templates: Some(false),
            images,
            streaming_behavior: options.deliver_as,
            source: Some(InputSource::Extension),
            preflight_result: None,
        };
        self.prompt(&text, options).await
    }

    /// Empties both queues and their mirrors, publishes the change and returns the texts
    /// that were pending.
    pub fn clear_queue(&self) -> QueueTexts {
        let texts = {
            let mut state = self.state.borrow_mut();
            QueueTexts {
                steering: std::mem::take(&mut state.steering_messages),
                follow_up: std::mem::take(&mut state.follow_up_messages),
            }
        };
        self.agent.clear_all_queues();
        self.emit_queue_update();
        texts
    }

    /// The number of pending steering and follow-up texts.
    #[must_use]
    pub fn pending_message_count(&self) -> usize {
        let state = self.state.borrow();
        state.steering_messages.len() + state.follow_up_messages.len()
    }

    /// The pending steering texts.
    #[must_use]
    pub fn get_steering_messages(&self) -> Vec<String> {
        self.state.borrow().steering_messages.clone()
    }

    /// The pending follow-up texts.
    #[must_use]
    pub fn get_follow_up_messages(&self) -> Vec<String> {
        self.state.borrow().follow_up_messages.clone()
    }

    /// Mirrors an expanded steering text, publishes it and queues it on the agent.
    pub(super) fn queue_steer(&self, text: String, images: &[ImageContent]) {
        let message = self.mirror(text, images, |state| &mut state.steering_messages);
        self.agent.steer(message);
    }

    /// Mirrors an expanded follow-up text, publishes it and queues it on the agent.
    pub(super) fn queue_follow_up(&self, text: String, images: &[ImageContent]) {
        let message = self.mirror(text, images, |state| &mut state.follow_up_messages);
        self.agent.follow_up(message);
    }

    /// Adds `text` to the mirror `select` names, publishes both mirrors and returns the user
    /// message to queue.
    fn mirror(
        &self,
        text: String,
        images: &[ImageContent],
        select: impl FnOnce(&mut super::state::AgentSessionState<E>) -> &mut Vec<String>,
    ) -> NativeAgentMessage {
        select(&mut self.state.borrow_mut()).push(text.clone());
        self.emit_queue_update();
        user_message(text, images)
    }

    /// Publishes both mirrors.
    pub(super) fn emit_queue_update(&self) {
        let event = {
            let state = self.state.borrow();
            AgentSessionEvent::QueueUpdate {
                steering: state.steering_messages.clone(),
                follow_up: state.follow_up_messages.clone(),
            }
        };
        self.emit(&event);
    }

    /// Refuses to queue the text of a registered command.
    fn refuse_extension_command(&self, text: &str) -> Result<(), DiagnosticErrorInfo> {
        if !text.starts_with('/') {
            return Ok(());
        }
        let (name, _) = split_command(text);
        if self.extension_runner().get_command(name).is_none() {
            return Ok(());
        }
        Err(DiagnosticErrorInfo::error(format!(
            "Extension command \"/{name}\" cannot be queued. Use prompt() or execute the command \
             when not streaming."
        )))
    }
}

/// The history index right after the tool results of the turn that `response` began, or
/// absent when the turn's response is not in the history.
fn turn_end_index(
    history: &[NativeAgentMessage],
    response: &SharedAssistantMessage,
    results: &[ToolResultMessage],
) -> Option<usize> {
    let start = history.iter().position(
        |message| matches!(message, AgentMessage::Assistant(reply) if Arc::ptr_eq(reply, response)),
    )?;
    let settled = history[start + 1..]
        .iter()
        .take_while(|message| {
            matches!(message, AgentMessage::ToolResult(result)
                if results.iter().any(|known| known.tool_call_id == read(result).tool_call_id))
        })
        .count();
    Some(start + 1 + settled)
}

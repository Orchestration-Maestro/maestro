//! Steering and follow-up queues with their text mirrors, custom messages and user
//! messages.
use super::prompting::{custom_message, split_command, user_message};
use super::state::thrown;
use super::{AgentSession, AgentSessionEvent, PromptOptions, QueueTexts};
use crate::utils::lock::write;
use maestro_agent::{AgentEvent, PromptInput};
use maestro_extensions::{ExtensionExecution, InputSource, NativeAgentMessage};
use maestro_models::{DiagnosticErrorInfo, ImageContent, UserBlock, UserContent};
use maestro_session::messages::{
    CustomMessageInput, MessageDelivery, SendMessageOptions, SendUserMessageOptions,
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
    /// otherwise queued while a response streams (steering unless follow-up is asked for),
    /// otherwise sent as a new turn when `trigger_turn` is set, otherwise appended to the
    /// history and the session with its own start and end announced.
    ///
    /// # Errors
    /// Returns the failure of a triggered run or the session history's failure.
    pub async fn send_custom_message(
        self: &Rc<Self>,
        message: CustomMessageInput,
        options: SendMessageOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        if matches!(options.deliver_as, Some(MessageDelivery::NextTurn)) {
            let message = custom_message(message);
            self.state
                .borrow_mut()
                .pending_next_turn_messages
                .push(message);
        } else if self.is_streaming() {
            let message = custom_message(message);
            if matches!(options.deliver_as, Some(MessageDelivery::FollowUp)) {
                self.agent.follow_up(message);
            } else {
                self.agent.steer(message);
            }
        } else if options.trigger_turn == Some(true) {
            let message = custom_message(message);
            let session = Rc::clone(self);
            self.start_operation(
                async move { session.run_agent(PromptInput::Message(message)).await },
            )
            .await
            .map_err(thrown)??;
        } else {
            self.append_custom_message(message)?;
        }
        Ok(())
    }

    /// Appends a custom message to the history and the session, then announces its start
    /// and end.
    fn append_custom_message(&self, input: CustomMessageInput) -> Result<(), DiagnosticErrorInfo> {
        let message = custom_message(input.clone());
        write(&self.messages()).push(message.clone());
        self.session_manager
            .borrow_mut()
            .append_custom_message_entry(
                &input.custom_type,
                input.content,
                input.display,
                input.details,
            )
            .map_err(thrown)?;
        for event in [
            AgentEvent::MessageStart {
                message: message.clone(),
            },
            AgentEvent::MessageEnd { message },
        ] {
            self.emit_agent_event(Arc::new(RwLock::new(event)));
        }
        Ok(())
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

//! Sending a prompt: registered commands, input handlers, skill and template expansion,
//! streaming admission, the credential check and delivery to the agent.
use super::state::{last_assistant_message, thrown};
use super::{AgentSession, PromptOptions};
use crate::configuration::config::get_docs_path;
use crate::utils::lock::{read, write};
use maestro_agent::{AgentMessage, PromptInput};
use maestro_credentials::format_no_api_key_found_message;
use maestro_extensions::{
    ExtensionError, ExtensionExecution, InputEventResult, InputSource, NativeAgentMessage,
};
use maestro_models::{
    DiagnosticErrorInfo, ImageContent, StopReason, TextContent, UserBlock, UserContent,
    UserMessage, timestamp_now,
};
use maestro_resources::{expand_prompt_template, skills::read_text, strip_frontmatter};
use maestro_session::Presence;
use maestro_session::messages::{
    CustomAgentMessages, CustomMessage, CustomMessageInput, UserMessageDelivery,
};
use std::rc::Rc;
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> AgentSession<E> {
    /// Sends a prompt. A registered command runs at once, even while a response streams;
    /// otherwise the input handlers, then skill and template expansion, then streaming
    /// admission, the deferred shell results, the credential check and the pre-prompt
    /// compaction run before the user message, the next-turn messages and the extension
    /// messages are sent. Returns after the run settles: the agent, any retry, compaction
    /// and queued continuation.
    ///
    /// `options.preflight_result` hears once whether the prompt was accepted, before the
    /// agent is prompted. The prompt starts when this is called and belongs to the session:
    /// the returned future only observes its result, and dropping it leaves it running.
    ///
    /// # Errors
    /// Returns the refusal of a prompt that arrives while a response streams without a queue
    /// choice, the missing-credentials message, and failures of the extensions, the
    /// compaction check, the session history and the agent.
    pub fn prompt(
        self: &Rc<Self>,
        text: &str,
        options: PromptOptions,
    ) -> impl std::future::Future<Output = Result<(), DiagnosticErrorInfo>> + use<E> {
        let session = Rc::clone(self);
        let text = text.to_owned();
        let result = self.start_operation(async move { session.run_prompt(&text, options).await });
        async move { result.await.map_err(thrown)? }
    }

    /// The prompt the session runs, from its preflight to the settled agent and retry.
    async fn run_prompt(
        self: &Rc<Self>,
        text: &str,
        mut options: PromptOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let preflight_result = options.preflight_result.take();
        let notify = |accepted| {
            if let Some(observer) = &preflight_result {
                observer(accepted);
            }
        };
        let messages = match self.prepare_prompt(text, options).await {
            Ok(messages) => messages,
            Err(error) => {
                notify(false);
                return Err(error);
            }
        };
        notify(true);
        let Some(messages) = messages else {
            return Ok(());
        };
        self.run_agent(PromptInput::Messages(messages)).await
    }

    /// The one run: marks it active, prompts the agent, then retries, compacts and delivers
    /// queued messages until nothing is left, and settles last. Every start of the agent
    /// goes through it, so the run flag and the idle waiters cover all of its work.
    pub(super) async fn run_agent(
        self: &Rc<Self>,
        input: PromptInput<CustomAgentMessages>,
    ) -> Result<(), DiagnosticErrorInfo> {
        if !self.begin_run() {
            // The agent refuses a second start while the first run is active.
            return self.agent.prompt(input).await.map(drop);
        }
        let outcome = self.drive_run(input).await;
        outcome.and(self.settle_run())
    }

    /// Marks a run active; false when one already is.
    fn begin_run(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if state.run_gate.is_some() {
            return false;
        }
        let (open, closed) = tokio::sync::oneshot::channel();
        state.run_gate = Some(futures_util::future::FutureExt::shared(closed));
        state.run_open = Some(open);
        true
    }

    /// Prompts the agent, then continues it for as long as the finished response asks.
    async fn drive_run(
        self: &Rc<Self>,
        input: PromptInput<CustomAgentMessages>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.agent.prompt(input).await?;
        while self.finish_response_run().await? {
            self.agent.r#continue().await?;
        }
        Ok(())
    }

    /// Ends the run: appends the shell results kept while it ran, marks it inactive and
    /// releases every idle waiter.
    fn settle_run(&self) -> Result<(), DiagnosticErrorInfo> {
        let flushed = self.flush_pending_bash_messages();
        let (gate, open) = {
            let mut state = self.state.borrow_mut();
            state.system_prompt_override = None;
            (state.run_gate.take(), state.run_open.take())
        };
        drop(gate);
        drop(open);
        flushed
    }

    /// Once the agent's events are processed, retries a recoverable failure or checks
    /// compaction for the newest response; reports whether the agent continues. Queued
    /// messages continue the run unless the response was aborted, which keeps them queued.
    async fn finish_response_run(self: &Rc<Self>) -> Result<bool, DiagnosticErrorInfo> {
        if let Some(queue) = self.agent_event_queue() {
            drop(queue.await);
        }
        let last = self.state.borrow_mut().last_assistant_message.take();
        let Some(last) = last else {
            return Ok(false);
        };
        let message = read(&last).clone();
        if self.is_retryable_error(&message) && self.handle_retryable_error(&message).await {
            return Ok(true);
        }
        if self.check_compaction(&message, true).await? {
            return Ok(true);
        }
        Ok(message.stop_reason != StopReason::Aborted && self.agent.has_queued_messages())
    }

    /// Runs every step before the agent is prompted; absent when the prompt ends without
    /// sending anything.
    async fn prepare_prompt(
        self: &Rc<Self>,
        text: &str,
        options: PromptOptions,
    ) -> Result<Option<Vec<NativeAgentMessage>>, DiagnosticErrorInfo> {
        let expand = options.expand_prompt_templates.unwrap_or(true);
        if expand && text.starts_with('/') && self.try_execute_extension_command(text).await? {
            return Ok(None);
        }
        let source = options.source.unwrap_or(InputSource::Interactive);
        let Some((mut text, images)) = self.emit_input(text, options.images, source).await? else {
            return Ok(None);
        };
        if expand {
            text = self.expand_text(&text)?;
        }
        if self.is_streaming() {
            let Some(behavior) = options.streaming_behavior else {
                return Err(DiagnosticErrorInfo::error(
                    "Agent is already processing. Specify streamingBehavior ('steer' or \
                     'followUp') to queue the message.",
                ));
            };
            let images = images.as_deref().unwrap_or_default();
            match behavior {
                UserMessageDelivery::FollowUp => self.queue_follow_up(text, images),
                UserMessageDelivery::Steer => self.queue_steer(text, images),
            }
            return Ok(None);
        }
        self.flush_pending_bash_messages()?;
        self.require_credentials()?;
        let last = last_assistant_message(&read(&self.messages()));
        if let Some(last) = last {
            let last = read(&last).clone();
            self.check_compaction(&last, false).await?;
        }
        self.assemble_messages(text, images).await.map(Some)
    }

    /// The input handlers' text and images; absent when a handler handled the input.
    async fn emit_input(
        &self,
        text: &str,
        images: Option<Vec<ImageContent>>,
        source: InputSource,
    ) -> Result<Option<(String, Option<Vec<ImageContent>>)>, DiagnosticErrorInfo> {
        if !self.extension_runner().has_handlers("input") {
            return Ok(Some((text.to_owned(), images)));
        }
        let mut current = images.map_or(Presence::Missing, Presence::Present);
        let result = self
            .extension_runner()
            .emit_input(text, &mut current, source)
            .await
            .map_err(thrown)?;
        let images = match current {
            Presence::Present(images) => Some(images),
            _ => None,
        };
        Ok(match result {
            InputEventResult::Handled => None,
            InputEventResult::Transform(transform) => Some((transform.text, images)),
            InputEventResult::Continue => Some((text.to_owned(), images)),
        })
    }

    /// Refuses a prompt for a model without configured credentials.
    fn require_credentials(&self) -> Result<(), DiagnosticErrorInfo> {
        let model = self.model();
        if self
            .model_registry
            .has_configured_auth(&model)
            .map_err(thrown)?
        {
            return Ok(());
        }
        let provider = read(&model).provider.clone();
        let docs = get_docs_path().map_err(thrown)?;
        Err(DiagnosticErrorInfo::error(format_no_api_key_found_message(
            &provider, &docs,
        )))
    }

    /// The user message, then the next-turn messages, then the extensions' messages; the
    /// extensions' system prompt, even an empty one, replaces the base prompt for the run,
    /// which is otherwise restored.
    async fn assemble_messages(
        &self,
        text: String,
        images: Option<Vec<ImageContent>>,
    ) -> Result<Vec<NativeAgentMessage>, DiagnosticErrorInfo> {
        let mut messages = vec![user_message(
            text.clone(),
            images.as_deref().unwrap_or_default(),
        )];
        let mut images = images.map_or(Presence::Missing, Presence::Present);
        messages.append(&mut self.state.borrow_mut().pending_next_turn_messages);
        let (base, base_options) = {
            let state = self.state.borrow();
            let options = state.base_system_prompt_options.clone();
            (state.base_system_prompt.clone(), options)
        };
        let result = self
            .extension_runner()
            .emit_before_agent_start(text, &mut images, base.clone(), base_options)
            .await
            .map_err(thrown)?;
        let (added, prompt) = result.map_or((None, None), |result| {
            (result.messages, result.system_prompt)
        });
        messages.extend(added.into_iter().flatten().map(custom_message));
        let effective = {
            let mut state = self.state.borrow_mut();
            state.system_prompt_override = prompt;
            state.effective_system_prompt()
        };
        write(self.agent.state()).system_prompt = effective;
        Ok(messages)
    }

    /// Runs the registered command `text` names; a failing handler is reported to the error
    /// listeners and still counts as run.
    async fn try_execute_extension_command(&self, text: &str) -> Result<bool, DiagnosticErrorInfo> {
        let (name, arguments) = split_command(text);
        let Some(command) = self.extension_runner().get_command(name) else {
            return Ok(false);
        };
        let context = self.extension_runner().create_command_context();
        if let Err(error) = (command.command.handler)(arguments.to_owned(), context).await {
            self.extension_runner()
                .emit_error(&ExtensionError {
                    extension_path: format!("command:{name}"),
                    event: "command".into(),
                    error: error.to_string(),
                    stack: None,
                })
                .map_err(thrown)?;
        }
        Ok(true)
    }

    /// Expands a `/skill:<name>` command into the skill's block followed by its arguments;
    /// other text, unknown skills and unreadable skill files keep the text.
    ///
    /// # Errors
    /// Returns an error listener's failure.
    fn expand_skill_command(&self, text: &str) -> Result<String, DiagnosticErrorInfo> {
        let Some(command) = text.strip_prefix("/skill:") else {
            return Ok(text.to_owned());
        };
        let (name, arguments) = command.split_once(' ').unwrap_or((command, ""));
        let arguments = arguments.trim_matches(maestro_text::is_whitespace_scalar);
        let skills = self.resource_loader.get_skills();
        let Some(skill) = skills.skills.iter().find(|skill| skill.name == name) else {
            return Ok(text.to_owned());
        };
        let body = read_text(std::path::Path::new(&skill.file_path))
            .map_err(|error| error.to_string())
            .and_then(|content| strip_frontmatter(&content).map_err(|error| error.to_string()));
        let body = match body {
            Ok(body) => body,
            Err(error) => {
                self.extension_runner()
                    .emit_error(&ExtensionError {
                        extension_path: skill.file_path.clone(),
                        event: "skill_expansion".into(),
                        error,
                        stack: None,
                    })
                    .map_err(thrown)?;
                return Ok(text.to_owned());
            }
        };
        let block = format!(
            "<skill name=\"{}\" location=\"{}\">\nReferences are relative to {}.\n\n{}\n</skill>",
            skill.name,
            skill.file_path,
            skill.base_dir,
            body.trim_matches(maestro_text::is_whitespace_scalar)
        );
        Ok(if arguments.is_empty() {
            block
        } else {
            format!("{block}\n\n{arguments}")
        })
    }

    /// Expands a skill command, then a prompt template.
    pub(super) fn expand_text(&self, text: &str) -> Result<String, DiagnosticErrorInfo> {
        let text = self.expand_skill_command(text)?;
        Ok(expand_prompt_template(
            &text,
            &self.prompt_templates().prompts,
        ))
    }
}

/// The command name and arguments of `/<name> <arguments>`, split at the first space.
pub(super) fn split_command(text: &str) -> (&str, &str) {
    let command = text.strip_prefix('/').unwrap_or(text);
    command.split_once(' ').unwrap_or((command, ""))
}

/// A user message of one text block followed by the images.
pub(super) fn user_message(text: String, images: &[ImageContent]) -> NativeAgentMessage {
    let mut blocks = vec![UserBlock::Text(TextContent::new(text))];
    blocks.extend(images.iter().cloned().map(UserBlock::Image));
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Blocks(blocks),
        timestamp: timestamp_now(),
    })))
}

/// A custom message stamped now.
pub(super) fn custom_message(message: CustomMessageInput) -> NativeAgentMessage {
    AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::Custom(
        CustomMessage {
            custom_type: message.custom_type,
            content: message.content,
            display: message.display,
            details: message.details,
            timestamp: timestamp_now(),
        },
    ))))
}

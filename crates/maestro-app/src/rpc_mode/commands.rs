//! Each command, run against the current session or runtime and answered in its declared
//! shape.
use super::RpcRecord;
use super::connection::{Connection, SessionConnection, write_to};
use crate::agent_session::{AgentSession, BashRunOptions, CycleDirection, PromptOptions};
use crate::rpc_types::{
    RpcAvailableModels, RpcCancelled, RpcCommandList, RpcData, RpcExportPath, RpcForkMessages,
    RpcForkResult, RpcLastAssistantText, RpcMessages, RpcResponse, RpcSessionCommand,
    RpcSessionState, RpcThinkingLevel, workspace, workspace_schema,
};
use crate::utils::lock::read;
use maestro_extensions::types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions};
use maestro_extensions::{ForkPosition, InputSource};
use maestro_models::ImageContent;
use maestro_session::Presence;
use maestro_session::messages::UserMessageDelivery;
use maestro_session::session_manager::ReadonlySessionManager;
use std::{cell::Cell, rc::Rc, sync::Arc};

/// How a handled record is answered.
pub(super) enum Reply {
    /// The command was handled; its response, unless it answers on its own.
    Handled(Option<RpcResponse>),
    /// The operation failed with this response.
    Failed(RpcResponse),
}

/// How a session command is answered: with data, with a refusal, or later by itself.
pub(super) enum Answer {
    /// A success carrying `data`.
    Data(RpcData),
    /// A failure the command reports.
    Refused(String),
    /// The command answers on its own.
    Later,
}

/// A command operation's failure, answered with its message.
type Failure = Box<dyn std::error::Error>;

impl SessionConnection {
    /// Runs a session command; a command's own refusal checks for a requested shutdown, a
    /// failing operation does not.
    pub(super) async fn run_session_command(
        self: &Rc<Self>,
        id: Option<String>,
        command: String,
        session_command: RpcSessionCommand,
    ) -> Reply {
        match self.answer(id.clone(), session_command).await {
            Ok(Answer::Data(data)) => {
                Reply::Handled(Some(RpcResponse::Success { id, command, data }))
            }
            Ok(Answer::Refused(error)) => Reply::Handled(Some(RpcResponse::Error {
                id,
                command: Some(command),
                error,
            })),
            Ok(Answer::Later) => Reply::Handled(None),
            Err(error) => Reply::Failed(RpcResponse::Error {
                id,
                command: Some(command),
                error: error.to_string(),
            }),
        }
    }

    /// The answer to `command`.
    async fn answer(
        self: &Rc<Self>,
        id: Option<String>,
        command: RpcSessionCommand,
    ) -> Result<Answer, Failure> {
        use RpcSessionCommand as Command;
        let session = self.session();
        let data = match command {
            Command::Prompt {
                message,
                images,
                streaming_behavior,
            } => return Ok(self.prompt(id, message, images, streaming_behavior)),
            Command::SetModel { provider, model_id } => {
                return self.set_model(&provider, &model_id).await;
            }
            Command::SetSessionName { name } => return set_session_name(&session, &name),
            Command::Clone => return self.clone_session().await,
            Command::Steer { message, images } => {
                session.steer(&message, images.as_deref().unwrap_or_default())?;
                RpcData::Empty
            }
            Command::FollowUp { message, images } => {
                session.follow_up(&message, images.as_deref().unwrap_or_default())?;
                RpcData::Empty
            }
            Command::Abort => {
                session.abort().await;
                RpcData::Empty
            }
            Command::NewSession { parent_session } => self.new_session(parent_session).await?,
            Command::GetState => RpcData::State(self.state()?),
            Command::CycleModel => (session.cycle_model(CycleDirection::Forward).await?)
                .map_or(RpcData::Null, RpcData::ModelCycle),
            Command::GetAvailableModels => RpcData::Models(RpcAvailableModels {
                models: session.model_registry().get_available()?,
            }),
            Command::SetThinkingLevel { level } => {
                session.set_thinking_level(level)?;
                RpcData::Empty
            }
            Command::CycleThinkingLevel => (session.cycle_thinking_level()?)
                .map_or(RpcData::Null, |level| {
                    RpcData::ThinkingLevel(RpcThinkingLevel { level })
                }),
            Command::Compact {
                custom_instructions,
            } => RpcData::Compact(session.compact(custom_instructions.as_deref()).await?),
            Command::Bash { command } => RpcData::Bash(
                (session.execute_bash(&command, None, BashRunOptions::default())).await?,
            ),
            Command::GetSessionStats => RpcData::Stats(session.get_session_stats()?),
            Command::ExportHtml { output_path } => self.export_html(output_path.as_deref())?,
            Command::SwitchSession { session_path } => self.switch_session(&session_path).await?,
            Command::Fork { entry_id } => self.fork(&entry_id).await?,
            other => read_or_apply(&session, &other)?,
        };
        Ok(Answer::Data(data))
    }

    /// Starts `message` as a prompt: answered once its preflight succeeds, or with its
    /// failure before that; a later failure is not reported.
    fn prompt(
        self: &Rc<Self>,
        id: Option<String>,
        message: String,
        images: Option<Vec<ImageContent>>,
        streaming_behavior: Option<UserMessageDelivery>,
    ) -> Answer {
        let accepted = Rc::new(Cell::new(false));
        let connection = Rc::clone(self);
        let (accepting, success_id) = (Rc::clone(&accepted), id.clone());
        let options = PromptOptions {
            images,
            streaming_behavior,
            source: Some(InputSource::Rpc),
            preflight_result: Some(Rc::new(move |succeeded| {
                if succeeded {
                    accepting.set(true);
                    drop(connection.write(RpcRecord::Response(&RpcResponse::Success {
                        id: success_id.clone(),
                        command: "prompt".into(),
                        data: RpcData::Empty,
                    })));
                }
            })),
            expand_prompt_templates: None,
        };
        let (connection, session) = (Rc::clone(self), self.session());
        drop(tokio::task::spawn_local(async move {
            if let Err(error) = session.prompt(&message, options).await
                && !accepted.get()
            {
                drop(connection.write(RpcRecord::Response(&RpcResponse::Error {
                    id,
                    command: Some("prompt".into()),
                    error: error.to_string(),
                })));
            }
        }));
        Answer::Later
    }

    /// Writes the session as HTML at `output_path`, or under a default name.
    fn export_html(&self, output_path: Option<&str>) -> Result<RpcData, Failure> {
        let path = self.session().export_to_html(
            output_path,
            &*self.options.export_operations,
            Rc::clone(&self.options.render_environment),
        )?;
        Ok(RpcData::Path(RpcExportPath { path }))
    }

    /// Resumes the session file at `session_path`.
    async fn switch_session(&self, session_path: &str) -> Result<RpcData, Failure> {
        let options = SwitchSessionOptions::default();
        let result = self
            .options
            .runtime
            .switch_session(session_path, None, options)
            .await?;
        Ok(RpcData::Cancelled(RpcCancelled {
            cancelled: result.cancelled,
        }))
    }

    /// Forks the session before or at `entry_id`.
    async fn fork(&self, entry_id: &str) -> Result<RpcData, Failure> {
        let result = self
            .options
            .runtime
            .fork(entry_id, ForkOptions::default())
            .await?;
        Ok(RpcData::Fork(RpcForkResult {
            text: result.selected_text,
            cancelled: result.cancelled,
        }))
    }

    /// Starts a new session, recording a nonempty `parent_session` as its parent.
    async fn new_session(&self, parent_session: Option<String>) -> Result<RpcData, Failure> {
        let options = NewSessionCommandOptions {
            parent_session: parent_session.filter(|parent| !parent.is_empty()),
            ..NewSessionCommandOptions::default()
        };
        let result = self.options.runtime.new_session(options).await?;
        Ok(RpcData::Cancelled(RpcCancelled {
            cancelled: result.cancelled,
        }))
    }

    /// The session state.
    fn state(&self) -> Result<RpcSessionState, Failure> {
        let session = self.session();
        Ok(RpcSessionState {
            model: Some(session.model()),
            thinking_level: session.thinking_level(),
            is_streaming: session.is_streaming(),
            is_compacting: session.is_compacting(),
            steering_mode: session.steering_mode(),
            follow_up_mode: session.follow_up_mode(),
            session_file: session.session_file(),
            session_id: session.session_id(),
            session_name: session.session_name()?,
            auto_compaction_enabled: session.auto_compaction_enabled(),
            message_count: read(&session.messages()).len(),
            pending_message_count: session.pending_message_count(),
        })
    }

    /// Selects the first available model with `provider` and `model_id`, answering with
    /// the selected model.
    async fn set_model(&self, provider: &str, model_id: &str) -> Result<Answer, Failure> {
        let session = self.session();
        let models = session.model_registry().get_available()?;
        let found = models.into_iter().find(|model| {
            let model = read(model);
            model.provider == provider && model.id == model_id
        });
        let Some(model) = found else {
            return Ok(Answer::Refused(format!(
                "Model not found: {provider}/{model_id}"
            )));
        };
        session.set_model(Arc::clone(&model)).await?;
        Ok(Answer::Data(RpcData::Model(model)))
    }

    /// Forks the session at its current entry.
    async fn clone_session(&self) -> Result<Answer, Failure> {
        let leaf = (self.session().session_manager().borrow())
            .get_leaf_id()
            .map(str::to_owned);
        let Some(leaf) = leaf else {
            return Ok(Answer::Refused(
                "Cannot clone session: no current entry selected".into(),
            ));
        };
        let options = ForkOptions {
            position: Some(ForkPosition::At),
            ..ForkOptions::default()
        };
        let result = self.options.runtime.fork(&leaf, options).await?;
        Ok(Answer::Data(RpcData::Cancelled(RpcCancelled {
            cancelled: result.cancelled,
        })))
    }
}

impl Connection {
    /// Runs a workspace command through the caller's operations, its events written on this
    /// connection's output, and writes its response, whose write attempt settles the
    /// operation's write completion.
    pub(super) async fn run_workspace_command(
        self: &Rc<Self>,
        id: Option<String>,
        command: String,
        workspace_command: workspace::RpcCommand,
    ) -> Reply {
        let Some(workspace) = &self.workspace else {
            return Reply::Handled(Some(RpcResponse::Error {
                id: None,
                error: format!("Unknown command: {command}"),
                command: Some(command),
            }));
        };
        let connection = Rc::downgrade(self);
        let events = Rc::new(move |event: workspace_schema::EventData| {
            write_to(&connection, RpcRecord::WorkspaceEvent(&event))
        });
        let (attempted, write_complete) = tokio::sync::oneshot::channel();
        let result = workspace
            .execute(workspace_command, events, write_complete)
            .await;
        let response = RpcResponse::Workspace(Box::new(workspace::RpcResponse {
            id,
            command,
            result,
        }));
        drop(self.send(RpcRecord::Response(&response), Some(attempted)));
        Reply::Handled(None)
    }
}

/// Names the session with `name` trimmed, refusing an empty name.
fn set_session_name(session: &AgentSession, name: &str) -> Result<Answer, Failure> {
    let name = name.trim_matches(maestro_text::is_whitespace_scalar);
    if name.is_empty() {
        return Ok(Answer::Refused("Session name cannot be empty".into()));
    }
    session.set_session_name(name)?;
    Ok(Answer::Data(RpcData::Empty))
}

/// Applies a setting or reads the session; commands answered elsewhere apply nothing.
fn read_or_apply(session: &AgentSession, command: &RpcSessionCommand) -> Result<RpcData, Failure> {
    use RpcSessionCommand as Command;
    match *command {
        Command::SetSteeringMode { mode } => session.set_steering_mode(mode)?,
        Command::SetFollowUpMode { mode } => session.set_follow_up_mode(mode)?,
        Command::SetAutoCompaction { enabled } => session.set_auto_compaction_enabled(enabled)?,
        Command::SetAutoRetry { enabled } => session.set_auto_retry_enabled(enabled)?,
        Command::AbortRetry => session.abort_retry(),
        Command::AbortBash => session.abort_bash(),
        Command::GetForkMessages => {
            return Ok(RpcData::ForkMessages(RpcForkMessages {
                messages: session.get_user_messages_for_forking(),
            }));
        }
        Command::GetLastAssistantText => {
            return Ok(RpcData::LastAssistantText(RpcLastAssistantText {
                text: session
                    .get_last_assistant_text()
                    .map_or(Presence::Missing, Presence::Present),
            }));
        }
        Command::GetMessages => {
            return Ok(RpcData::Messages(RpcMessages {
                messages: session.messages(),
            }));
        }
        Command::GetCommands => {
            return Ok(RpcData::Commands(RpcCommandList {
                commands: session.get_commands(),
            }));
        }
        _ => {}
    }
    Ok(RpcData::Empty)
}

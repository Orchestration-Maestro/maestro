//! The session commands, each sent as one request.
//!
//! Every method fails with [`RpcClientError::Response`] carrying the session's message
//! when the session answers with a failure, and with the request failures of
//! [`RpcClient::send`] otherwise.
use super::{ModelInfo, RpcClient, RpcClientError, requests::unexpected_data};
use crate::agent_session::{ForkMessage, ModelCycleResult, SessionStats};
use crate::rpc_types::{
    RpcCancelled, RpcData, RpcExportPath, RpcForkResult, RpcSessionCommand, RpcSessionState,
    RpcSlashCommand, RpcThinkingLevel,
};
use maestro_agent::QueueMode;
use maestro_extensions::NativeAgentMessage;
use maestro_models::{ImageContent, Model, ModelThinkingLevel};
use maestro_session::{CompactionResult, Presence};
use maestro_tools::BashResult;
use std::sync::{Arc, RwLock};

impl RpcClient {
    /// Sends `command` and takes the data of its answer apart with `pick`.
    ///
    /// # Errors
    /// Returns a request failure, or a decoding failure when `pick` rejects the data.
    async fn send_for<T>(
        &self,
        command: RpcSessionCommand,
        pick: fn(RpcData) -> Option<T>,
    ) -> Result<T, RpcClientError> {
        pick(self.send(command).await?).ok_or_else(unexpected_data)
    }

    /// Sends a prompt and returns once the session admits it; the run's events arrive
    /// through [`RpcClient::on_event`].
    ///
    /// # Errors
    /// Returns the session's refusal or a request failure.
    pub async fn prompt(
        &self,
        message: &str,
        images: Option<Vec<ImageContent>>,
    ) -> Result<(), RpcClientError> {
        let message = message.to_owned();
        let command = RpcSessionCommand::Prompt {
            message,
            images,
            streaming_behavior: None,
        };
        self.send(command).await.map(drop)
    }

    /// Queues a steering message for the running turn.
    ///
    /// # Errors
    /// Returns the session's refusal or a request failure.
    pub async fn steer(
        &self,
        message: &str,
        images: Option<Vec<ImageContent>>,
    ) -> Result<(), RpcClientError> {
        let message = message.to_owned();
        self.send(RpcSessionCommand::Steer { message, images })
            .await
            .map(drop)
    }

    /// Queues a message for after the run finishes.
    ///
    /// # Errors
    /// Returns the session's refusal or a request failure.
    pub async fn follow_up(
        &self,
        message: &str,
        images: Option<Vec<ImageContent>>,
    ) -> Result<(), RpcClientError> {
        let message = message.to_owned();
        self.send(RpcSessionCommand::FollowUp { message, images })
            .await
            .map(drop)
    }

    /// Stops the running turn.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn abort(&self) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::Abort).await.map(drop)
    }

    /// Starts a new session, recording `parent_session` as its parent when given.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn new_session(
        &self,
        parent_session: Option<&str>,
    ) -> Result<RpcCancelled, RpcClientError> {
        let parent_session = parent_session.map(str::to_owned);
        let command = RpcSessionCommand::NewSession { parent_session };
        self.send_for(command, |data| match data {
            RpcData::Cancelled(cancelled) => Some(cancelled),
            _ => None,
        })
        .await
    }

    /// Reads the session state.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_state(&self) -> Result<RpcSessionState, RpcClientError> {
        self.send_for(RpcSessionCommand::GetState, |data| match data {
            RpcData::State(state) => Some(state),
            _ => None,
        })
        .await
    }

    /// Selects the model `model_id` of `provider` and returns it.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_model(
        &self,
        provider: &str,
        model_id: &str,
    ) -> Result<Arc<RwLock<Model>>, RpcClientError> {
        let command = RpcSessionCommand::SetModel {
            provider: provider.to_owned(),
            model_id: model_id.to_owned(),
        };
        self.send_for(command, |data| match data {
            RpcData::Model(model) => Some(model),
            _ => None,
        })
        .await
    }

    /// Selects the next model; `None` when there is none to select.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn cycle_model(&self) -> Result<Option<ModelCycleResult>, RpcClientError> {
        self.send_for(RpcSessionCommand::CycleModel, |data| match data {
            RpcData::ModelCycle(cycle) => Some(Some(cycle)),
            RpcData::Null => Some(None),
            _ => None,
        })
        .await
    }

    /// Lists the available models.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_available_models(
        &self,
    ) -> Result<Vec<Arc<RwLock<ModelInfo>>>, RpcClientError> {
        self.send_for(RpcSessionCommand::GetAvailableModels, |data| match data {
            RpcData::Models(models) => Some(models.models),
            _ => None,
        })
        .await
    }

    /// Sets the thinking level.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_thinking_level(
        &self,
        level: ModelThinkingLevel,
    ) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::SetThinkingLevel { level })
            .await
            .map(drop)
    }

    /// Selects the next thinking level; `None` when the model does not think.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn cycle_thinking_level(&self) -> Result<Option<RpcThinkingLevel>, RpcClientError> {
        self.send_for(RpcSessionCommand::CycleThinkingLevel, |data| match data {
            RpcData::ThinkingLevel(level) => Some(Some(level)),
            RpcData::Null => Some(None),
            _ => None,
        })
        .await
    }

    /// Sets how queued steering messages are delivered.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_steering_mode(&self, mode: QueueMode) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::SetSteeringMode { mode })
            .await
            .map(drop)
    }

    /// Sets how queued follow-up messages are delivered.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_follow_up_mode(&self, mode: QueueMode) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::SetFollowUpMode { mode })
            .await
            .map(drop)
    }

    /// Compacts the session context, following `custom_instructions` when given.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn compact(
        &self,
        custom_instructions: Option<&str>,
    ) -> Result<CompactionResult, RpcClientError> {
        let custom_instructions = custom_instructions.map(str::to_owned);
        let command = RpcSessionCommand::Compact {
            custom_instructions,
        };
        self.send_for(command, |data| match data {
            RpcData::Compact(result) => Some(result),
            _ => None,
        })
        .await
    }

    /// Turns automatic compaction on or off.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_auto_compaction(&self, enabled: bool) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::SetAutoCompaction { enabled })
            .await
            .map(drop)
    }

    /// Turns automatic retry on or off.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_auto_retry(&self, enabled: bool) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::SetAutoRetry { enabled })
            .await
            .map(drop)
    }

    /// Stops a pending retry.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn abort_retry(&self) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::AbortRetry).await.map(drop)
    }

    /// Runs a shell command and returns its result.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn bash(&self, command: &str) -> Result<BashResult, RpcClientError> {
        let command = RpcSessionCommand::Bash {
            command: command.to_owned(),
        };
        self.send_for(command, |data| match data {
            RpcData::Bash(result) => Some(result),
            _ => None,
        })
        .await
    }

    /// Stops the running shell command.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn abort_bash(&self) -> Result<(), RpcClientError> {
        self.send(RpcSessionCommand::AbortBash).await.map(drop)
    }

    /// Reads the session statistics.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_session_stats(&self) -> Result<SessionStats, RpcClientError> {
        self.send_for(RpcSessionCommand::GetSessionStats, |data| match data {
            RpcData::Stats(stats) => Some(stats),
            _ => None,
        })
        .await
    }

    /// Writes the session as an HTML document, at `output_path` when given, and returns
    /// the written path.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn export_html(
        &self,
        output_path: Option<&str>,
    ) -> Result<RpcExportPath, RpcClientError> {
        let output_path = output_path.map(str::to_owned);
        let command = RpcSessionCommand::ExportHtml { output_path };
        self.send_for(command, |data| match data {
            RpcData::Path(path) => Some(path),
            _ => None,
        })
        .await
    }

    /// Resumes the session file at `session_path`.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn switch_session(&self, session_path: &str) -> Result<RpcCancelled, RpcClientError> {
        let session_path = session_path.to_owned();
        let command = RpcSessionCommand::SwitchSession { session_path };
        self.send_for(command, |data| match data {
            RpcData::Cancelled(cancelled) => Some(cancelled),
            _ => None,
        })
        .await
    }

    /// Forks the session before the user message `entry_id`.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn fork(&self, entry_id: &str) -> Result<RpcForkResult, RpcClientError> {
        let entry_id = entry_id.to_owned();
        self.send_for(RpcSessionCommand::Fork { entry_id }, |data| match data {
            RpcData::Fork(fork) => Some(fork),
            _ => None,
        })
        .await
    }

    /// Duplicates the active branch into a new session; this is the session command, not
    /// a copy of the client.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn clone(&self) -> Result<RpcCancelled, RpcClientError> {
        self.send_for(RpcSessionCommand::Clone, |data| match data {
            RpcData::Cancelled(cancelled) => Some(cancelled),
            _ => None,
        })
        .await
    }

    /// Lists the user messages the session can be forked from.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_fork_messages(&self) -> Result<Vec<ForkMessage>, RpcClientError> {
        self.send_for(RpcSessionCommand::GetForkMessages, |data| match data {
            RpcData::ForkMessages(messages) => Some(messages.messages),
            _ => None,
        })
        .await
    }

    /// Reads the text of the last assistant message: missing without one, and `null` or
    /// text as the session answers.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_last_assistant_text(&self) -> Result<Presence<String>, RpcClientError> {
        self.send_for(RpcSessionCommand::GetLastAssistantText, |data| match data {
            RpcData::LastAssistantText(text) => Some(text.text),
            _ => None,
        })
        .await
    }

    /// Names the session.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn set_session_name(&self, name: &str) -> Result<(), RpcClientError> {
        let name = name.to_owned();
        self.send(RpcSessionCommand::SetSessionName { name })
            .await
            .map(drop)
    }

    /// Reads the session messages.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_messages(
        &self,
    ) -> Result<Arc<RwLock<Vec<NativeAgentMessage>>>, RpcClientError> {
        self.send_for(RpcSessionCommand::GetMessages, |data| match data {
            RpcData::Messages(messages) => Some(messages.messages),
            _ => None,
        })
        .await
    }

    /// Lists the commands a prompt can invoke, with where each comes from.
    ///
    /// # Errors
    /// Returns the session's failure or a request failure.
    pub async fn get_commands(&self) -> Result<Vec<RpcSlashCommand>, RpcClientError> {
        self.send_for(RpcSessionCommand::GetCommands, |data| match data {
            RpcData::Commands(commands) => Some(commands.commands),
            _ => None,
        })
        .await
    }
}

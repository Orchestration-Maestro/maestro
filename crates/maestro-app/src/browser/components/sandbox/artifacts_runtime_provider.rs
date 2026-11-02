//! Artifact storage offered to sandboxed code, online through host messages and
//! offline through an injected snapshot.
use super::sandbox_runtime_provider::{
    RuntimeProviderValue, SandboxRespond, SandboxRuntimeProvider,
};
use crate::browser::components::messages::{ArtifactAction, ArtifactMessage, CustomAgentMessages};
use crate::browser::tools::artifacts::{
    ArtifactCommand, ArtifactPresentation, Artifacts, ArtifactsPanel, ArtifactsPanelState,
    ArtifactsParams, Options, SharedArtifactPresentation,
};
use crate::browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW,
};
use crate::browser::utils::attachment_utils::SharedAttachment;
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_agent::agent::Native;
use maestro_agent::{
    AgentExecution, AgentMessage, AgentState, CustomAgentMessages as CustomMessage,
};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, SharedHandle};
use std::sync::{Arc, RwLock};

/// The conversation an artifact runtime reads attachments from and records writes in.
pub trait AgentLike: Native {
    /// Attachments of every current user message that carries them, in conversation order.
    fn attachments(&self) -> Vec<SharedAttachment>;
    /// Append one artifact history entry to the current conversation.
    fn push_artifact(&self, message: ArtifactMessage);
}
impl<C, E> AgentLike for RwLock<AgentState<CustomAgentMessages<C>, E>>
where
    C: CustomMessage + 'static,
    E: AgentExecution,
    Self: Native,
{
    fn attachments(&self) -> Vec<SharedAttachment> {
        let messages = Arc::clone(read(self).messages());
        let messages = read(&messages);
        messages
            .iter()
            .filter_map(|message| match message {
                AgentMessage::Custom(custom) => match &*read(custom) {
                    CustomAgentMessages::UserWithAttachments(user) => user.attachments.clone(),
                    _ => None,
                },
                _ => None,
            })
            .flatten()
            .collect()
    }
    fn push_artifact(&self, message: ArtifactMessage) {
        let messages = Arc::clone(read(self).messages());
        write(&messages).push(AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::Artifact(message),
        ))));
    }
}
/// The string fields of one `artifact-operation` message; absent or non-string fields are empty.
pub struct ArtifactOperation {
    /// Requested action.
    pub action: String,
    /// Target filename.
    pub filename: String,
    /// New content for a create-or-update request.
    pub content: String,
}
/// One reply to an `artifact-operation` message.
pub enum ArtifactResponse {
    /// A write succeeded.
    Done,
    /// Filenames in map order.
    Listed(Vec<String>),
    /// One file's content.
    Read(String),
    /// The operation failed with this text.
    Failed(String),
}
/// Artifact storage access; writes are available only with read-write access.
pub struct ArtifactsRuntimeProvider<P = SharedArtifactPresentation> {
    /// Store access with the optional command and history owners.
    access: Access<P>,
}
/// Read-only store access, or the command owner with its optional conversation.
#[derive(Clone)]
enum Access<P> {
    /// Read the live store only.
    ReadOnly(ArtifactsPanelState),
    /// Run artifact commands and record successful writes.
    ReadWrite {
        /// Command owner.
        panel: ArtifactsPanel<P>,
        /// Conversation receiving history entries.
        agent: Option<SharedHandle<dyn AgentLike>>,
    },
}
impl<P> ArtifactsRuntimeProvider<P>
where
    P: Clone + std::ops::Deref,
    P::Target: ArtifactPresentation,
{
    /// Offer the panel's artifacts. Read-write access runs writes through the panel's
    /// commands and records each successful write in `agent`; read-only access refuses
    /// writes and never uses `agent`.
    #[must_use]
    pub fn new(
        artifacts_panel: &ArtifactsPanel<P>,
        agent: Option<SharedHandle<dyn AgentLike>>,
        read_write: bool,
    ) -> Self {
        let access = if read_write {
            Access::ReadWrite {
                panel: artifacts_panel.clone(),
                agent,
            }
        } else {
            Access::ReadOnly(artifacts_panel.state.clone())
        };
        Self { access }
    }
}
impl ArtifactsRuntimeProvider {
    /// Read-only access to the supplied live panel state.
    pub(crate) fn read_only(state: ArtifactsPanelState) -> Self {
        Self {
            access: Access::ReadOnly(state),
        }
    }
}
impl<P> Access<P>
where
    P: Clone + std::ops::Deref,
    P::Target: ArtifactPresentation,
{
    /// The live map handle.
    fn artifacts(&self) -> Artifacts {
        match self {
            Self::ReadOnly(state) => state.artifacts(),
            Self::ReadWrite { panel, .. } => panel.artifacts(),
        }
    }
    /// Answer one operation; reads use the live store, writes run commands.
    async fn operate(&self, operation: ArtifactOperation) -> ArtifactResponse {
        let ArtifactOperation {
            action,
            filename,
            content,
        } = operation;
        let artifacts = self.artifacts();
        match action.as_str() {
            "list" => ArtifactResponse::Listed(read(&artifacts).keys().cloned().collect()),
            "get" => {
                let record = read(&artifacts).get(&filename).cloned();
                record.map_or_else(
                    || ArtifactResponse::Failed(format!("Artifact not found: {filename}")),
                    |record| ArtifactResponse::Read(read(&record).content.clone()),
                )
            }
            "createOrUpdate" => {
                let exists = read(&artifacts).contains_key(&filename);
                let (command, action) = if exists {
                    (ArtifactCommand::Rewrite, ArtifactAction::Update)
                } else {
                    (ArtifactCommand::Create, ArtifactAction::Create)
                };
                self.change(command, action, filename, Some(content)).await
            }
            "delete" => {
                self.change(
                    ArtifactCommand::Delete,
                    ArtifactAction::Delete,
                    filename,
                    None,
                )
                .await
            }
            _ => ArtifactResponse::Failed(format!("Unknown artifact action: {action}")),
        }
    }
    /// Run a write command, recording it only after the command succeeded.
    async fn change(
        &self,
        command: ArtifactCommand,
        action: ArtifactAction,
        filename: String,
        content: Option<String>,
    ) -> ArtifactResponse {
        let Self::ReadWrite { panel, agent } = self else {
            return ArtifactResponse::Failed(
                if matches!(action, ArtifactAction::Delete) {
                    "Cannot delete artifacts in read-only mode"
                } else {
                    "Cannot create/update artifacts in read-only mode"
                }
                .into(),
            );
        };
        let params = ArtifactsParams {
            command,
            filename,
            content,
            old_str: None,
            new_str: None,
        };
        if let Err(error) = panel.execute(&params, Options::default()).await {
            return ArtifactResponse::Failed(error);
        }
        if let Some(agent) = agent {
            let ArtifactsParams {
                filename, content, ..
            } = params;
            agent.push_artifact(ArtifactMessage {
                title: matches!(action, ArtifactAction::Create).then(|| filename.clone()),
                action,
                filename,
                content,
                timestamp: panel
                    .presentation
                    .now()
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            });
        }
        ArtifactResponse::Done
    }
}
impl<V, P> SandboxRuntimeProvider<V> for ArtifactsRuntimeProvider<P>
where
    V: RuntimeProviderValue,
    P: Clone + std::ops::Deref + Native + 'static,
    P::Target: ArtifactPresentation,
{
    fn get_data(&self) -> Result<IndexMap<String, V>, DiagnosticErrorInfo> {
        let artifacts = self.access.artifacts();
        let snapshot = read(&artifacts)
            .iter()
            .map(|(filename, record)| (filename.clone(), read(record).content.clone()))
            .collect();
        Ok(IndexMap::from([(
            "artifacts".into(),
            V::artifact_snapshot(snapshot)?,
        )]))
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(include_str!("artifacts_runtime_provider/runtime.js").into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(match self.access {
            Access::ReadOnly(_) => ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO,
            Access::ReadWrite { .. } => ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW,
        }
        .into())
    }
    fn handle_message(
        &self,
        message: V,
        respond: SandboxRespond<V>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let access = self.access.clone();
        Box::pin(async move {
            if message.message_type()?.as_deref() != Some("artifact-operation") {
                return Ok(());
            }
            let operation = message.project_artifact_operation()?;
            let response = access.operate(operation).await;
            respond(V::artifact_response(response)?)
        })
    }
}

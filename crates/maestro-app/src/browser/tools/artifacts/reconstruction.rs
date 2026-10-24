//! Artifact state rebuilt from conversation history.
use super::{
    ArtifactCommand, ArtifactPresentation, ArtifactsPanel, ArtifactsParams, Options,
    created_content, definition, edited_content,
};
use crate::browser::components::messages::{ArtifactAction, ArtifactMessage, CustomAgentMessages};
use crate::utils::lock::{read, write};
use indexmap::IndexMap;
use maestro_agent::{AgentMessage, CustomAgentMessages as CustomMessage};
use maestro_models::{AssistantContent, Tool, ToolCall, validate_tool_arguments};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

impl<P> ArtifactsPanel<P>
where
    P: Clone + std::ops::Deref,
    P::Target: ArtifactPresentation,
{
    /// Replace every artifact with the files the conversation's artifact history leaves.
    ///
    /// History entries apply as recorded, and an entry whose content is empty is
    /// skipped; artifact tool results apply only when the live command would have
    /// accepted them. Views are removed and recreated without HTML waits or per-file
    /// notifications, and every HTML view reloads after each file. The first file is
    /// then selected and the host is notified once.
    pub async fn reconstruct_from_messages<C: CustomMessage>(
        &self,
        messages: &[AgentMessage<CustomAgentMessages<C>>],
    ) {
        let files = replay(messages);
        let previous = self.artifacts();
        let names: Vec<String> = {
            let mut previous = write(&previous);
            previous.drain(..).map(|(name, _)| name).collect()
        };
        for name in &names {
            self.presentation.remove_artifact_element(name);
        }
        let artifacts = Arc::new(RwLock::new(IndexMap::new()));
        {
            let mut state = write(&self.state.0);
            state.active = None;
            state.artifacts = Arc::clone(&artifacts);
        }
        for (filename, content) in files {
            self.insert(&artifacts, &filename, &content);
            let options = Options {
                silent: true,
                skip_wait: true,
            };
            self.created(&filename, &content, options).await;
        }
        let first = read(&self.artifacts()).keys().next().cloned();
        let unselected = self.active_filename().is_none_or(|name| name.is_empty());
        if let Some(first) = first.filter(|_| unselected) {
            self.show_artifact(&first);
        }
        self.presentation.on_artifacts_change();
        self.presentation.request_update();
    }
}
/// Fold artifact history entries and successful artifact tool commands in message order.
fn replay<C: CustomMessage>(
    messages: &[AgentMessage<CustomAgentMessages<C>>],
) -> IndexMap<String, String> {
    let declaration = definition();
    let calls = artifact_calls(messages, &declaration.name);
    let mut files = IndexMap::new();
    for message in messages {
        match message {
            AgentMessage::Custom(custom) => {
                if let CustomAgentMessages::Artifact(record) = &*read(custom) {
                    apply_record(&mut files, record);
                }
            }
            AgentMessage::ToolResult(result) => {
                let result = read(result);
                let call = calls.get(&result.tool_call_id);
                if result.tool_name == declaration.name
                    && !result.is_error
                    && let Some(params) = call.and_then(|call| prepared(&declaration, call))
                {
                    apply_command(&mut files, &params);
                }
            }
            AgentMessage::User(_) | AgentMessage::Assistant(_) => {}
        }
    }
    files
}
/// Every assistant artifact tool call by identifier; a later duplicate wins.
fn artifact_calls<C: CustomMessage>(
    messages: &[AgentMessage<CustomAgentMessages<C>>],
    tool_name: &str,
) -> HashMap<String, ToolCall> {
    let mut calls = HashMap::new();
    for message in messages {
        let AgentMessage::Assistant(assistant) = message else {
            continue;
        };
        for block in &read(assistant).content {
            if let AssistantContent::ToolCall(call) = block
                && call.name == tool_name
            {
                calls.insert(call.id.clone(), call.clone());
            }
        }
    }
    calls
}
/// The arguments the live tool received, prepared by the shared argument checker.
fn prepared(declaration: &Tool, call: &ToolCall) -> Option<ArtifactsParams> {
    let arguments = validate_tool_arguments(declaration, call).ok()?;
    ArtifactsParams::deserialize(arguments).ok()
}
/// Apply a recorded entry: create and update set nonempty content, delete removes.
fn apply_record(files: &mut IndexMap<String, String>, record: &ArtifactMessage) {
    match record.action {
        ArtifactAction::Create | ArtifactAction::Update => {
            if let Some(content) = record
                .content
                .as_ref()
                .filter(|content| !content.is_empty())
            {
                files.insert(record.filename.clone(), content.clone());
            }
        }
        ArtifactAction::Delete => {
            files.shift_remove(&record.filename);
        }
    }
}
/// Apply a tool command only when the live command accepts it against the replayed file.
fn apply_command(files: &mut IndexMap<String, String>, params: &ArtifactsParams) {
    match params.command {
        ArtifactCommand::Create => {
            let exists = files.contains_key(&params.filename);
            if let Ok(content) = created_content(params, exists) {
                files.insert(params.filename.clone(), content.into());
            }
        }
        ArtifactCommand::Update | ArtifactCommand::Rewrite => {
            if let Some(current) = files.get_mut(&params.filename)
                && let Ok(edited) = edited_content(params, current)
            {
                *current = edited;
            }
        }
        ArtifactCommand::Delete => {
            files.shift_remove(&params.filename);
        }
        ArtifactCommand::Get | ArtifactCommand::Logs => {}
    }
}

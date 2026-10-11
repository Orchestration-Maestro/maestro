//! Writable setup resources and one-shot setup callbacks.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{
    Exports, Imports, callbacks,
    exports::encode_value,
    session::{decode, reader},
};
use crate::{
    AgentMessage, ExtensionResult, NewSessionOptions, ReadonlySessionManager, SessionContext,
    SessionEntry, SessionManager, SessionWriterPort, SetupSession,
};
use std::rc::Rc;
/// Host writer behind the author facade.
struct Writer<I: Imports> {
    /// Adapter serving this capability.
    imports: I,
    /// Owned writable session resource.
    resource: I::Writer,
}
/// Wraps an owned writer with its host reader.
pub(super) fn writer<I: Imports>(imports: I, resource: I::Writer) -> SessionManager {
    SessionManager::new(Rc::new(Writer { imports, resource }))
}
impl<I: Imports> SessionWriterPort for Writer<I> {
    fn reader(&self) -> ReadonlySessionManager {
        reader(
            self.imports.clone(),
            self.imports.writer_reader(&self.resource),
        )
    }
    fn set_session_file(&self, path: &str) -> ExtensionResult<()> {
        self.imports.writer_set_session_file(&self.resource, path)
    }
    fn new_session(&self, options: Option<NewSessionOptions>) -> ExtensionResult<Option<String>> {
        self.imports.writer_new_session(
            &self.resource,
            options.as_ref().map(encode_value).transpose()?.as_deref(),
        )
    }
    fn is_persisted(&self) -> ExtensionResult<bool> {
        self.imports.writer_is_persisted(&self.resource)
    }
    fn get_children(&self, parent_id: &str) -> ExtensionResult<Vec<SessionEntry>> {
        decode(
            &self
                .imports
                .writer_get_children(&self.resource, parent_id)?,
        )
    }
    fn build_session_context(&self) -> ExtensionResult<SessionContext> {
        decode(&self.imports.writer_build_session_context(&self.resource)?)
    }
    fn append_message(&self, message: AgentMessage) -> ExtensionResult<String> {
        self.imports
            .writer_append_message(&self.resource, &encode_value(&message)?)
    }
    fn append_thinking_level_change(&self, level: &str) -> ExtensionResult<String> {
        self.imports
            .writer_append_thinking_level_change(&self.resource, level)
    }
    fn append_model_change(&self, provider: &str, model_id: &str) -> ExtensionResult<String> {
        self.imports
            .writer_append_model_change(&self.resource, provider, model_id)
    }
    fn append_compaction(
        &self,
        compaction: crate::CompactionResult,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String> {
        self.imports.writer_append_compaction(
            &self.resource,
            &encode_value(&compaction)?,
            from_hook,
        )
    }
    fn append_custom_entry(
        &self,
        custom_type: &str,
        data: Option<serde_json::Value>,
    ) -> ExtensionResult<String> {
        self.imports.writer_append_custom_entry(
            &self.resource,
            custom_type,
            data.as_ref().map(encode_value).transpose()?.as_deref(),
        )
    }
    fn append_session_info(&self, name: &str) -> ExtensionResult<String> {
        self.imports
            .writer_append_session_info(&self.resource, name)
    }
    fn append_custom_message_entry(
        &self,
        message: crate::CustomMessageInput,
    ) -> ExtensionResult<String> {
        self.imports
            .writer_append_custom_message_entry(&self.resource, &encode_value(&message)?)
    }
    fn append_label_change(&self, target_id: &str, label: Option<&str>) -> ExtensionResult<String> {
        self.imports
            .writer_append_label_change(&self.resource, target_id, label)
    }
    fn branch(&self, from_id: &str) -> ExtensionResult<()> {
        self.imports.writer_branch(&self.resource, from_id)
    }
    fn reset_leaf(&self) -> ExtensionResult<()> {
        self.imports.writer_reset_leaf(&self.resource)
    }
    fn branch_with_summary(
        &self,
        from_id: Option<&str>,
        summary: &str,
        details: Option<serde_json::Value>,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String> {
        self.imports.writer_branch_with_summary(
            &self.resource,
            from_id,
            summary,
            (
                details.as_ref().map(encode_value).transpose()?.as_deref(),
                from_hook,
            ),
        )
    }
    fn create_branched_session(&self, leaf_id: &str) -> ExtensionResult<Option<String>> {
        self.imports
            .writer_create_branched_session(&self.resource, leaf_id)
    }
}
impl<I: Imports> Exports<I> {
    /// Takes the one-shot setup closure before calling and awaiting author code.
    pub(crate) async fn invoke_setup(
        &self,
        handler: u32,
        resource: I::Writer,
    ) -> ExtensionResult<()> {
        callbacks::take::<SetupSession>(handler)?(writer(self.imports.clone(), resource)).await
    }
}

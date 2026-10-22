//! Retained runtime provider interface.
use super::super::sandboxed_iframe::SandboxValue;
use super::artifacts_runtime_provider::{ArtifactOperation, ArtifactResponse};
use maestro_agent::agent::Native;
use maestro_models::SharedHandle;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};

/// Native response callback preserving caller failures.
#[cfg(not(target_arch = "wasm32"))]
pub type SandboxRespond<V> =
    SharedHandle<dyn Fn(V) -> Result<(), DiagnosticErrorInfo> + Send + Sync>;
/// Browser-local response callback preserving caller failures.
#[cfg(target_arch = "wasm32")]
pub type SandboxRespond<V> = SharedHandle<dyn Fn(V) -> Result<(), DiagnosticErrorInfo>>;
/// Supplied data and self-contained function source for injection.
pub trait SandboxRuntimeProvider<V: SandboxValue = serde_json::Value>: Native {
    /// Obtain owned globals in insertion order.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_data(&self) -> Result<indexmap::IndexMap<String, V>, DiagnosticErrorInfo>;
    /// Obtain the complete function source.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo>;
    /// Obtain model-facing helper guidance.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo>;
    /// Handle an incoming message; by default no action is needed.
    fn handle_message(
        &self,
        _message: V,
        _respond: SandboxRespond<V>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(async { Ok(()) })
    }
    /// Notify execution start; by default no action is needed.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn on_execution_start(
        &self,
        _sandbox_id: &str,
        _signal: Option<Cancellation>,
    ) -> Result<(), DiagnosticErrorInfo> {
        Ok(())
    }
    /// Notify execution settlement; by default no action is needed.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn on_execution_end(&self, _sandbox_id: &str) -> Result<(), DiagnosticErrorInfo> {
        Ok(())
    }
}

/// Typed projections needed by the concrete runtime providers.
pub trait RuntimeProviderValue: SandboxValue + Clone {
    /// Read a string tag without coercing or inspecting unrelated fields.
    ///
    /// # Errors
    /// Returns a native property-access failure.
    fn message_type(&self) -> Result<Option<String>, DiagnosticErrorInfo>;
    /// Retain the console payload without formatting it.
    ///
    /// # Errors
    /// Returns a native property-access failure.
    fn project_console(
        &self,
    ) -> Result<super::console_runtime_provider::ConsoleLog<Self>, DiagnosticErrorInfo>;
    /// Retain returned-file fields without coercion.
    ///
    /// # Errors
    /// Returns a native property-access failure.
    fn project_download(
        &self,
    ) -> Result<super::file_download_runtime_provider::DownloadableFile<Self>, DiagnosticErrorInfo>;
    /// Select the execution error's ordinary message and stack fields.
    ///
    /// # Errors
    /// Returns a native property-access failure.
    fn project_completion_error(
        &self,
    ) -> Result<super::super::sandboxed_iframe::SandboxError, DiagnosticErrorInfo>;
    /// Create the successful collection acknowledgement.
    ///
    /// # Errors
    /// Returns a native object-construction failure.
    fn success_response() -> Result<Self, DiagnosticErrorInfo>;
    /// Snapshot the six injectable attachment fields in input order.
    ///
    /// # Errors
    /// Returns a native object-construction failure.
    fn attachment_array(
        attachments: &[crate::browser::utils::attachment_utils::Attachment],
    ) -> Result<Self, DiagnosticErrorInfo>;
    /// Read an artifact operation's action, filename and content strings.
    ///
    /// # Errors
    /// Returns a native property-access failure.
    fn project_artifact_operation(&self) -> Result<ArtifactOperation, DiagnosticErrorInfo>;
    /// Snapshot artifact contents as one object keyed by filename in map order.
    ///
    /// # Errors
    /// Returns a native object-construction failure.
    fn artifact_snapshot(
        artifacts: indexmap::IndexMap<String, String>,
    ) -> Result<Self, DiagnosticErrorInfo>;
    /// Create one artifact operation reply.
    ///
    /// # Errors
    /// Returns a native object-construction failure.
    fn artifact_response(response: ArtifactResponse) -> Result<Self, DiagnosticErrorInfo>;
}
impl RuntimeProviderValue for serde_json::Value {
    fn message_type(&self) -> Result<Option<String>, DiagnosticErrorInfo> {
        Ok(self.get("type").and_then(Self::as_str).map(str::to_owned))
    }
    fn project_console(
        &self,
    ) -> Result<super::console_runtime_provider::ConsoleLog<Self>, DiagnosticErrorInfo> {
        use super::console_runtime_provider::{ConsoleLog, ConsoleLogType};
        Ok(ConsoleLog {
            r#type: ConsoleLogType::from_method(self.get("method").and_then(Self::as_str)),
            text: self.get("text").cloned().unwrap_or_default(),
            args: self.get("args").cloned(),
        })
    }
    fn project_download(
        &self,
    ) -> Result<super::file_download_runtime_provider::DownloadableFile<Self>, DiagnosticErrorInfo>
    {
        Ok(super::super::sandboxed_iframe::SandboxFile {
            file_name: self.get("fileName").cloned().unwrap_or_default(),
            content: self.get("content").cloned().unwrap_or_default(),
            mime_type: self.get("mimeType").cloned().unwrap_or_default(),
        })
    }
    fn project_completion_error(
        &self,
    ) -> Result<super::super::sandboxed_iframe::SandboxError, DiagnosticErrorInfo> {
        let error = &self["error"];
        Ok(super::super::sandboxed_iframe::SandboxError {
            message: error
                .get("message")
                .and_then(Self::as_str)
                .unwrap_or_default()
                .into(),
            stack: error
                .get("stack")
                .and_then(Self::as_str)
                .unwrap_or_default()
                .into(),
        })
    }
    fn success_response() -> Result<Self, DiagnosticErrorInfo> {
        Ok(serde_json::json!({"success": true}))
    }
    fn attachment_array(
        attachments: &[crate::browser::utils::attachment_utils::Attachment],
    ) -> Result<Self, DiagnosticErrorInfo> {
        Ok(Self::Array(
            attachments
                .iter()
                .map(|attachment| {
                    let mut value = serde_json::json!({
                        "id": attachment.id, "fileName": attachment.file_name,
                        "mimeType": attachment.mime_type, "size": attachment.size,
                        "content": attachment.content,
                    });
                    if let Some(text) = &attachment.extracted_text {
                        value["extractedText"] = Self::String(text.clone());
                    }
                    value
                })
                .collect(),
        ))
    }
    fn project_artifact_operation(&self) -> Result<ArtifactOperation, DiagnosticErrorInfo> {
        let text = |name| {
            self.get(name)
                .and_then(Self::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(ArtifactOperation {
            action: text("action"),
            filename: text("filename"),
            content: text("content"),
        })
    }
    fn artifact_snapshot(
        artifacts: indexmap::IndexMap<String, String>,
    ) -> Result<Self, DiagnosticErrorInfo> {
        Ok(Self::Object(
            artifacts
                .into_iter()
                .map(|(filename, content)| (filename, Self::String(content)))
                .collect(),
        ))
    }
    fn artifact_response(response: ArtifactResponse) -> Result<Self, DiagnosticErrorInfo> {
        Ok(match response {
            ArtifactResponse::Done => return Self::success_response(),
            ArtifactResponse::Listed(names) => {
                serde_json::json!({"success": true, "result": names})
            }
            ArtifactResponse::Read(content) => {
                serde_json::json!({"success": true, "result": content})
            }
            ArtifactResponse::Failed(error) => {
                serde_json::json!({"success": false, "error": error})
            }
        })
    }
}

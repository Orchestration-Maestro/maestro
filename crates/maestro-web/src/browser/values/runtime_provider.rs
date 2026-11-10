//! Native field selection for application-owned sandbox runtime providers.
use super::{BrowserSandboxValue, diagnostic};
use indexmap::IndexMap;
use js_sys::{Array, Object, Reflect};
use maestro_app::{
    DiagnosticErrorInfo,
    browser::{
        components::{
            sandbox::{
                artifacts_runtime_provider::{ArtifactOperation, ArtifactResponse},
                console_runtime_provider::{ConsoleLog, ConsoleLogType},
                file_download_runtime_provider::DownloadableFile,
                providers::RuntimeProviderValue,
            },
            sandboxed_iframe::{SandboxError, SandboxFile},
        },
        utils::attachments::Attachment,
    },
};
use wasm_bindgen::JsValue;

/// Read one property without serializing the object or any sibling field.
fn field(value: &JsValue, name: &str) -> Result<JsValue, DiagnosticErrorInfo> {
    Reflect::get(value, &name.into()).map_err(|error| diagnostic(&error))
}
/// Set a freshly allocated object's property, retaining the supplied native value.
fn set(object: &Object, name: &str, value: &JsValue) -> Result<(), DiagnosticErrorInfo> {
    Reflect::set(object, &name.into(), value)
        .map(|_| ())
        .map_err(|error| diagnostic(&error))
}
impl RuntimeProviderValue for BrowserSandboxValue {
    fn message_type(&self) -> Result<Option<String>, DiagnosticErrorInfo> {
        Ok(field(&self.0, "type")?.as_string())
    }
    fn project_console(&self) -> Result<ConsoleLog<Self>, DiagnosticErrorInfo> {
        let method = field(&self.0, "method")?.as_string();
        let text = Self(field(&self.0, "text")?);
        let args = field(&self.0, "args")?;
        Ok(ConsoleLog {
            r#type: ConsoleLogType::from_method(method.as_deref()),
            text,
            args: (!args.is_undefined()).then_some(Self(args)),
        })
    }
    fn project_download(&self) -> Result<DownloadableFile<Self>, DiagnosticErrorInfo> {
        Ok(SandboxFile {
            file_name: Self(field(&self.0, "fileName")?),
            content: Self(field(&self.0, "content")?),
            mime_type: Self(field(&self.0, "mimeType")?),
        })
    }
    fn project_completion_error(&self) -> Result<SandboxError, DiagnosticErrorInfo> {
        let error = field(&self.0, "error")?;
        if error.is_null() || error.is_undefined() {
            return Ok(SandboxError {
                message: String::new(),
                stack: String::new(),
            });
        }
        Ok(SandboxError {
            message: field(&error, "message")?.as_string().unwrap_or_default(),
            stack: field(&error, "stack")?.as_string().unwrap_or_default(),
        })
    }
    fn success_response() -> Result<Self, DiagnosticErrorInfo> {
        let response = Object::new();
        set(&response, "success", &JsValue::TRUE)?;
        Ok(Self(response.into()))
    }
    fn attachment_array(attachments: &[Attachment]) -> Result<Self, DiagnosticErrorInfo> {
        let values = Array::new();
        for attachment in attachments {
            let object = Object::new();
            for (name, value) in [
                ("id", JsValue::from_str(&attachment.id)),
                ("fileName", JsValue::from_str(&attachment.file_name)),
                ("mimeType", JsValue::from_str(&attachment.mime_type)),
                ("size", JsValue::from(attachment.size)),
                ("content", JsValue::from_str(&attachment.content)),
                (
                    "extractedText",
                    attachment
                        .extracted_text
                        .as_deref()
                        .map_or(JsValue::UNDEFINED, JsValue::from_str),
                ),
            ] {
                set(&object, name, &value)?;
            }
            values.push(&object);
        }
        Ok(Self(values.into()))
    }
    fn project_artifact_operation(&self) -> Result<ArtifactOperation, DiagnosticErrorInfo> {
        let text = |name| {
            Ok::<_, DiagnosticErrorInfo>(field(&self.0, name)?.as_string().unwrap_or_default())
        };
        Ok(ArtifactOperation {
            action: text("action")?,
            filename: text("filename")?,
            content: text("content")?,
        })
    }
    fn artifact_snapshot(artifacts: IndexMap<String, String>) -> Result<Self, DiagnosticErrorInfo> {
        let snapshot = Object::new();
        for (filename, content) in &artifacts {
            set(&snapshot, filename, &JsValue::from_str(content))?;
        }
        Ok(Self(snapshot.into()))
    }
    fn artifact_response(response: ArtifactResponse) -> Result<Self, DiagnosticErrorInfo> {
        let reply = |success: bool, name: &str, value: JsValue| {
            let reply = Object::new();
            set(&reply, "success", &JsValue::from_bool(success))?;
            set(&reply, name, &value)?;
            Ok(Self(reply.into()))
        };
        match response {
            ArtifactResponse::Done => Self::success_response(),
            ArtifactResponse::Listed(names) => reply(
                true,
                "result",
                names
                    .iter()
                    .map(|name| JsValue::from_str(name))
                    .collect::<Array>()
                    .into(),
            ),
            ArtifactResponse::Read(content) => reply(true, "result", JsValue::from_str(&content)),
            ArtifactResponse::Failed(error) => reply(false, "error", JsValue::from_str(&error)),
        }
    }
}

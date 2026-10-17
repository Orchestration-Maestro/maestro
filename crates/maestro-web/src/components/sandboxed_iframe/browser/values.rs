//! Narrow native projections at message arrival and the application byte boundary.
use super::super::execution::ExecutionMessage;
use super::{BrowserSandboxValue, DiagnosticErrorInfo, diagnostic};
use crate::browser::sandbox_value::native_string;
use js_sys::{Reflect, Uint8Array};
use maestro_app::browser::components::{
    sandbox::{
        console_runtime_provider::{ConsoleLogType, ConsoleRuntimeProvider},
        sandbox_runtime_provider::RuntimeProviderValue,
    },
    sandboxed_iframe::{SandboxConsoleEntry, SandboxFileContent},
};
use maestro_app::{SandboxFile, SandboxResult};
use wasm_bindgen::JsCast as _;

/// Retain only the fields the execution collector consumes.
pub(super) fn select(
    message: &BrowserSandboxValue,
    kind: Option<&str>,
) -> Result<ExecutionMessage<BrowserSandboxValue, BrowserSandboxValue>, DiagnosticErrorInfo> {
    Ok(match kind {
        Some("file-returned") => {
            let file = message.project_download()?;
            ExecutionMessage::File(SandboxFile {
                file_name: file.file_name.0.as_string().unwrap_or_default(),
                content: file.content,
                mime_type: file.mime_type.0.as_string().unwrap_or_default(),
            })
        }
        Some("execution-complete") => {
            let value = Reflect::get(&message.0, &"returnValue".into())
                .map_err(|error| diagnostic(&error))?;
            ExecutionMessage::Complete(
                (!value.is_undefined()).then_some(BrowserSandboxValue(value)),
            )
        }
        Some("execution-error") => ExecutionMessage::Error(message.project_completion_error()?),
        _ => ExecutionMessage::Other,
    })
}
/// Snapshot the console owner's string producer after end hooks, without formatting returns.
pub(super) fn console_entries(
    console: &ConsoleRuntimeProvider<BrowserSandboxValue>,
) -> Vec<SandboxConsoleEntry> {
    let logs = console.get_logs();
    let logs = logs
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    logs.iter()
        .map(|entry| SandboxConsoleEntry {
            r#type: match entry.r#type {
                ConsoleLogType::Log => "log",
                ConsoleLogType::Warn => "warn",
                ConsoleLogType::Error => "error",
                ConsoleLogType::Info => "info",
            }
            .into(),
            text: entry.text.0.as_string().unwrap_or_default(),
        })
        .collect()
}
/// Take the application route's sole owned byte/text snapshot after settlement hooks.
pub(super) fn application_result(
    result: SandboxResult<BrowserSandboxValue, BrowserSandboxValue>,
) -> Result<SandboxResult<BrowserSandboxValue>, DiagnosticErrorInfo> {
    Ok(SandboxResult {
        success: result.success,
        console: result.console,
        error: result.error,
        return_value: result.return_value,
        files: result
            .files
            .map(|files| {
                files
                    .into_iter()
                    .map(|file| {
                        let content = if let Some(bytes) = file.content.0.dyn_ref::<Uint8Array>() {
                            SandboxFileContent::Bytes(bytes.to_vec())
                        } else {
                            let text = file
                                .content
                                .0
                                .as_string()
                                .map_or_else(|| native_string(&file.content.0), Ok)
                                .map_err(|error| diagnostic(&error))?;
                            SandboxFileContent::Text(text)
                        };
                        Ok(SandboxFile {
                            file_name: file.file_name,
                            mime_type: file.mime_type,
                            content,
                        })
                    })
                    .collect::<Result<Vec<_>, DiagnosticErrorInfo>>()
            })
            .transpose()?,
    })
}

//! Assembly of provider carriers, the inert configuration and the runtime bundle.
use super::document::encode_program;
use super::lowering;
use crate::components::sandbox::runtime_bundle::BUNDLE;
use maestro_app::{BuiltinRuntime, Runtime};
use maestro_json::compact_json;
use maestro_models::DiagnosticErrorInfo;
use std::fmt::Write as _;

/// Window property the runtime exposes for the whole-source completion signal.
pub const COMPLETION_PROPERTY: &str = "__maestroComplete";

/// What the document contributes as its supplied program.
pub(super) enum Program {
    /// No program: an authored HTML document.
    None,
    /// A compiled program whose outcome observer is installed under this property.
    Captured(String),
    /// A program the parser refused; the runtime settles it as a syntax error.
    Refused(String),
}

/// Everything one document's runtime insertion needs.
pub(super) struct Insertion<'a> {
    /// Sandbox identifier handed to the runtime and to supplied providers.
    pub id: &'a str,
    /// Merged globals as ordered pairs of serialized key and serialized value.
    pub globals: &'a [(String, String)],
    /// Ordered runtime entries of every provider.
    pub runtimes: &'a [Runtime],
    /// Whether the document omits the bridge and navigation interception.
    pub standalone: bool,
    /// The supplied program, if any.
    pub program: &'a Program,
}

/// Emit provider carriers, the configuration block and the bundle, in that order.
pub(super) fn assemble(insertion: &Insertion<'_>) -> Result<String, DiagnosticErrorInfo> {
    let mut markup = String::new();
    for runtime in insertion.runtimes {
        if let Runtime::Content(source) = runtime {
            let provider = lowering::provider(source)
                .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
            let _ = write!(
                markup,
                "<template data-maestro-sandbox-provider data-maestro-capture=\"{}\"><svg><script>{}</script></svg></template>",
                provider.capture(),
                encode_program(provider.code()),
            );
        }
    }
    let _ = write!(
        markup,
        "<script type=\"application/json\" data-maestro-sandbox>{}</script><script>{BUNDLE}</script>",
        configuration(insertion)?
    );
    Ok(markup)
}

/// The configuration the runtime reads, with `<` written so the block stays inert.
fn configuration(insertion: &Insertion<'_>) -> Result<String, DiagnosticErrorInfo> {
    let quote = |text: &str| {
        compact_json(&serde_json::Value::String(text.into()))
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
    };
    let mut text = format!("{{\"sandboxId\":{},\"globals\":[", quote(insertion.id)?);
    for (index, (key, value)) in insertion.globals.iter().enumerate() {
        let separator = if index == 0 { "" } else { "," };
        let _ = write!(text, "{separator}[{key},{value}]");
    }
    text.push_str("],\"runtime\":[");
    let names: Vec<&str> = insertion.runtimes.iter().filter_map(selection).collect();
    text.push_str(
        &names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(","),
    );
    let _ = write!(
        text,
        "],\"standalone\":{},\"completion\":\"{COMPLETION_PROPERTY}\"",
        insertion.standalone
    );
    match insertion.program {
        Program::None => {}
        Program::Captured(capture) => {
            let _ = write!(text, ",\"program\":{}", quote(capture)?);
        }
        Program::Refused(message) => {
            let _ = write!(text, ",\"programError\":{}", quote(message)?);
        }
    }
    text.push('}');
    Ok(text.replace('<', "\\u003c"))
}

/// The configuration name of a runtime entry, if it installs anything.
fn selection(runtime: &Runtime) -> Option<&'static str> {
    match runtime {
        Runtime::Builtin(BuiltinRuntime::Artifacts) => Some("artifacts"),
        Runtime::Builtin(BuiltinRuntime::Attachments) => Some("attachments"),
        Runtime::Builtin(BuiltinRuntime::Console) => Some("console"),
        Runtime::Builtin(BuiltinRuntime::FileDownload) => Some("fileDownload"),
        Runtime::Content(_) => Some("provider"),
        Runtime::Empty => None,
    }
}

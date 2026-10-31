//! Authored argument records and their consumption.

mod help;
mod parse;

pub(crate) use help::describe_options;
pub use help::print_help;
pub use parse::{is_valid_thinking_level, parse_args};

use indexmap::IndexMap;
use maestro_app::{FlagValue, ThinkingLevel};

/// The accepted output modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Plain text output.
    Text,
    /// One JSON document.
    Json,
    /// JSONL requests and responses.
    Rpc,
}

/// A request for every available model, or one limited by a search string.
#[derive(Debug, PartialEq, Eq)]
pub enum ListModels {
    /// Every available model.
    All,
    /// Models matching the retained search string.
    Search(String),
}

impl serde::Serialize for ListModels {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::All => serializer.serialize_bool(true),
            Self::Search(pattern) => serializer.serialize_str(pattern),
        }
    }
}

/// The severity of one argument diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticType {
    /// A warning that does not stop the operation.
    Warning,
    /// An error reported to the caller.
    Error,
}

/// One retained diagnostic with its exact message.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    /// The diagnostic severity.
    pub r#type: DiagnosticType,
    /// The diagnostic text.
    pub message: String,
}

/// The parsed command-line arguments.
///
/// Supplied options stay absent until authored, while messages, file arguments,
/// unknown flags and diagnostics always exist.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Args {
    /// Selected provider name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Selected model pattern or identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Supplied API key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Replaced system prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Appended system prompt parts in authored order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append_system_prompt: Option<Vec<String>>,
    /// Selected reasoning level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingLevel>,
    /// Whether the previous session continues.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#continue: Option<bool>,
    /// Whether a session is chosen to resume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<bool>,
    /// Whether help is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<bool>,
    /// Whether the version is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<bool>,
    /// Selected output mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// Whether the session is unsaved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_session: Option<bool>,
    /// Session file or partial identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Session file or partial identifier to fork.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork: Option<String>,
    /// Session storage directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_dir: Option<String>,
    /// Model patterns for cycling, with empty segments retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    /// Tool allowlist, with empty names dropped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Whether every tool is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_tools: Option<bool>,
    /// Whether only built-in tools are disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_builtin_tools: Option<bool>,
    /// Explicitly loaded extension paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    /// Whether extension discovery is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_extensions: Option<bool>,
    /// Whether the prompt is processed without interaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print: Option<bool>,
    /// Session file to export.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export: Option<String>,
    /// Whether skill discovery is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_skills: Option<bool>,
    /// Explicitly loaded skill paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    /// Explicitly loaded prompt template paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_templates: Option<Vec<String>>,
    /// Whether prompt template discovery is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_prompt_templates: Option<bool>,
    /// Explicitly loaded theme paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    /// Whether theme discovery is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_themes: Option<bool>,
    /// Whether context file discovery is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_context_files: Option<bool>,
    /// Retained model listing request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_models: Option<ListModels>,
    /// Whether startup network operations are disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline: Option<bool>,
    /// Whether verbose startup is forced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
    /// Authored messages in their own order.
    #[serde(serialize_with = "serialize_messages")]
    pub messages: std::collections::VecDeque<String>,
    /// Authored file arguments without their prefix.
    pub file_args: Vec<String>,
    /// Unrecognized flags with their retained values in insertion order.
    #[serde(serialize_with = "serialize_unknown_flags")]
    pub unknown_flags: IndexMap<String, FlagValue>,
    /// Diagnostics in their authored order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Serialize the retained flags as an ordered sequence of name and value pairs.
fn serialize_unknown_flags<S: serde::Serializer>(
    flags: &IndexMap<String, FlagValue>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(flags.iter())
}

/// Serialize the prompt queue as one ordered array.
fn serialize_messages<S: serde::Serializer>(
    messages: &std::collections::VecDeque<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(messages)
}

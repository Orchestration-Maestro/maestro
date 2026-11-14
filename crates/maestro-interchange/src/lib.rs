//! The records a guest shares with its host: model requests, session, tool, package, toolkit,
//! child-process, extension-context and front matter records.
pub mod credentials;
pub mod diagnostics;
pub mod entry;
pub mod extension_context;
pub mod messages;
pub mod metadata;
pub mod model_registry;
pub mod native_io;
pub mod oauth;
pub mod packages;
pub mod parameter_schema;
pub mod presence;
pub mod provider_config;
pub mod skills;
pub mod slash_commands;
pub mod source_info;
pub mod tool;
pub mod tool_info;
pub mod tui;
pub mod types;

pub mod extension_events;
pub mod system_prompt;

/// Opt-in helpers for controlled tests.
#[cfg(feature = "test-support")]
pub mod test_support;

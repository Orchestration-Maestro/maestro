#![doc = include_str!("../../../docs/records.md")]

pub mod arguments;
pub use arguments::*;
mod builtins;
pub use builtins::{find_env_keys, get_env_api_key};

pub mod cancellation;
mod catalog;
mod options;
pub use options::{
    AdjustedMaxTokens, adjust_max_tokens_for_thinking, build_base_options, clamp_reasoning,
    is_context_overflow,
};
mod projection;
pub mod records;
pub use projection::transform_messages;

pub use cancellation::Cancellation;
pub use catalog::models::{
    calculate_cost, clamp_thinking_level, get_model, get_models, get_providers,
    get_supported_thinking_levels, models_are_equal,
};

pub use records::api_registry::{
    ApiProvider, ApiStreamFunction, ApiStreamSimpleFunction, clear_api_providers, get_api_provider,
    get_api_providers, register_api_provider, unregister_api_providers,
};
pub use records::diagnostics::{
    AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo, DiagnosticInput,
    append_assistant_message_diagnostic, create_assistant_message_diagnostic,
    extract_diagnostic_error, format_thrown_value,
};
pub use records::event_stream::EventStream;
pub use records::event_stream::create_assistant_message_event_stream;
pub use records::hash::short_hash;
pub use records::headers::headers_to_record;
pub use records::session_resources::{
    SessionResourceCleanup, SessionResourceCleanupError, SessionResourceRemoval,
    cleanup_session_resources, register_session_resource_cleanup,
};
pub use records::stream::{complete, complete_simple, stream, stream_simple};
pub use records::typebox_helpers::{StringEnumOptions, string_enum};
pub use records::types::*;

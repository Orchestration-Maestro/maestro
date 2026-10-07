#![doc = include_str!("../../../docs/records.md")]

pub mod cancellation;
pub mod records;

pub use cancellation::Cancellation;

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

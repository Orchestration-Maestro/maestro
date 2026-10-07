//! Caller-supplied model invocation, offline descriptors and shared producer-owned events.
#![doc = include_str!("../../../docs/records.md")]
#![doc = include_str!("../../../docs/models/catalog.md")]
mod auth;
mod cancellation;
mod catalog;
mod projection;
/// Supplied records and independent invocation modules.
pub mod records;
mod scalar;
mod schema;
mod types;
mod validation;
pub use auth::{
    AuthResolver, AuthStatus, RequestAuth, SecretString, TokenExchange, TokenExchangeResult,
};
pub use cancellation::Cancellation;
pub use catalog::*;
pub use projection::project_context;
pub use records::hash::*;
pub use records::headers::*;
pub use records::session_resources::*;
pub use records::typebox_helpers::*;
pub use records::{api_registry::*, diagnostics::*, event_stream::*, stream::*, types::*};
pub use types::Failure;
pub use validation::{ToolValidationError, validate_tool_call};

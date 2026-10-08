//! Model, message and diagnostic records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod diagnostics;
mod types;

pub use diagnostics::{AssistantMessageDiagnostic, DiagnosticCode, DiagnosticErrorInfo};
pub use types::*;

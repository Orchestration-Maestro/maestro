//! Lossless adapters between the shared diagnostic record and canonical wire records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
#[cfg(all(test, not(target_arch = "wasm32")))]
use crate::bindings::host_side::maestro::extension::host as wire;
#[cfg(target_arch = "wasm32")]
use crate::bindings::maestro::extension::host as wire;
use maestro_request::diagnostics::{DiagnosticCode, DiagnosticErrorInfo};

/// Preserve every supplied diagnostic field on the wire.
#[cfg(target_arch = "wasm32")]
pub(crate) fn to_wire(error: DiagnosticErrorInfo) -> wire::DiagnosticErrorInfo {
    wire::DiagnosticErrorInfo {
        name: error.name,
        message: error.message,
        stack: error.stack,
        code: error.code.map(|code| match code {
            DiagnosticCode::Text(value) => wire::DiagnosticCode::Text(value),
            DiagnosticCode::Number(value) => wire::DiagnosticCode::Number(value),
        }),
    }
}
/// Recover the shared diagnostic without rebuilding it from display text.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) fn from_wire(error: wire::DiagnosticErrorInfo) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: error.name,
        message: error.message,
        stack: error.stack,
        code: error.code.map(|code| match code {
            wire::DiagnosticCode::Text(value) => DiagnosticCode::Text(value),
            wire::DiagnosticCode::Number(value) => DiagnosticCode::Number(value),
        }),
    }
}

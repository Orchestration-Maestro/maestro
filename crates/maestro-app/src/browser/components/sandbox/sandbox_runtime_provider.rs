//! Retained runtime provider interface.
use super::super::sandboxed_iframe::SandboxValue;
use maestro_agent::agent::Native;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use std::sync::Arc;

/// Native response callback preserving caller failures.
#[cfg(not(target_arch = "wasm32"))]
pub type SandboxRespond<V> = Arc<dyn Fn(V) -> Result<(), DiagnosticErrorInfo> + Send + Sync>;
/// Browser-local response callback preserving caller failures.
#[cfg(target_arch = "wasm32")]
pub type SandboxRespond<V> = Arc<dyn Fn(V) -> Result<(), DiagnosticErrorInfo>>;
/// Supplied data and self-contained function source for injection.
pub trait SandboxRuntimeProvider<V: SandboxValue = serde_json::Value>: Native {
    /// Obtain owned globals in insertion order.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_data(&self) -> Result<indexmap::IndexMap<String, V>, DiagnosticErrorInfo>;
    /// Obtain the complete function source.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo>;
    /// Obtain model-facing helper guidance.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo>;
    /// Handle an incoming message; by default no action is needed.
    fn handle_message(
        &self,
        _message: V,
        _respond: SandboxRespond<V>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(async { Ok(()) })
    }
    /// Notify execution start; by default no action is needed.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn on_execution_start(
        &self,
        _sandbox_id: &str,
        _signal: Option<Cancellation>,
    ) -> Result<(), DiagnosticErrorInfo> {
        Ok(())
    }
    /// Notify execution settlement; by default no action is needed.
    ///
    /// # Errors
    /// Returns the provider's diagnostic.
    fn on_execution_end(&self, _sandbox_id: &str) -> Result<(), DiagnosticErrorInfo> {
        Ok(())
    }
}

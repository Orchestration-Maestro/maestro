//! Source generation for sandbox and extension runtime messages.
use maestro_app::{DiagnosticErrorInfo, compact_json};

/// Messaging intent available to runtime callers.
pub enum MessageType {
    /// A message expecting a correlated reply.
    RequestResponse,
    /// A one-way notification.
    FireAndForget,
}
/// Host in which the injected messaging globals execute.
#[derive(Clone, Copy)]
pub enum RuntimeMessageContext {
    /// A frame communicating with its parent window.
    SandboxIframe,
    /// An extension user script using native messaging.
    UserScript,
}
/// Supplied execution channel and unchanged sandbox identifier.
#[derive(Clone, Copy)]
pub struct RuntimeMessageBridgeOptions<'a> {
    /// Execution channel.
    pub context: RuntimeMessageContext,
    /// Identifier embedded as a JavaScript string literal.
    pub sandbox_id: &'a str,
}
/// Generator for the complete injectable messaging globals.
pub struct RuntimeMessageBridge;
impl RuntimeMessageBridge {
    /// Generate channel-specific messaging and shared completion registration.
    ///
    /// ```rust
    /// use maestro_web::RuntimeMessageBridge;
    /// use maestro_web::components::sandbox::routing::{
    ///     RuntimeMessageBridgeOptions, RuntimeMessageContext,
    /// };
    ///
    /// let source = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
    ///     context: RuntimeMessageContext::SandboxIframe,
    ///     sandbox_id: "execution",
    /// })?;
    /// assert!(source.contains("window.sendRuntimeMessage"));
    /// # Ok::<(), maestro_app::DiagnosticErrorInfo>(())
    /// ```
    ///
    /// # Errors
    /// Returns the string serializer's diagnostic.
    pub fn generate_bridge_code(
        options: RuntimeMessageBridgeOptions<'_>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let template = match options.context {
            RuntimeMessageContext::SandboxIframe => {
                include_str!("routing/sandbox_iframe.js")
            }
            RuntimeMessageContext::UserScript => {
                include_str!("routing/user_script.js")
            }
        };
        let id = compact_json(&serde_json::Value::String(options.sandbox_id.into()))
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
        let body = template.trim().replacen("__SANDBOX_ID__", &id, 1);
        Ok(include_str!("routing/runtime.js")
            .trim()
            .replacen("__SEND_RUNTIME_MESSAGE__", &body, 1))
    }
}

//! Source generation for sandbox and extension runtime messages.
use super::runtime_bundle::BUNDLE;
use maestro_json::compact_json;
use maestro_models::DiagnosticErrorInfo;

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
    /// Generate the runtime script that installs one channel's messaging and the shared
    /// completion registration.
    ///
    /// Every channel is the sandbox runtime bundle preceded by the channel's role and the
    /// identifier as program arguments: `frame-bridge` for a frame talking to its parent
    /// window, `user-script` for an extension user script using native messaging.
    ///
    /// ```rust
    /// use maestro_web::RuntimeMessageBridge;
    /// use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
    /// use maestro_web::components::sandbox::routing::{
    ///     RuntimeMessageBridgeOptions, RuntimeMessageContext,
    /// };
    ///
    /// let source = RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
    ///     context: RuntimeMessageContext::SandboxIframe,
    ///     sandbox_id: "execution",
    /// })?;
    /// assert!(source.starts_with("var Module={arguments:[\"frame-bridge\",\"execution\"]};"));
    /// assert!(source.ends_with(BUNDLE));
    /// # Ok::<(), maestro_app::DiagnosticErrorInfo>(())
    /// ```
    ///
    /// # Errors
    /// Returns the string serializer's diagnostic.
    pub fn generate_bridge_code(
        options: RuntimeMessageBridgeOptions<'_>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let role = match options.context {
            RuntimeMessageContext::SandboxIframe => "frame-bridge",
            RuntimeMessageContext::UserScript => "user-script",
        };
        let arguments = compact_json(&serde_json::json!([role, options.sandbox_id]))
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
        Ok(format!("var Module={{arguments:{arguments}}};{BUNDLE}"))
    }
}

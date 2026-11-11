//! Source generation for sandbox and extension runtime messages.
use super::runtime_bundle::BUNDLE;
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
    /// Generate the runtime script that installs one channel's messaging and the shared
    /// completion registration.
    ///
    /// The frame channel is the sandbox runtime bundle preceded by the channel and
    /// identifier as program arguments. The extension user-script channel keeps its own
    /// script until that world is qualified for the bundle's WebAssembly policy.
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
        let serialized = |value: &serde_json::Value| {
            compact_json(value).map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
        };
        match options.context {
            RuntimeMessageContext::SandboxIframe => {
                let arguments =
                    serialized(&serde_json::json!(["frame-bridge", options.sandbox_id]))?;
                Ok(format!("var Module={{arguments:{arguments}}};{BUNDLE}"))
            }
            RuntimeMessageContext::UserScript => {
                let id = serialized(&serde_json::Value::String(options.sandbox_id.into()))?;
                let body = include_str!("../../../assets/user_script_bridge/user_script.js")
                    .trim()
                    .replacen("__SANDBOX_ID__", &id, 1);
                Ok(
                    include_str!("../../../assets/user_script_bridge/runtime.js")
                        .trim()
                        .replacen("__SEND_RUNTIME_MESSAGE__", &body, 1),
                )
            }
        }
    }
}

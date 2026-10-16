//! Assembly of already-serialized globals and complete provider functions.
use super::document::script_resource;
use crate::components::sandbox::runtime_message_bridge::{
    RuntimeMessageBridge, RuntimeMessageBridgeOptions, RuntimeMessageContext,
};
use maestro_app::{DiagnosticErrorInfo, compact_json};
use std::fmt::Write as _;

/// Emit globals, optional bridge, provider invocations and optional navigation in order.
pub(super) fn assemble(
    id: &str,
    globals: &str,
    functions: &[String],
    standalone: bool,
) -> Result<String, DiagnosticErrorInfo> {
    let quoted = compact_json(&serde_json::Value::String(id.into()))
        .map_err(|e| DiagnosticErrorInfo::error(e.to_string()))?;
    let bridge = if standalone {
        String::new()
    } else {
        RuntimeMessageBridge::generate_bridge_code(RuntimeMessageBridgeOptions {
            context: RuntimeMessageContext::SandboxIframe,
            sandbox_id: id,
        })?
    };
    let mut script = format!("window.sandboxId = {quoted};\n{globals}\n{bridge}\n");
    for function in functions {
        let _ = writeln!(script, "({function})({quoted});");
    }
    if !standalone {
        script.push_str(include_str!("navigation.js"));
    }
    Ok(include_str!("runtime.html").replacen("__SCRIPT__", &script_resource(&script), 1))
}

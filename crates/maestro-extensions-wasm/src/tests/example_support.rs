//! Support shared by the tests of the shipped extension examples.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::Value;

use super::controlled::Controlled;
use super::host::Harness;
use super::host_family::guest::{ToolCapabilities, ToolInvocation};
use crate::component_adapter::Exports;
use crate::loader::Extension;

/// Starts a shipped factory on a fresh controlled host.
pub async fn start<E: Extension>() -> Result<(Controlled, Exports<Controlled>), String> {
    let host = Controlled::default();
    let exports = Exports::new(host.clone());
    exports.start::<E>().await?;
    Ok((host, exports))
}

/// Starts a shipped factory on a fresh controlled host and returns the key of the tool it
/// registered under `tool`.
pub async fn registered<E: Extension>(
    tool: &str,
) -> Result<(Controlled, u32, Exports<Controlled>), String> {
    let (host, exports) = start::<E>().await?;
    let key = host.observed().callback(tool)?;
    Ok((host, key, exports))
}

/// Decodes the SDK's opaque details document before comparing a caller-visible result.
pub fn result_value(encoded: &str) -> Value {
    let mut value: Value = serde_json::from_str(encoded).unwrap();
    value["details"] = serde_json::from_str(value["details"].as_str().unwrap()).unwrap();
    value
}

/// Runs the built component's command callback and returns its outcome.
pub fn invoke_command_on_component(
    harness: &mut Harness,
    key: u32,
    args: &str,
) -> Result<(), String> {
    let ctx = harness
        .command_context("/work")
        .map_err(|error| error.to_string())?;
    let exports = harness.exports.clone();
    super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_command(accessor, super::host::borrow(key), args.to_owned(), ctx)
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    Ok(())
}

/// Invokes a tool callback of the built component and returns its encoded result.
pub fn invoke_tool_on_component(
    harness: &mut Harness,
    key: u32,
    params: &Value,
    resources: ToolCapabilities,
) -> Result<String, String> {
    let invocation = ToolInvocation {
        tool_call_id: "component-call".into(),
        params: params.to_string(),
    };
    let exports = harness.exports.clone();
    let encoded = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_tool(accessor, super::host::borrow(key), invocation, resources)
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    Ok(encoded)
}

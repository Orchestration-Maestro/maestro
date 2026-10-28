//! Authored agents running in separate Maestro processes.

/// Authored definitions and directory discovery.
pub mod agents;
/// Tool registration over guest capabilities.
mod extension;
/// Child observations and native process isolation.
mod runs;

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the authored-agent tool.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.register_tool(extension::tool(api.clone())) })
}

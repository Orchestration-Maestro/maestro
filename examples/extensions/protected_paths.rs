//! An opt-in tool-call hook that blocks writes and edits to illustrative path substrings.
// maestro-component: MAESTRO_PROTECTED_PATHS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    NotificationType, Presence, ToolCallEventResult,
};
use serde::Deserialize;

/// The only tool operand the hook reads during its invocation.
#[derive(Deserialize)]
struct PathInput {
    /// The authored path, retained without normalization.
    path: String,
}

/// Returns the protection verdict for a write or edit call.
fn tool_call(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::ToolCall(event) = event else {
            return Ok(None);
        };
        if !matches!(event.tool_name.as_str(), "write" | "edit") {
            return Ok(None);
        }
        let PathInput { path } = event
            .input
            .decode::<PathInput>()
            .map_err(|error| error.to_string())?;
        if ![".env", ".git/", "node_modules/"]
            .iter()
            .any(|pattern| path.contains(pattern))
        {
            return Ok(None);
        }
        if ctx.has_ui()? {
            ctx.ui()?.notify(
                &format!("Blocked write to protected path: {path}"),
                Some(NotificationType::Warning),
            )?;
        }
        Ok(Some(ExtensionEventResult::ToolCall(ToolCallEventResult {
            block: Presence::Present(true),
            reason: Presence::Present(format!("Path \"{path}\" is protected")),
        })))
    })
}

/// Registers the opt-in protection hook through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("tool_call", Rc::new(tool_call)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

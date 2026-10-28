//! An opt-in confirmation hook for illustrative dangerous bash patterns.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    Presence, ToolCallEventResult,
};
use regex::RegexSet;
use serde::Deserialize;

/// ASCII word boundaries, whitespace scalars and line terminators used by the three patterns.
const DANGEROUS_PATTERNS: [&str; 3] = [
    r"(?i-u:\brm)[\t-\r \u{00a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]+(?i-u:-rf?|--recursive)",
    r"(?i-u:\bsudo\b)",
    r"(?i-u:\b(chmod|chown)\b)[^\r\n\u{2028}\u{2029}]*777",
];

/// The bash operand the hook reads without parsing the shell command.
#[derive(Deserialize)]
struct BashInput {
    /// The command exactly as the caller authored it.
    command: String,
}

/// Returns a refusal unless a dangerous bash call receives explicit approval.
fn tool_call(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    patterns: Rc<RegexSet>,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::ToolCall(event) = event else {
            return Ok(None);
        };
        if event.tool_name != "bash" {
            return Ok(None);
        }
        let BashInput { command } = event
            .input
            .decode::<BashInput>()
            .map_err(|error| error.to_string())?;
        if !patterns.is_match(&command) {
            return Ok(None);
        }
        let reason = if ctx.has_ui()? {
            let choice = ctx
                .ui()?
                .select(
                    format!("⚠️ Dangerous command:\n\n  {command}\n\nAllow?"),
                    vec!["Yes".into(), "No".into()],
                    None,
                )
                .await?;
            if choice.as_deref() == Some("Yes") {
                return Ok(None);
            }
            "Blocked by user"
        } else {
            "Dangerous command blocked (no UI for confirmation)"
        };
        Ok(Some(ExtensionEventResult::ToolCall(ToolCallEventResult {
            block: Presence::Present(true),
            reason: Presence::Present(reason.to_owned()),
        })))
    })
}

/// Registers the bash confirmation hook through the supplied host.
///
/// # Errors
/// The returned future fails when the dangerous-command patterns do not compile, or
/// with the host's rejection when it refuses the handler registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let patterns =
            Rc::new(RegexSet::new(DANGEROUS_PATTERNS).map_err(|error| error.to_string())?);
        api.on(
            "tool_call",
            Rc::new(move |event, ctx| tool_call(event, ctx, Rc::clone(&patterns))),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

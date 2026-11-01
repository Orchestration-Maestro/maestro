//! Reports the effective system prompt's length as extension status.
//!
//! The length the widget shows counts UTF-16 code units of the prompt the host supplies
//! when the handler runs, including whitespace and combining characters. Nothing is
//! trimmed, rebuilt or cached, and shutdown removes the status it set.
// maestro-component: MAESTRO_SYSTEM_PROMPT_HEADER_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the status write on start and its removal on shutdown.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.on(
            "agent_start",
            Rc::new(|_, ctx| {
                Box::pin(async move {
                    let prompt = ctx.get_system_prompt()?;
                    let length = prompt.encode_utf16().count();
                    ctx.ui()?
                        .set_status("system-prompt", Some(&format!("System: {length} chars")))?;
                    Ok(None)
                })
            }),
        )?;
        api.on(
            "session_shutdown",
            Rc::new(|_, ctx| {
                Box::pin(async move {
                    ctx.ui()?.set_status("system-prompt", None)?;
                    Ok(None)
                })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

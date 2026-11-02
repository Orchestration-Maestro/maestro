//! Shows themed turn progress as persistent footer status.
// maestro-component: MAESTRO_STATUS_LINE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::Cell;
use std::rc::Rc;

use maestro_extensions_wasm::{ExtensionAPI, ExtensionEvent, ExtensionFuture, ExtensionHandler};

/// Registers session readiness and a turn count shared by this factory's hooks.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses a handler
/// registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    let turn_count = Cell::new(0_u64);
    let handler: ExtensionHandler = Rc::new(move |event, ctx| {
        if matches!(event, ExtensionEvent::TurnStart(_)) {
            turn_count.set(turn_count.get() + 1);
        }
        let count = turn_count.get();
        Box::pin(async move {
            let ui = ctx.ui()?;
            let theme = ui.theme()?;
            let text = match event {
                ExtensionEvent::TurnStart(_) => {
                    let spinner = theme.fg("accent", "●")?;
                    spinner + &theme.fg("dim", &format!(" Turn {count}..."))?
                }
                ExtensionEvent::TurnEnd(_) => {
                    let check = theme.fg("success", "✓")?;
                    check + &theme.fg("dim", &format!(" Turn {count} complete"))?
                }
                _ => theme.fg("dim", "Ready")?,
            };
            ui.set_status("status-demo", Some(&text))?;
            Ok(None)
        })
    });
    Box::pin(async move {
        for event in ["session_start", "turn_start", "turn_end"] {
            api.on(event, handler.clone())?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

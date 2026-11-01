//! An extension that sets the label shown when thinking blocks are hidden.
// maestro-component: MAESTRO_HIDDEN_THINKING_LABEL_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionHandler};

/// The label this example starts with and returns to on reset. A reset clears the host's
/// label, and the next session start applies this one again.
const DEFAULT_LABEL: &str = "Pondering...";

/// Applies the label the factory currently retains at every session start.
fn session_start(label: Rc<RefCell<Rc<str>>>) -> ExtensionHandler {
    Rc::new(move |_event, ctx| {
        let label = Rc::clone(&label);
        Box::pin(async move {
            let current = Rc::clone(&label.borrow());
            ctx.ui()?.set_hidden_thinking_label(Some(&current))?;
            Ok(None)
        })
    })
}

/// The `thinking-label` command, which sets a custom label or restores the default.
fn command(label: Rc<RefCell<Rc<str>>>) -> CommandOptions {
    CommandOptions {
        description: Some("Set the hidden thinking label. Use without args to reset.".into()),
        handler: Rc::new(move |args, ctx| {
            let label = Rc::clone(&label);
            Box::pin(async move {
                let ui = ctx.ui()?;
                let next = args.trim();
                if next.is_empty() {
                    *label.borrow_mut() = Rc::from(DEFAULT_LABEL);
                    ui.set_hidden_thinking_label(None)?;
                    return ui.notify(
                        &format!("Hidden thinking label reset to: {DEFAULT_LABEL}"),
                        None,
                    );
                }
                let next: Rc<str> = Rc::from(next);
                *label.borrow_mut() = Rc::clone(&next);
                ui.set_hidden_thinking_label(Some(&next))?;
                ui.notify(&format!("Hidden thinking label set to: {next}"), None)
            })
        }),
        get_argument_completions: None,
    }
}

/// Registers the session-start handler followed by the label command.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let label: Rc<RefCell<Rc<str>>> = Rc::new(RefCell::new(Rc::from(DEFAULT_LABEL)));
        api.on("session_start", session_start(Rc::clone(&label)))?;
        api.register_command("thinking-label", command(label))
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

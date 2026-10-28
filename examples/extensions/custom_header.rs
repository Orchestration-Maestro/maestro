//! Replaces the startup header with a themed illustration and restores it on command.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    CallbackEmitter, CommandOptions, Component, ComponentView, ExtensionAPI, ExtensionComponent,
    ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture, ExtensionResult,
    NotificationType, Theme, config,
};

/// A header that renders the host version with the live theme its factory received.
struct Header(Theme, String);

impl ComponentView for Header {
    fn render(&self, _width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let accent = |text| self.0.fg("accent", text);
        Ok(vec![
            String::new(),
            accent("       ▄")?,
            accent("      ▀█▀")?,
            accent("       ▀")?,
            accent("  █▄       ▄█")?,
            accent("  █ ▀▄   ▄▀ █")?,
            accent("  █   ▀█▀   █")?,
            String::new(),
            format!(
                "{}{}",
                self.0.fg("muted", "   Custom header")?,
                self.0.fg("dim", &format!(" v{}", self.1))?
            ),
        ])
    }

    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Installs the illustration when the supplied context has UI.
fn session_start(
    api: ExtensionAPI,
    _event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if ctx.has_ui()? {
            let version = config::version(&api)?;
            ctx.ui()?
                .set_header(Some(Rc::new(move |_tui, theme, _emitter| {
                    Ok(ExtensionComponent {
                        component: Component::new(Header(theme, version.clone())),
                        dispose: None,
                        set_expanded: None,
                        width: None,
                    })
                })))?;
        }
        Ok(None)
    })
}

/// Registers the UI-only startup header and the command restoring the built-in header.
///
/// # Errors
/// Returns the host's rejection when it refuses a registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.on(
            "session_start",
            Rc::new(move |event, ctx| session_start(registered.clone(), event, ctx)),
        )?;
        api.register_command(
            "builtin-header",
            CommandOptions {
                description: Some("Restore built-in header with keybinding hints".into()),
                get_argument_completions: None,
                handler: Rc::new(|_args, ctx| {
                    Box::pin(async move {
                        let ui = ctx.ui()?;
                        ui.set_header(None)?;
                        ui.notify("Built-in header restored", Some(NotificationType::Info))
                    })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

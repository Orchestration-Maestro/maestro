//! A retained terminal view authored once for the controlled and production hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionComponent, ExtensionResult,
    ExtensionUIContext, TUI,
};
use std::rc::Rc;

/// The original terminal, retained after its factory returns.
struct View(TUI);

impl ComponentView for View {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("redraws={}", self.0.full_redraws()?)])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Shows a component that reads its original terminal on every render.
pub fn install(ui: &ExtensionUIContext) -> ExtensionResult<()> {
    ui.set_widget_factory(
        "terminal",
        Rc::new(|tui, _, _| {
            Ok(ExtensionComponent {
                component: Component::new(View(tui)),
                dispose: None,
                set_expanded: None,
                width: None,
            })
        }),
        None,
    )
}

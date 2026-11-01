//! A factory component with independent width and expansion capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::Cell, rc::Rc};

use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, ExtensionComponent, ExtensionResult,
    ExtensionUIContext,
};
use serde_json::json;

/// The expansion choice shared by the callback and the retained view.
struct View(Rc<Cell<bool>>);

impl ComponentView for View {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("expanded={} width={width}", self.0.get())])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Installs a component whose expansion callback is retained independently of its factory.
pub fn install(
    ui: &ExtensionUIContext,
    width: Option<f64>,
    expandable: bool,
) -> ExtensionResult<()> {
    ui.set_widget_factory(
        "capabilities",
        Rc::new(move |_, _, _| {
            let expanded = Rc::new(Cell::new(false));
            let state = expanded.clone();
            Ok(ExtensionComponent {
                component: Component::new(View(expanded)),
                dispose: None,
                set_expanded: expandable.then_some(Rc::new(
                    move |expanded, emitter: CallbackEmitter<'_>| {
                        emitter.emit("factory-expanded", json!(expanded))?;
                        state.set(expanded);
                        if expanded {
                            Ok(())
                        } else {
                            Err("expansion rejected 雪\n".to_owned())
                        }
                    },
                )
                    as maestro_extensions_wasm::ComponentExpansionHandler),
                width,
            })
        }),
        None,
    )
}

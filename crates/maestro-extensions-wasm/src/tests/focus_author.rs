//! Authored focus state, synchronous setters and the hardware-cursor marker.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CURSOR_MARKER, CallbackEmitter, Component, ComponentView, ExtensionComponent, ExtensionResult,
    ExtensionUIContext, Focusable,
};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Focus state retained by an authored component.
#[derive(Default)]
struct Field {
    /// Current focus.
    focused: Cell<bool>,
    /// Setter calls, including repeated assignments.
    changes: RefCell<Vec<bool>>,
}
impl ComponentView for Field {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let marker = if self.focused.get() {
            CURSOR_MARKER
        } else {
            ""
        };
        Ok(vec![format!("field{marker} {:?}", self.changes.borrow())])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl Focusable for Field {
    fn focused(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(self.focused.get())
    }
    fn set_focused(&self, focused: bool, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.focused.set(focused);
        self.changes.borrow_mut().push(focused);
        emitter.emit("focus-changed", json!(focused))
    }
}

/// Creates an independently retained authored field.
pub fn component() -> Component {
    Component::new(Field::default())
}

/// Installs a field whose focus setters the host invokes after its factory returns.
pub fn install(ui: &ExtensionUIContext) -> ExtensionResult<()> {
    ui.set_widget_factory(
        "focus",
        Rc::new(|_, _, _| {
            Ok(ExtensionComponent {
                component: component(),
                dispose: None,
                set_expanded: None,
                width: None,
            })
        }),
        None,
    )
}

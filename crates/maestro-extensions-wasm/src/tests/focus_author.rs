//! Authored focus state, synchronous setters and the hardware-cursor marker.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CURSOR_MARKER, CallbackEmitter, Component, ComponentView, CustomEditor, Editor, EditorOptions,
    ExtensionComponent, ExtensionResult, ExtensionUIContext, Focusable, get_editor_theme,
    get_keybindings, is_focusable,
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

/// Gives focus through each component conversion of the host editors.
pub fn install_editors(ui: &ExtensionUIContext) -> ExtensionResult<()> {
    ui.set_widget_factory(
        "editor-focus",
        Rc::new(|tui, _, emitter| {
            let theme = get_editor_theme()?;
            let editor = Editor::new(&tui, &theme, EditorOptions::default())?;
            check_editor(&editor, &Component::from(&editor), emitter)?;
            check_editor(&editor, &Component::new(editor.clone()), emitter)?;
            let custom =
                CustomEditor::new(&tui, &theme, &get_keybindings()?, EditorOptions::default())?;
            check_editor(&custom, &Component::from(&custom), emitter)?;
            check_editor(&custom, &Component::new(custom.clone()), emitter)?;
            Ok(ExtensionComponent {
                component: Component::from(&editor),
                dispose: None,
                set_expanded: None,
                width: None,
            })
        }),
        None,
    )
}

/// Both views read and change the same native focus flag.
fn check_editor(
    editor: &Editor,
    view: &Component,
    emitter: CallbackEmitter<'_>,
) -> ExtensionResult<()> {
    assert!(is_focusable(Some(view)));
    assert_eq!(view.focused(emitter)?, Some(false));
    view.set_focused(true, emitter)?;
    assert!(editor.focused()?);
    assert_eq!(view.focused(emitter)?, Some(true));
    view.set_focused(false, emitter)?;
    assert!(!editor.focused()?);
    Ok(())
}

//! A minimal authored editor used by a synchronous multi-operation component factory.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AutocompleteProvider, CallbackEmitter, Component, ComponentFactory, ComponentView,
    EditorComponent, ExtensionComponent, ExtensionResult, ExtensionUIContext, Style, TextHandler,
};
use serde_json::json;

/// A plain editor with no optional capabilities.
#[derive(Default)]
struct Plain {
    /// Current text.
    text: RefCell<String>,
    /// Submission handler.
    submit: RefCell<Option<TextHandler>>,
    /// Change handler.
    change: RefCell<Option<TextHandler>>,
}

impl ComponentView for Plain {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![self.text.borrow().clone()])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

impl EditorComponent for Plain {
    fn get_text(&self, _: CallbackEmitter<'_>) -> ExtensionResult<String> {
        Ok(self.text.borrow().clone())
    }
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.text.replace(text.to_owned());
        let handler = self.change.borrow().clone();
        handler.map_or(Ok(()), |handler| handler(text, emitter))
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let handler = self.submit.borrow().clone();
        let text = self.text.borrow().clone();
        if data == "enter" {
            handler.map_or(Ok(()), |handler| handler(&text, emitter))
        } else {
            self.set_text(&format!("{text}{data}"), emitter)
        }
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.submit.replace(handler);
        Ok(())
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.change.replace(handler);
        Ok(())
    }
    fn on_change(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        Ok(self.change.borrow().clone())
    }
}

/// An editor exposed as an ordinary component delegates its required editor input handler.
struct View(Rc<dyn EditorComponent>);

impl ComponentView for View {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::handle_input(self.0.as_ref(), data, emitter)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}

/// Reports absence independently for every optional operation.
fn optional(
    editor: &dyn EditorComponent,
    provider: Rc<dyn AutocompleteProvider>,
    emitter: CallbackEmitter<'_>,
) -> ExtensionResult<()> {
    let absent = [
        editor.add_to_history("past", emitter)?.is_none(),
        editor.insert_text_at_cursor("absent", emitter)?.is_none(),
        editor.get_expanded_text(emitter)?.is_none(),
        editor.set_padding_x(7.0, emitter)?.is_none(),
        editor.set_autocomplete_max_visible(3.0, emitter)?.is_none(),
        editor.border_color(emitter)?.is_none(),
        editor
            .set_border_color(Style::new(|text| Ok(text.to_owned())), emitter)?
            .is_none(),
        editor
            .set_autocomplete_provider(provider, emitter)?
            .is_none(),
    ];
    emitter.emit("editor-optionals", json!(absent))
}

/// Builds an editor, installs callbacks and uses it before returning its retained component.
fn factory(ui: ExtensionUIContext) -> ComponentFactory {
    let provider = super::completion_author::provider(ui);
    Rc::new(move |_, _, emitter| {
        let editor: Rc<dyn EditorComponent> = Rc::new(Plain::default());
        emitter.emit("editor-text", json!(editor.get_text(emitter)?))?;
        editor.set_on_change(
            Some(Rc::new(|text, emitter| {
                emitter.emit("editor-change", json!(text))
            })),
            emitter,
        )?;
        editor.set_on_submit(
            Some(Rc::new(|text, emitter| {
                emitter.emit("editor-submit", json!(text))
            })),
            emitter,
        )?;
        editor.set_text("λ\n雪", emitter)?;
        EditorComponent::handle_input(editor.as_ref(), "!", emitter)?;
        EditorComponent::handle_input(editor.as_ref(), "enter", emitter)?;
        emitter.emit(
            "editor-callback-present",
            json!(editor.on_change(emitter)?.is_some()),
        )?;
        optional(editor.as_ref(), Rc::clone(&provider), emitter)?;
        emitter.emit("editor-text", json!(editor.get_text(emitter)?))?;
        editor.set_on_change(None, emitter)?;
        editor.set_on_submit(None, emitter)?;
        emitter.emit(
            "editor-callback-present",
            json!(editor.on_change(emitter)?.is_some()),
        )?;
        editor.set_text("after", emitter)?;
        EditorComponent::handle_input(editor.as_ref(), "enter", emitter)?;
        Ok(ExtensionComponent {
            component: Component::new(View(editor)),
            dispose: None,
            set_expanded: None,
            width: None,
        })
    })
}

/// Shows the authored editor through the existing component factory transport.
pub fn install(ui: &ExtensionUIContext) -> ExtensionResult<()> {
    ui.set_widget_factory("editor-contract", factory(ui.clone()), None)
}

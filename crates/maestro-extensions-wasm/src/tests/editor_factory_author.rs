//! Editor factories that build an authored editor, the host's editor and an authored editor on
//! the application's custom editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    CallbackEmitter, ComponentView, CustomEditor, CustomEditorComponent, Editor, EditorComponent,
    EditorFactory, EditorFactoryOutcome, EditorOptions, ExtensionResult, ExtensionUIContext, Style,
    TextHandler,
};
use serde_json::{Value, json};

/// An editor with only the required operations: `enter` submits, other input is appended.
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
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let mut rows = vec![format!("plain|{width}")];
        rows.extend(self.text.borrow().split('\n').map(str::to_owned));
        Ok(rows)
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
        let text = self.text.borrow().clone();
        if data != "enter" {
            return self.set_text(&format!("{text}{data}"), emitter);
        }
        let handler = self.submit.borrow().clone();
        handler.map_or(Ok(()), |handler| handler(&text, emitter))
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.submit.replace(handler);
        Ok(())
    }
    fn on_change(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        Ok(self.change.borrow().clone())
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.change.replace(handler);
        Ok(())
    }
}

/// An editor on the application's custom editor that keeps `x` for itself and passes other
/// input on, as a modal editor does.
struct Modal(CustomEditor);

impl ComponentView for Modal {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        ComponentView::render(&self.0, width, emitter)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::invalidate(&self.0, emitter)
    }
}

impl EditorComponent for Modal {
    fn get_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<String> {
        EditorComponent::get_text(&self.0, emitter)
    }
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::set_text(&self.0, text, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if data == "x" {
            return emitter.emit("modal-kept", json!(data));
        }
        EditorComponent::handle_input(&self.0, data, emitter)
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_submit(&self.0, handler, emitter)
    }
    fn on_change(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        EditorComponent::on_change(&self.0, emitter)
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_change(&self.0, handler, emitter)
    }
    fn border_color(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        EditorComponent::border_color(&self.0, emitter)
    }
    fn set_padding_x(
        &self,
        padding: f64,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_padding_x(&self.0, padding, emitter)
    }
}

impl CustomEditorComponent for Modal {
    fn custom_editor(&self) -> &CustomEditor {
        &self.0
    }
}

/// A factory building the editor `kind` names; `wrap` builds with the current factory.
fn factory(ui: &ExtensionUIContext, kind: &str) -> ExtensionResult<EditorFactory> {
    Ok(match kind {
        "wrap" => {
            let current = ui
                .get_editor_component()?
                .ok_or("no editor factory to wrap")?;
            Rc::new(move |tui, theme, keybindings, emitter| {
                current(tui, theme, keybindings, emitter)
            })
        }
        "keys" => Rc::new(|_, _, keys, _| {
            let editor = Plain::default();
            editor.submit.replace(Some(Rc::new(move |_, emitter| {
                let before = keys.get_keys("app.demo")?;
                let mut snapshot = keys.get_effective_config()?;
                let saved = super::widget_author::bindings_json(&snapshot);
                snapshot.clear();
                let owned =
                    saved == super::widget_author::bindings_json(&keys.get_effective_config()?);
                keys.reload()?;
                emitter.emit("factory-keys", json!({
                    "before": before, "after": keys.get_keys("app.demo")?,
                    "snapshot": saved, "snapshotOwned": owned,
                    "effective": super::widget_author::bindings_json(&keys.get_effective_config()?),
                    "user": super::widget_author::bindings_json(&keys.get_user_bindings()?),
                }))
            })));
            Ok(EditorFactoryOutcome::Editor(Rc::new(editor)))
        }),
        "plain" => {
            Rc::new(|_, _, _, _| Ok(EditorFactoryOutcome::Editor(Rc::new(Plain::default()))))
        }
        "host" => Rc::new(|tui, theme, _, _| {
            let editor = Editor::new(&tui, &theme, EditorOptions::default())?;
            Ok(EditorFactoryOutcome::Editor(Rc::new(editor)))
        }),
        "custom" => Rc::new(|tui, theme, keybindings, _| {
            let editor = CustomEditor::new(&tui, &theme, &keybindings, EditorOptions::default())?;
            editor.on_action(
                "app.demo",
                Rc::new(|emitter| emitter.emit("custom-action", json!("app.demo"))),
            )?;
            Ok(EditorFactoryOutcome::CustomEditor(Rc::new(Modal(editor))))
        }),
        "failing" => Rc::new(|_, _, _, _| Err("editor factory failed 雪\n".to_owned())),
        _ => return Err(format!("unknown editor {kind}")),
    })
}

/// Installs the editor `kind` names, or restores the host's editor for `none`, and reports
/// whether the host now answers the same factory.
pub fn install(ui: &ExtensionUIContext, kind: &str) -> ExtensionResult<Value> {
    if kind == "none" {
        ui.set_editor_component(None)?;
        return Ok(json!({ "current": ui.get_editor_component()?.is_some() }));
    }
    let factory = factory(ui, kind)?;
    ui.set_editor_component(Some(Rc::clone(&factory)))?;
    let current = ui.get_editor_component()?;
    Ok(json!({ "same": current.is_some_and(|current| Rc::ptr_eq(&current, &factory)) }))
}

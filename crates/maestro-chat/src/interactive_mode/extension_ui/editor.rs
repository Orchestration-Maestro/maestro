//! The extension editor in place of the default editor, and the current editor the mode's
//! text, history, appearance and completion operations reach.
use super::super::InteractiveMode;
use crate::CustomEditor;
use crate::components::extension_widgets::keybindings;
use maestro_app::{CustomEditorComponent, EditorFactory, EventError};
use maestro_theme::get_editor_theme;
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{AutocompleteProvider, Cancellation, EditorComponent};
use std::{cell::RefCell, rc::Rc};

/// The completion provider the mode installs on its editors.
pub(in super::super) type EditorAutocomplete = Rc<dyn AutocompleteProvider<Signal = Cancellation>>;

/// The current editor, the extension factory that built it and the retained completion
/// provider.
pub(in super::super) struct EditorState {
    /// The editor the mode's text, history, appearance and completion operations reach.
    current: RefCell<Rc<dyn EditorComponent<Signal = Cancellation>>>,
    /// The extension editor factory, when one is set.
    factory: RefCell<Option<EditorFactory>>,
    /// The completion provider last installed.
    pub(in super::super) autocomplete: RefCell<Option<EditorAutocomplete>>,
}

impl EditorState {
    /// Starts with `editor` current and no factory or provider.
    pub(in super::super) fn new(editor: Rc<dyn EditorComponent<Signal = Cancellation>>) -> Self {
        Self {
            current: RefCell::new(editor),
            factory: RefCell::default(),
            autocomplete: RefCell::default(),
        }
    }
}

/// Reads one optional application callback of the default editor.
type ReadAction = fn(&CustomEditor) -> Option<Rc<dyn Fn()>>;

impl InteractiveMode {
    /// The editor the mode's text, history, appearance and completion operations reach.
    pub(in super::super) fn current_editor(
        &self,
    ) -> Rc<dyn EditorComponent<Signal = Cancellation>> {
        Rc::clone(&self.editor_state.current.borrow())
    }

    /// Whether an extension editor is current instead of the default editor.
    pub(in super::super) fn custom_editor_current(&self) -> bool {
        !std::ptr::addr_eq(Rc::as_ptr(&self.current_editor()), Rc::as_ptr(&self.editor))
    }

    /// The extension editor factory, when one is set.
    pub(super) fn editor_factory(&self) -> Option<EditorFactory> {
        self.editor_state.factory.borrow().clone()
    }

    /// The current editor's expanded text, or its raw text when it does not expand.
    ///
    /// # Errors
    /// Returns the editor's original failure.
    pub(in super::super) fn expanded_editor_text(&self) -> Result<String, EventError> {
        match self.current_editor().try_get_expanded_text()? {
            Some(text) => Ok(text),
            None => self.current_editor().try_get_text(),
        }
    }

    /// Keeps `factory`, then replaces the current editor with the editor it builds over the
    /// current raw text, the default editor's callbacks, border, padding and completion and,
    /// for an application-capable editor, its actions; `None` returns that text to the
    /// default editor. The installed editor takes focus and a frame is requested.
    ///
    /// # Errors
    /// Returns the original text, factory or editor failure, keeping the effects before it.
    pub(super) fn set_custom_editor_component(
        &self,
        factory: Option<EditorFactory>,
    ) -> Result<(), EventError> {
        drop(self.editor_state.factory.replace(factory.clone()));
        let text = self.current_editor().try_get_text()?;
        self.editor_container.clear();
        let editor = if let Some(factory) = factory {
            let theme = get_editor_theme(self.themes.theme());
            let built = factory.call((
                self.ui.as_ref().clone(),
                theme,
                keybindings(self.keybindings.clone()),
            ))?;
            let editor = built.editor();
            editor.set_on_submit(self.editor.on_submit());
            editor.set_on_change(self.editor.on_change());
            editor.try_set_text(&text)?;
            if editor.border_color().is_some() {
                editor.set_border_color(maestro_tui::Editor::border_color(&self.editor));
            }
            editor.try_set_padding_x(self.editor.get_padding_x())?;
            let provider = self.editor_state.autocomplete.borrow().clone();
            if let Some(provider) = provider {
                editor.try_set_autocomplete_provider(provider)?;
            }
            if let Some(custom) = built.as_custom() {
                self.copy_editor_handlers(custom);
            }
            editor
        } else {
            self.editor.try_set_text(&text)?;
            self.editor.clone()
        };
        drop(self.editor_state.current.replace(Rc::clone(&editor)));
        self.editor_container.add_child(editor.clone());
        self.ui.set_focus(Some(editor as ComponentHandle));
        self.ui.request_render(false);
        Ok(())
    }

    /// Gives `next` the default editor's application actions in their order, and a
    /// forwarding callback for each special callback it does not supply itself.
    fn copy_editor_handlers(&self, next: &dyn CustomEditorComponent) {
        if next.on_escape().is_none() {
            next.set_on_escape(Some(forward(self.editor.clone(), CustomEditor::on_escape)));
        }
        if next.on_ctrl_d().is_none() {
            next.set_on_ctrl_d(Some(forward(self.editor.clone(), CustomEditor::on_ctrl_d)));
        }
        if next.on_paste_image().is_none() {
            let read = CustomEditor::on_paste_image;
            next.set_on_paste_image(Some(forward(self.editor.clone(), read)));
        }
        if next.on_extension_shortcut().is_none() {
            let editor = self.editor.clone();
            next.set_on_extension_shortcut(Some(Rc::new(move |data: &str| {
                editor
                    .on_extension_shortcut()
                    .is_some_and(|handler| handler(data))
            })));
        }
        let actions: Vec<_> = self
            .editor
            .action_handlers()
            .borrow()
            .iter()
            .map(|(name, handler)| (name.clone(), Rc::clone(handler)))
            .collect();
        let target = next.action_handlers();
        for (name, handler) in actions {
            let replaced = target.borrow_mut().insert(name, handler);
            drop(replaced);
        }
    }
}

/// A callback that calls the default editor's callback current when it runs, if any.
fn forward(editor: Rc<CustomEditor>, read: ReadAction) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if let Some(callback) = read(&editor) {
            callback();
        }
    })
}

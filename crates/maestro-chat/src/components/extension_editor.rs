//! Multiline extension input with inherited-terminal external editing.
use super::dynamic_border::DynamicBorder;
use crate::external_editor::{ExternalEditorFile, edit_external_file, external_command};
use maestro_theme::{LiveTheme, ThemeColor, ThemeError};
use maestro_tui::editor_component::TextCallback;
use maestro_tui::tui::{InputHandler, TuiRuntime};
use maestro_tui::{
    Component, Container, Editor, EditorOptions, FocusFlag, Focusable, KeybindingsManager, Spacer,
    TUI, Text,
};
use std::{
    io,
    rc::{Rc, Weak},
};

/// Existing editor constructor inputs with the caller's style and host owners.
pub struct ExtensionEditorOptions {
    /// Initial text; empty strings leave the new editor untouched.
    pub prefill: Option<String>,
    /// Receives submitted text after the editor clears its buffer.
    pub on_submit: TextCallback,
    /// Receives cancellation without changing the buffer.
    pub on_cancel: Rc<dyn Fn()>,
    /// Options forwarded directly into the editor constructor.
    pub editor_options: EditorOptions,
    /// Published style slot for live borders and prepared title/help.
    pub theme: LiveTheme,
    /// Host environment, wall clock and error delivery.
    pub runtime: Rc<dyn TuiRuntime>,
}
/// Bordered multiline input retaining its editor independently of visible children.
pub struct ExtensionEditorComponent {
    /// Shared visible child collection.
    container: Container,
    /// Retained input target.
    editor: Rc<Editor>,
    /// Independent parent state forwarding every setter to the editor.
    focus: FocusFlag,
    /// Caller cancellation notification.
    on_cancel: Rc<dyn Fn()>,
    /// Live application shortcuts.
    keybindings: KeybindingsManager,
    /// Non-owning containing writer.
    tui: Weak<TUI>,
    /// Retained host effects.
    runtime: Rc<dyn TuiRuntime>,
}
impl ExtensionEditorComponent {
    /// Constructs the dialog, preparing title/help but retaining live border colors.
    ///
    /// # Errors
    /// Returns a reached title or hint theme error.
    pub fn new(
        tui: &Rc<TUI>,
        keybindings: KeybindingsManager,
        title: String,
        options: ExtensionEditorOptions,
    ) -> Result<Self, ThemeError> {
        let container = Container::new();
        let border = || Rc::new(DynamicBorder::new(options.theme.clone(), None));
        container.add_child(border());
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(Rc::new(Text::new(
            options.theme.get()?.fg(&ThemeColor::Accent, &title)?,
            1,
            0,
            None,
        )));
        drop(title);
        container.add_child(Rc::new(Spacer::new(1)));
        let editor = Rc::new(Editor::new(
            tui,
            maestro_theme::get_editor_theme(options.theme.clone()),
            options.editor_options,
        ));
        if let Some(prefill) = options.prefill.filter(|text| !text.is_empty()) {
            editor.set_text(&prefill);
        }
        editor.set_on_submit(Some(options.on_submit));
        container.add_child(editor.clone());
        container.add_child(Rc::new(Spacer::new(1)));
        let hint = editor_hint(&options.theme, options.runtime.as_ref())?;
        container.add_child(Rc::new(Text::new(hint, 1, 0, None)));
        container.add_child(Rc::new(Spacer::new(1)));
        container.add_child(border());
        Ok(Self {
            container,
            focus: FocusFlag::forwarding(editor.clone()),
            editor,
            on_cancel: options.on_cancel,
            keybindings,
            tui: Rc::downgrade(tui),
            runtime: options.runtime,
        })
    }
    /// Routes cancellation before application shortcuts and ordinary editor input.
    ///
    /// # Errors
    /// Returns filesystem, terminal or process-bridge errors from external editing.
    pub fn handle_input(&self, data: &str) -> io::Result<()> {
        if maestro_tui::get_keybindings().matches(data, "tui.select.cancel") {
            (self.on_cancel)();
        } else if self.keybindings.matches(data, "app.editor.external") {
            self.open_external_editor()?;
        } else {
            self.editor.handle_input(data);
        }
        Ok(())
    }
    /// Edits the buffer in the configured external editor through the shared edit owner.
    fn open_external_editor(&self) -> io::Result<()> {
        let Some(command) = external_command(self.runtime.as_ref()) else {
            return Ok(());
        };
        let Some(tui) = self.tui.upgrade() else {
            return Ok(());
        };
        let path = std::env::temp_dir().join(format!(
            "maestro-extension-editor-{}.md",
            self.runtime.log_context().unix_ms
        ));
        let file = ExternalEditorFile {
            command: &command,
            path: &path,
            text: &self.editor.get_text(),
        };
        edit_external_file(
            &tui,
            #[cfg(windows)]
            self.runtime.as_ref(),
            |text| {
                self.editor.set_text(text);
                Ok::<_, io::Error>(())
            },
            &file,
        )
    }
}
component_children!(ExtensionEditorComponent, Container, container);
impl Focusable for ExtensionEditorComponent {
    fn focus_flag(&self) -> &FocusFlag {
        &self.focus
    }
}
impl InputHandler for ExtensionEditorComponent {
    fn handle_input(&self, data: &str) {
        if let Err(error) = Self::handle_input(self, data) {
            self.runtime.spawn_local(Box::pin(async move {
                Err(Box::new(error) as Box<dyn std::error::Error>)
            }));
        }
    }
}
impl Component for ExtensionEditorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.container.render(width)
    }
    fn invalidate(&self) {
        self.container.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}

/// Prepares the constructor-time help using the global registry.
fn editor_hint(theme: &LiveTheme, runtime: &dyn TuiRuntime) -> Result<String, ThemeError> {
    let mut hints = vec![
        ("tui.select.confirm", "submit"),
        ("tui.input.newLine", "newline"),
        ("tui.select.cancel", "cancel"),
    ];
    if external_command(runtime).is_some() {
        hints.push(("app.editor.external", "external editor"));
    }
    Ok(hints
        .into_iter()
        .map(|(action, label)| maestro_app::key_hint(action, label, theme))
        .collect::<Result<Vec<_>, _>>()?
        .join("  "))
}

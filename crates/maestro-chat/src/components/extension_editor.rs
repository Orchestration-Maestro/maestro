//! Multiline extension input with inherited-terminal external editing.
use super::dynamic_border::DynamicBorder;
use maestro_theme::{LiveTheme, ThemeColor, ThemeError};
use maestro_tui::editor_component::TextCallback;
use maestro_tui::tui::{InputHandler, TuiRuntime};
use maestro_tui::{
    Component, Container, Editor, EditorOptions, FocusFlag, Focusable, KeybindingsManager, Spacer,
    TUI, Text,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
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
    /// Creates an owned edit file and restores the terminal after every reached body outcome.
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
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut owned = false;
        let result = (|| {
            let mut file = options.open(&path)?;
            owned = true;
            file.write_all(self.editor.get_text().as_bytes())?;
            drop(file);
            tui.stop()?;
            self.edit_file(&command, &path)
        })();
        if owned {
            let _ = fs::remove_file(&path);
        }
        tui.start()?;
        tui.request_render(true);
        result
    }
    /// Replaces the buffer only after a successfully observed zero exit.
    fn edit_file(&self, command: &str, path: &Path) -> io::Result<()> {
        let command = native_command(
            command,
            path,
            #[cfg(windows)]
            self.runtime.as_ref(),
        );
        match command.spawn_sync(maestro_app::SpawnSyncOptions::Inherit) {
            Ok(output) if output.status.success() => {
                let bytes = fs::read(path)?;
                let content = String::from_utf8_lossy(&bytes);
                self.editor
                    .set_text(content.strip_suffix('\n').unwrap_or(&content));
            }
            Ok(_) | Err(maestro_app::SpawnSyncError::Spawn(_)) => {}
            Err(
                maestro_app::SpawnSyncError::Infrastructure(error)
                | maestro_app::SpawnSyncError::Wait(error),
            ) => return Err(error),
            Err(maestro_app::SpawnSyncError::Timeout) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "external editor timed out",
                ));
            }
        }
        Ok(())
    }
}
/// Samples the current nonempty visual editor, falling back to the editor variable.
fn external_command(runtime: &dyn TuiRuntime) -> Option<String> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|key| runtime.environment(key).filter(|value| !value.is_empty()))
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

/// Builds the launch with the path last; only non-Windows commands split on ASCII spaces.
fn native_command(
    command: &str,
    path: &Path,
    #[cfg(windows)] runtime: &dyn TuiRuntime,
) -> maestro_app::LaunchContext<'static, Vec<(&'static str, String)>> {
    #[cfg(not(windows))]
    {
        let mut tokens = command.split(' ');
        maestro_app::Launch {
            executable: tokens.next().unwrap_or_default().into(),
            args: tokens.map(str::to_owned).collect(),
            verbatim: false,
        }
        .with_context(vec![], None)
        .arg(path)
    }
    #[cfg(windows)]
    {
        let (launch, env) = windows_command(
            command,
            &path.to_string_lossy(),
            runtime.environment("comspec").as_deref(),
        );
        launch.with_context(vec![env], None)
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

/// Returns native interpreter arguments while preserving the original command string.
#[cfg(any(windows, test))]
fn windows_command(
    command: &str,
    path: &str,
    shell: Option<&str>,
) -> (maestro_app::Launch, (&'static str, String)) {
    /// Environment operand keeps the filename out of interpreted command text.
    const EDITOR_FILE_ENV: &str = "MAESTRO_EDITOR_FILE";
    let mut launch = maestro_app::windows_shell_launch(command, shell);
    let operand = if launch.verbatim {
        format!(" \"%{EDITOR_FILE_ENV}%\"")
    } else {
        format!(" \"${EDITOR_FILE_ENV}\"")
    };
    if let Some(text) = launch.args.last_mut() {
        text.insert_str(text.len() - usize::from(launch.verbatim), &operand);
    }
    (launch, (EDITOR_FILE_ENV, path.into()))
}
#[cfg(test)]
mod tests;

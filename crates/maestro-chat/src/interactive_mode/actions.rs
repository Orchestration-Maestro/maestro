//! Editor actions: the shell-mode border, thinking and model cycling, external editing
//! and image paste.
use super::InteractiveMode;
use crate::components::thinking_selector::thinking_text;
use crate::external_editor::{ExternalEditorFile, edit_external_file, external_command};
use crate::temporary_file::write_temporary_file;
use maestro_app::agent_session::{CycleDirection, ModelCycleResult};
use maestro_app::utils::clipboard_image::{
    ClipboardImage, extension_for_image_mime_type, read_clipboard_image,
};
use maestro_app::{DiagnosticErrorInfo, ThinkingLevel, is_whitespace_scalar};
use maestro_tui::Component;
use std::{error::Error, io, rc::Rc, sync::PoisonError};

impl InteractiveMode {
    /// Installs the editor's change callback and image paste without keeping the mode
    /// alive: text whose first non-whitespace character is `!` selects the shell border,
    /// and only a change of mode repaints it.
    pub(super) fn setup_editor_callbacks(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_change(Some(Rc::new(move |text| {
            let Some(mode) = mode.upgrade() else {
                return;
            };
            let shell = text
                .trim_start_matches(is_whitespace_scalar)
                .starts_with('!');
            if mode.is_bash_mode.replace(shell) != shell {
                mode.report(mode.update_editor_border_color().map_err(Into::into));
            }
        })));
        let mode = Rc::downgrade(self);
        self.editor.set_on_paste_image(Some(Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.handle_clipboard_image_paste();
            }
        })));
    }

    /// Binds model cycling in `direction` to `action` without retaining the mode; the
    /// started cycle keeps the mode until it completes on the terminal runtime.
    pub(super) fn on_model_cycle(self: &Rc<Self>, action: &str, direction: CycleDirection) {
        let mode = Rc::downgrade(self);
        self.editor.on_action(
            action.to_owned(),
            Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    let runtime = Rc::clone(&mode.tool_environment.runtime);
                    runtime.spawn_local(Box::pin(mode.cycle_model(direction)));
                }
            }),
        );
    }

    /// Selects the session's next thinking level and shows it, refreshing the footer and
    /// border, or says the model cannot reason.
    ///
    /// # Errors
    /// Returns the session's recording failure or the published theme's failure.
    pub(super) fn cycle_thinking_level(&self) -> Result<(), Box<dyn Error>> {
        let Some(level) = self.runtime_host.session().cycle_thinking_level()? else {
            return self.show_status("Current model does not support thinking");
        };
        self.footer.invalidate();
        self.update_editor_border_color()?;
        self.show_status(&format!("Thinking level: {}", thinking_text(&level).0))
    }

    /// Selects the session's next or previous model and shows the outcome.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    async fn cycle_model(self: Rc<Self>, direction: CycleDirection) -> Result<(), Box<dyn Error>> {
        let result = self.runtime_host.session().cycle_model(direction).await;
        self.show_model_cycle_result(result)
    }

    /// Shows a finished model cycle: the selected model and its thinking level after
    /// refreshing the footer and border, why nothing changed, or the session's error.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    pub(super) fn show_model_cycle_result(
        &self,
        result: Result<Option<ModelCycleResult>, DiagnosticErrorInfo>,
    ) -> Result<(), Box<dyn Error>> {
        let result = match result {
            Ok(Some(result)) => result,
            Ok(None) if self.runtime_host.session().scoped_models().is_empty() => {
                return self.show_status("Only one model available");
            }
            Ok(None) => return self.show_status("Only one model in scope"),
            Err(error) => return Ok(self.show_error(&error.message)?),
        };
        self.footer.invalidate();
        self.update_editor_border_color()?;
        let model = result.model.read().unwrap_or_else(PoisonError::into_inner);
        let name = if model.name.is_empty() {
            &model.id
        } else {
            &model.name
        };
        let thinking = if model.reasoning && result.thinking_level != ThinkingLevel::Off {
            format!(" (thinking: {})", thinking_text(&result.thinking_level).0)
        } else {
            String::new()
        };
        self.show_status(&format!("Switched to {name}{thinking}"))
    }

    /// Edits the expanded editor text in the configured external editor, or warns when
    /// neither `VISUAL` nor `EDITOR` names one.
    ///
    /// # Errors
    /// Returns the warning's theme failure or the edit's file, terminal or process failure.
    pub(super) fn open_external_editor(&self) -> Result<(), Box<dyn Error>> {
        let Some(command) = external_command(self.tool_environment.runtime.as_ref()) else {
            return Ok(self.show_warning(
                "No editor configured. Set $VISUAL or $EDITOR environment variable.",
            )?);
        };
        let path = std::env::temp_dir().join(format!(
            "maestro-editor-{}.maestro.md",
            self.tool_environment.runtime.log_context().unix_ms
        ));
        let file = ExternalEditorFile {
            command: &command,
            path: &path,
            text: &self.editor.get_expanded_text(),
        };
        edit_external_file(
            &self.ui,
            #[cfg(windows)]
            self.tool_environment.runtime.as_ref(),
            &self.editor,
            &file,
        )?;
        Ok(())
    }

    /// Suspends the terminal to the background until the shell continues the process.
    ///
    /// Interrupts are ignored from now until the continuation, which restarts the terminal
    /// with a full redraw on the terminal runtime. When stopping the terminal or the
    /// process group fails, interrupts take their default action again and no later
    /// continuation restarts the terminal.
    ///
    /// # Errors
    /// Returns the native signal registration, terminal or signalling failure.
    #[cfg(unix)]
    pub(super) fn handle_ctrl_z(&self) -> Result<(), Box<dyn Error>> {
        let interrupts = ignore_interrupts()?;
        let suspended = maestro_app::continued().and_then(|continued| {
            self.ui.stop()?;
            maestro_app::stop_process_group()?;
            Ok(continued)
        });
        let continued = match suspended {
            Ok(continued) => continued,
            Err(error) => {
                interrupts.restore();
                return Err(error.into());
            }
        };
        let ui = self.ui.clone();
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                continued.await;
                interrupts.restore();
                ui.start()?;
                ui.request_render(true);
                Ok(())
            }));
        Ok(())
    }

    /// Says this host cannot suspend the terminal to the background.
    ///
    /// # Errors
    /// Returns the published theme's failure.
    #[cfg(any(windows, test))]
    pub(super) fn show_suspend_unsupported(&self) -> Result<(), Box<dyn Error>> {
        self.show_status("Suspend to background is not supported on Windows")
    }

    /// Reads the clipboard image on the host, keeping the mode until the read completes,
    /// then pastes it.
    fn handle_clipboard_image_paste(self: Rc<Self>) {
        let host = Rc::clone(&self.tool_environment.runtime);
        host.spawn_local(Box::pin(async move {
            self.paste_clipboard_result(read_clipboard_image(None).await);
            Ok(())
        }));
    }

    /// Writes a read clipboard image to a new private temporary file and inserts its path
    /// at the cursor; no image and every failure leave the editor untouched.
    pub(super) fn paste_clipboard_result(&self, result: io::Result<Option<ClipboardImage>>) {
        let Ok(Some(image)) = result else {
            return;
        };
        let extension = extension_for_image_mime_type(&image.mime_type).unwrap_or("png");
        let path = std::env::temp_dir().join(format!(
            "maestro-clipboard-{}.{extension}",
            uuid::Uuid::new_v4()
        ));
        if write_temporary_file(&path, &image.bytes).is_ok() {
            self.editor.insert_text_at_cursor(&path.to_string_lossy());
            self.ui.request_render(false);
        }
    }
}

/// The interrupt's default action, prepared once for the process at its first suspension,
/// so a later suspension cannot leave an earlier preparation taking the default action
/// first.
#[cfg(unix)]
static INTERRUPT_ACTIONS: std::sync::Mutex<Option<maestro_app::DefaultSignalActions>> =
    std::sync::Mutex::new(None);

/// Makes interrupts do nothing until the returned actions are restored.
///
/// # Errors
/// Returns the native handler registration failure.
#[cfg(unix)]
fn ignore_interrupts() -> io::Result<maestro_app::DefaultSignalActions> {
    let mut prepared = INTERRUPT_ACTIONS
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(actions) = prepared.as_ref() {
        actions.defer();
        return Ok(actions.clone());
    }
    let interrupt = [maestro_app::TerminationSignal::Interrupt];
    Ok(prepared
        .insert(maestro_app::DefaultSignalActions::new(&interrupt)?)
        .clone())
}

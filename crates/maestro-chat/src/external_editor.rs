//! Editing text in the user's external editor on the inherited terminal.
use crate::temporary_file::write_temporary_file;
use maestro_tui::{TUI, tui::TuiRuntime};
use std::{fs, io, path::Path};

/// One edit: the configured command, the file it edits and the text written there first.
pub(crate) struct ExternalEditorFile<'a> {
    /// The configured editor command.
    pub command: &'a str,
    /// The temporary file created for the edit.
    pub path: &'a Path,
    /// The text the file starts with.
    pub text: &'a str,
}

/// Samples the current nonempty visual editor, falling back to the editor variable,
/// without trimming or interpreting either value.
pub(crate) fn external_command(runtime: &dyn TuiRuntime) -> Option<String> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|key| runtime.environment(key).filter(|value| !value.is_empty()))
}

/// Writes the file, stops the terminal and runs the editor on it; a zero exit hands the
/// file's text, less one final line feed, to `set_text`. After every reached outcome the
/// created file's removal is attempted, a failed removal ignored, and the terminal
/// restarted with a full redraw.
///
/// # Errors
/// Returns the file creation, terminal, read, process-bridge or `set_text` failure; a
/// nonzero exit, a signal or a refused spawn leave the text unchanged without an error.
pub(crate) fn edit_external_file<E: From<io::Error>>(
    ui: &TUI,
    #[cfg(windows)] runtime: &dyn TuiRuntime,
    set_text: impl FnOnce(&str) -> Result<(), E>,
    file: &ExternalEditorFile<'_>,
) -> Result<(), E> {
    let mut owned = false;
    let result = (|| -> Result<(), E> {
        write_temporary_file(file.path, file.text.as_bytes())?;
        owned = true;
        ui.stop()?;
        let command = native_command(
            file.command,
            file.path,
            #[cfg(windows)]
            runtime,
        );
        match command.spawn_sync(maestro_app::SpawnSyncOptions::Inherit) {
            Ok(output) if output.status.success() => {
                let bytes = fs::read(file.path)?;
                let text = String::from_utf8_lossy(&bytes);
                set_text(text.strip_suffix('\n').unwrap_or(&text))?;
            }
            Ok(_) | Err(maestro_app::SpawnSyncError::Spawn(_)) => {}
            Err(
                maestro_app::SpawnSyncError::Infrastructure(error)
                | maestro_app::SpawnSyncError::Wait(error),
            ) => return Err(error.into()),
            Err(maestro_app::SpawnSyncError::Timeout) => {
                return Err(
                    io::Error::new(io::ErrorKind::TimedOut, "external editor timed out").into(),
                );
            }
        }
        Ok(())
    })();
    if owned {
        // Cleanup failures are ignored; the edit's own outcome is the one reported.
        let _ = fs::remove_file(file.path);
    }
    ui.start()?;
    ui.request_render(true);
    result
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

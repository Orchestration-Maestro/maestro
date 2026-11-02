//! Session transfer commands, their path argument and native export preparation.

use super::InteractiveMode;
use maestro_app::configuration::config::{
    APP_NAME, get_package_dir, installation::Installation, paths,
};
use maestro_app::{ExtensionUIContextPort, NativeExportOperations, is_whitespace_scalar};
use std::{error::Error, path::Path, rc::Rc};

impl InteractiveMode {
    /// Writes the current session as JSONL for an exact `.jsonl` path and as HTML
    /// otherwise, then reports the path the export wrote or the recoverable failure.
    ///
    /// # Errors
    /// Returns the failure of showing that report.
    pub(super) fn handle_export_command(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let output_path = get_path_command_argument(text, "/export");
        let exported = if output_path.is_some_and(selects_jsonl) {
            self.runtime_host
                .session()
                .export_to_jsonl(output_path)
                .map_err(Into::into)
        } else {
            self.export_to_html(output_path)
        };
        match exported {
            Ok(path) => self
                .show_status(&format!("Session exported to: {path}"))
                .map_err(Into::into),
            Err(error) => Ok(self.show_error(&format!("Failed to export session: {error}"))?),
        }
    }

    /// Exports the session as HTML through the native bundle of the selected package
    /// layout and the current theme, with the environment the transcript renders through.
    ///
    /// # Errors
    /// Returns the package-directory, installation or export failure.
    pub(super) fn export_to_html(
        &self,
        output_path: Option<&str>,
    ) -> Result<String, Box<dyn Error>> {
        let template_dir = paths::get_export_template_dir(
            &get_package_dir()?,
            Installation::current()?.layout,
            |path| Path::new(path).exists(),
        );
        let operations = NativeExportOperations {
            app_name: APP_NAME,
            template_dir: &template_dir,
            theme: &self.themes,
        };
        Ok(self.runtime_host.session().export_to_html(
            output_path,
            &operations,
            Rc::clone(&self.tool_environment),
        )?)
    }
}

/// Whether `path` carries the exact lower-case JSONL suffix; any other spelling exports
/// HTML, so the comparison stays case-sensitive.
pub(super) fn selects_jsonl(path: &str) -> bool {
    path.strip_suffix("jsonl")
        .is_some_and(|prefix| prefix.ends_with('.'))
}

/// Reads the path argument after `command`: the first bare token, or the text between the
/// first quote pair, without unescaping, expanding or normalizing it.
pub(super) fn get_path_command_argument<'a>(text: &'a str, command: &str) -> Option<&'a str> {
    let arguments = text
        .strip_prefix(command)?
        .strip_prefix(' ')?
        .trim_start_matches(is_whitespace_scalar);
    match arguments.chars().next()? {
        quote @ ('\'' | '"') => arguments
            .strip_prefix(quote)?
            .split_once(quote)
            .map(|(path, _)| path),
        _ => arguments.split(is_whitespace_scalar).next(),
    }
}

impl InteractiveMode {
    /// Confirms importing an authored path, clears progress, replaces the session and
    /// reports the result, asking once for the offered directory when it is missing.
    ///
    /// # Errors
    /// Returns a dialog or notice failure outside the first attempt, or any retry failure.
    pub(super) async fn handle_import_command(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let Some(path) = get_path_command_argument(text, "/import").filter(|path| !path.is_empty())
        else {
            return Ok(self.show_error("Usage: /import <path.jsonl>")?);
        };
        if !self
            .confirm(
                "Import session".to_owned(),
                format!("Replace current session with {path}?"),
                None,
            )
            .await
            .map_err(|error| -> Box<dyn Error> { error })?
        {
            return Ok(self.show_status("Import cancelled")?);
        }
        self.clear_status_indicator(None);
        match self.runtime_host.import_from_jsonl(path, None).await {
            Ok(result) => {
                if let Err(error) = self.show_import_result(path, result) {
                    self.handle_fatal_runtime_error("Failed to import session", error.as_ref());
                }
                Ok(())
            }
            Err(maestro_app::AgentSessionRuntimeError::MissingSessionCwd(error)) => {
                let Some(cwd) = self
                    .prompt_for_missing_session_cwd(&error)
                    .await
                    .map_err(|error| -> Box<dyn Error> { error })?
                    .filter(|cwd| !cwd.is_empty())
                else {
                    return Ok(self.show_status("Import cancelled")?);
                };
                let result = self
                    .runtime_host
                    .import_from_jsonl(path, Some(&cwd))
                    .await?;
                self.show_import_result(path, result)
            }
            Err(maestro_app::AgentSessionRuntimeError::SessionImportFileNotFound(error)) => {
                Ok(self.show_error(&format!("Failed to import session: {error}"))?)
            }
            Err(error) => self.handle_fatal_runtime_error("Failed to import session", &error),
        }
    }

    /// Reports an import veto or replays the replacement and names the authored path.
    ///
    /// # Errors
    /// Returns a replay or notice failure without changing its caller's catch boundary.
    fn show_import_result(
        &self,
        path: &str,
        result: maestro_app::SessionChangeResult,
    ) -> Result<(), Box<dyn Error>> {
        if result.cancelled {
            return Ok(self.show_status("Import cancelled")?);
        }
        self.render_current_session_state()
            .map_err(|error| -> Box<dyn Error> { error })?;
        Ok(self.show_status(&format!("Session imported from: {path}"))?)
    }
}

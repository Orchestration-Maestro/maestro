//! The export command, its path argument and the native bundle preparation.

use super::InteractiveMode;
use maestro_app::configuration::config::{
    APP_NAME, get_package_dir, installation::Installation, paths,
};
use maestro_app::{NativeExportOperations, is_whitespace_scalar};
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
            Ok(path) => self.show_status(&format!("Session exported to: {path}")),
            Err(error) => self.show_error(&format!("Failed to export session: {error}")),
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

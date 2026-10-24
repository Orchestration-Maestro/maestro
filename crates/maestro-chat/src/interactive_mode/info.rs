//! The session name and session information commands.
use super::InteractiveMode;
use maestro_app::is_whitespace_scalar;
use maestro_theme::ThemeColor;
use std::error::Error;

impl InteractiveMode {
    /// Shows the current session name for `/name` alone, or a usage warning without one;
    /// otherwise records the supplied name, trimmed, and confirms it.
    ///
    /// # Errors
    /// Returns the failure to read or record the name, or of the published theme's colours.
    pub fn handle_name_command(&self, text: &str) -> Result<(), Box<dyn Error>> {
        let name = text
            .strip_prefix("/name")
            .unwrap_or(text)
            .trim_matches(is_whitespace_scalar);
        let session = self.runtime_host.session();
        if name.is_empty() {
            return match session.session_name()? {
                Some(current) => {
                    Ok(self.notice(&ThemeColor::Dim, &format!("Session name: {current}"))?)
                }
                None => Ok(self.show_warning("Usage: /name <name>")?),
            };
        }
        session.set_session_name(name)?;
        Ok(self.notice(&ThemeColor::Dim, &format!("Session name set: {name}"))?)
    }
}

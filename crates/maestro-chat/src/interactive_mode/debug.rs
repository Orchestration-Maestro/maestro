//! The debug capture command and its global key.
use super::InteractiveMode;
use maestro_app::configuration::config::{get_agent_dir, paths::get_debug_log_path};
use maestro_app::{AgentSession, serialize_json_line};
use maestro_theme::ThemeColor;
use maestro_tui::{Component, visible_width};
use std::{error::Error, fmt::Write as _, path::Path, rc::Rc, sync::PoisonError};

impl InteractiveMode {
    /// Renders the presentation at the terminal's width, then writes the terminal size,
    /// each rendered line with its visible width and escaped text, and each session
    /// message as one JSON line to the debug log, and shows the log's path.
    ///
    /// # Errors
    /// Returns the failure to render, to find or write the log, to serialize a message, or
    /// of the published theme's colours; a failed write adds no notice.
    pub fn handle_debug_command(&self) -> Result<(), Box<dyn Error>> {
        let size = {
            let terminal = self.ui.terminal().borrow();
            (terminal.columns(), terminal.rows())
        };
        let lines = self.ui.try_render(size.0)?;
        let path = get_debug_log_path(&get_agent_dir()?);
        let time = self.tool_environment.runtime.log_context().iso_time;
        let data = debug_data(&time, size, &lines, &self.runtime_host.session())?;
        self.tool_environment
            .runtime
            .write_log(Path::new(&path), &data)?;
        let theme = self.themes.theme().get()?;
        let notice = format!(
            "{}\n{}",
            theme.fg(&ThemeColor::Accent, "✓ Debug log written")?,
            theme.fg(&ThemeColor::Muted, &path)?
        );
        self.add_text(notice, 1);
        Ok(())
    }

    /// Runs the debug capture when the toolkit's global debug key is pressed, whatever has
    /// focus, without keeping the mode alive; a failure goes to the terminal host.
    pub(super) fn setup_debug_handler(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.ui.set_on_debug(Some(Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.debug_from_key();
            }
        })));
    }

    /// Runs the debug capture for the global key, whose callback cannot return a failure,
    /// so the terminal host reports it.
    fn debug_from_key(&self) {
        if let Err(error) = self.handle_debug_command() {
            self.report(Err(error));
        }
    }
}

/// The debug log text: header, every rendered line, then every session message.
///
/// # Errors
/// Returns the failure to serialize a line or a message.
fn debug_data(
    time: &str,
    (width, height): (usize, usize),
    lines: &[String],
    session: &AgentSession,
) -> Result<String, Box<dyn Error>> {
    let mut data = format!(
        "Debug output at {time}\nTerminal: {width}x{height}\nTotal lines: {}\n\n\
         === All rendered lines with visible widths ===\n",
        lines.len()
    );
    for (index, line) in lines.iter().enumerate() {
        let escaped = serde_json::to_string(line)?;
        writeln!(data, "[{index}] (w={}) {escaped}", visible_width(line))?;
    }
    data += "\n=== Agent messages (JSONL) ===\n";
    let messages = session.messages();
    let messages = messages.read().unwrap_or_else(PoisonError::into_inner);
    for message in messages.iter() {
        data += &serialize_json_line(&serde_json::to_value(message)?)?;
    }
    Ok(data)
}

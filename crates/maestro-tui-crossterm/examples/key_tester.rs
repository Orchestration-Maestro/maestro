//! Displays terminal input bytes, Unicode scalars and escaped control characters.

#[path = "support/process_terminal.rs"]
mod process_terminal;

/// Native key display and process lifetime.
#[cfg(unix)]
mod unix {
    use maestro_tui::{
        Component, TUI, TruncateOptions, keys::matches_key, truncate_to_width, tui::InputHandler,
    };
    use std::{cell::RefCell, collections::VecDeque, io, process::ExitCode, rc::Rc};
    use tokio::sync::Notify;

    /// Arrival-ordered input with the writer and its exit notification.
    struct KeyLogger {
        /// At most twenty complete observations, never clipped in storage.
        log: RefCell<VecDeque<String>>,
        /// The writer whose references are cleared before this program returns.
        tui: TUI,
        /// Notifies the main future when the cancellation key arrives.
        quit: Rc<Notify>,
    }

    impl InputHandler for KeyLogger {
        fn handle_input(&self, data: &str) {
            if matches_key(data, "ctrl+c") {
                self.quit.notify_one();
                return;
            }
            let hex = data.bytes().fold(String::new(), |mut text, byte| {
                use std::fmt::Write;
                let _ = write!(text, "{byte:02x}");
                text
            });
            let codes = data
                .chars()
                .map(|scalar| u32::from(scalar).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let repr = data
                .replace('\x1b', "\\x1b")
                .replace('\r', "\\r")
                .replace('\n', "\\n")
                .replace('\t', "\\t")
                .replace('\x7f', "\\x7f");
            let mut log = self.log.borrow_mut();
            log.push_back(format!(
                "Hex: {hex:20} | Chars: [{codes:15}] | Repr: \"{repr}\""
            ));
            if log.len() > 20 {
                log.pop_front();
            }
            drop(log);
            self.tui.request_render(false);
        }
    }

    impl Component for KeyLogger {
        fn input_handler(&self) -> Option<&dyn InputHandler> {
            Some(self)
        }
        fn render(&self, width: usize) -> Vec<String> {
            let ruler = "=".repeat(width);
            let mut lines = vec![
                ruler.clone(),
                "Key Code Tester - Press keys to see their codes (Ctrl+C to exit)".into(),
                ruler.clone(),
                String::new(),
            ];
            lines.extend(self.log.borrow().iter().cloned());
            lines.resize(25, " ".repeat(width));
            lines.extend([
                ruler.clone(),
                "Test these:".into(),
                "  - Shift + Enter (should show: \\x1b[13;2u with Kitty protocol)".into(),
                "  - Alt/Option + Enter".into(),
                "  - Option/Alt + Backspace".into(),
                "  - Cmd/Ctrl + Backspace".into(),
                "  - Regular Backspace".into(),
                ruler,
            ]);
            lines
                .into_iter()
                .map(|line| clip_row(line, width))
                .collect()
        }
    }

    /// Leaves separators empty and clips other rows without changing their stored text.
    fn clip_row(line: String, width: usize) -> String {
        if line.is_empty() {
            return line;
        }
        truncate_to_width(
            &line,
            width,
            TruncateOptions {
                ellipsis: "",
                pad: true,
            },
        )
    }

    /// Displays input until the cancellation key or SIGINT, then prints the exit line.
    pub(super) fn main() -> io::Result<ExitCode> {
        use std::io::Write;
        let status = super::process_terminal::run(async |tui, _| {
            let quit = Rc::new(Notify::new());
            let logger = Rc::new(KeyLogger {
                log: RefCell::default(),
                tui: tui.clone(),
                quit: Rc::clone(&quit),
            });
            tui.add_child(logger.clone());
            tui.set_focus(Some(logger));
            tui.start()?;
            tokio::select! { () = quit.notified() => {}, signal = tokio::signal::ctrl_c() => { signal?; } }
            Ok(ExitCode::SUCCESS)
        })?;
        writeln!(io::stdout(), "\nExiting...")?;
        Ok(status)
    }
}

/// Starts the native key display.
#[cfg(unix)]
fn main() -> std::io::Result<std::process::ExitCode> {
    unix::main()
}

/// Reports the pending native adapter on unsupported platforms.
#[cfg(not(unix))]
fn main() -> std::io::Result<std::process::ExitCode> {
    process_terminal::unsupported()
}

//! Streams text across a small terminal viewport and into its history.

#[path = "support/process_terminal.rs"]
mod process_terminal;

/// Native viewport stream and its retained rows.
#[cfg(unix)]
mod unix {
    use maestro_tui::{Component, TUI, TruncateOptions, truncate_to_width};
    use std::{cell::RefCell, io, process::ExitCode, rc::Rc, time::Duration};

    /// Arrival-ordered text rows.
    #[derive(Default)]
    struct Lines {
        /// Complete rows, clipped only while rendering.
        lines: RefCell<Vec<String>>,
    }

    impl Lines {
        /// Replaces the stored rows.
        fn set(&self, lines: Vec<String>) {
            self.lines.replace(lines);
        }
        /// Appends rows in caller order.
        fn append(&self, lines: Vec<String>) {
            self.lines.borrow_mut().extend(lines);
        }
    }

    impl Component for Lines {
        fn render(&self, width: usize) -> Vec<String> {
            self.lines
                .borrow()
                .iter()
                .map(|line| {
                    truncate_to_width(
                        line,
                        width,
                        TruncateOptions {
                            ellipsis: "",
                            pad: true,
                        },
                    )
                })
                .collect()
        }
    }

    /// Appends numbered rows, requesting a frame before each delay.
    async fn stream_lines(buffer: &Lines, label: &str, count: usize, delay_ms: u64, ui: &TUI) {
        for index in 1..=count {
            buffer.append(vec![format!("{label} {index:02}")]);
            ui.request_render(false);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    /// Runs the three streams using the height sampled at startup.
    fn run() -> io::Result<ExitCode> {
        super::process_terminal::run(async |ui, _| {
            let buffer = Rc::new(Lines::default());
            ui.add_child(buffer.clone());
            ui.start()?;
            let height = ui.terminal().borrow().rows();
            buffer.set(vec![
                "TUI viewport overwrite repro".into(),
                format!("Viewport rows detected: {height}"),
                "(Resize to ~8-12 rows for best repro)".into(),
                String::new(),
                "=== PRE-TOOL STREAM ===".into(),
            ]);
            ui.request_render(false);
            tokio::time::sleep(Duration::from_millis(300)).await;
            stream_lines(&buffer, "PRE-TOOL LINE", height + 8, 30, &ui).await;
            buffer.append(vec![
                String::new(),
                "--- TOOL CALL START ---".into(),
                "(pause...)".into(),
                String::new(),
            ]);
            ui.request_render(false);
            tokio::time::sleep(Duration::from_millis(700)).await;
            stream_lines(&buffer, "TOOL OUT", height + 12, 20, &ui).await;
            buffer.append(vec![String::new(), "=== POST-TOOL STREAM ===".into()]);
            ui.request_render(false);
            tokio::time::sleep(Duration::from_millis(300)).await;
            stream_lines(&buffer, "POST-TOOL LINE", 6, 40, &ui).await;
            tokio::time::sleep(Duration::from_millis(1500)).await;
            Ok(ExitCode::SUCCESS)
        })
    }

    /// Reports a stream failure after the original terminal is cleaned up.
    pub(super) fn main() -> io::Result<ExitCode> {
        use std::io::Write;
        match run() {
            Ok(status) => Ok(status),
            Err(error) => {
                writeln!(io::stderr(), "{error}")?;
                Ok(ExitCode::FAILURE)
            }
        }
    }
}

/// Starts the native viewport demonstration.
#[cfg(unix)]
fn main() -> std::io::Result<std::process::ExitCode> {
    unix::main()
}

/// Reports the pending native adapter on unsupported platforms.
#[cfg(not(unix))]
fn main() -> std::io::Result<std::process::ExitCode> {
    process_terminal::unsupported()
}

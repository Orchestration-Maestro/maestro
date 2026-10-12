//! Concrete native composition shared by the terminal demonstrations.

use std::{io, process::ExitCode};

/// Runs a native demonstration and attempts original-terminal cleanup after it returns.
#[cfg(unix)]
pub(super) fn run<F, Fut>(body: F) -> io::Result<ExitCode>
where
    F: FnOnce(maestro_tui::TUI, maestro_tui::TerminalImage) -> Fut,
    Fut: Future<Output = io::Result<ExitCode>>,
{
    use maestro_tui::{TUI, TerminalImage};
    use maestro_tui_crossterm::{ProcessTerminal, ProcessTuiRuntime};
    use std::{cell::RefCell, rc::Rc};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local = Rc::new(tokio::task::LocalSet::new());
    let images = TerminalImage::default();
    let tui = TUI::new(
        Rc::new(RefCell::new(ProcessTerminal::new(Rc::clone(&local)))),
        Rc::new(ProcessTuiRuntime::new(Rc::clone(&local))),
        images.clone(),
        None,
    );
    runtime.block_on(local.run_until(async {
        let result = body(tui.clone(), images).await;
        let stopped = tui.stop();
        if stopped.is_err() {
            let _ = tui.terminal().borrow_mut().stop();
        }
        tui.set_focus(None);
        tui.clear();
        result.and_then(|status| stopped.map(|()| status))
    }))
}

/// Reports the unavailable native terminal adapter.
#[cfg(not(unix))]
pub(super) fn unsupported() -> io::Result<ExitCode> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "The Windows terminal adapter is not implemented.",
    ))
}

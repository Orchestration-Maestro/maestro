//! The frame writer one presented selector runs in.

use maestro_theme::ThemeState;
use maestro_tui::TUI;
#[cfg(any(unix, windows))]
use maestro_tui::TerminalImage;
#[cfg(any(unix, windows))]
use maestro_tui::tui::{TerminalHandle, TuiRuntime};
use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};
use tokio::sync::oneshot;

/// Construct the native terminal and its runtime over `local` without starting either.
#[cfg(any(unix, windows))]
#[must_use]
pub fn native_terminal(local: Rc<tokio::task::LocalSet>) -> (TerminalHandle, Rc<dyn TuiRuntime>) {
    let terminal: TerminalHandle = Rc::new(RefCell::new(
        maestro_tui_crossterm::ProcessTerminal::new(Rc::clone(&local)),
    ));
    let runtime = Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(local));
    (terminal, runtime)
}

/// The frame writer and the exit effect one presentation supplies.
#[derive(Clone)]
pub struct SelectorHost {
    /// Creates the frame writer the presented component is added to.
    pub create_ui: Rc<dyn Fn() -> io::Result<TUI>>,
    /// Ends the process after the explicit-exit notification.
    pub exit: Rc<dyn Fn(i32)>,
}

impl SelectorHost {
    /// Run on the native terminal of the supplied local set.
    ///
    /// `local` drives the deferred terminal work; the terminal itself is created
    /// when the presentation asks for its frame writer.
    #[cfg(any(unix, windows))]
    #[must_use]
    pub fn native(local: Rc<tokio::task::LocalSet>) -> Self {
        Self {
            create_ui: Rc::new(move || {
                let (terminal, runtime) = native_terminal(Rc::clone(&local));
                Ok(TUI::new(terminal, runtime, TerminalImage::default(), None))
            }),
            exit: Rc::new(|code: i32| std::process::exit(code)),
        }
    }
}

/// One running presentation: its frame writer, theme watch and single settlement.
///
/// Notifications keep a weak handle, so a settled or dropped presentation owns no
/// callback cycle. Dropping the owner stops whatever it already started, so a
/// cancelled presentation, a failed presentation and a settled one share one cleanup.
pub(crate) struct Running<T> {
    /// The writer the presented component was added to, once it is created.
    ui: RefCell<Option<TUI>>,
    /// The theme state whose watch close and exit stop.
    theme: Option<Rc<ThemeState>>,
    /// Whether the caller's settlement already ran.
    settled: Cell<bool>,
    /// Whether the started effects were already stopped.
    stopped: Cell<bool>,
    /// Delivers the caller's result once.
    outcome: RefCell<Option<oneshot::Sender<T>>>,
}

impl<T> Running<T> {
    /// Retain the theme this presentation owns and return the result receiver.
    pub(crate) fn new(theme: Option<Rc<ThemeState>>) -> (Rc<Self>, oneshot::Receiver<T>) {
        let (sender, receiver) = oneshot::channel();
        let running = Rc::new(Self {
            ui: RefCell::default(),
            theme,
            settled: Cell::new(false),
            stopped: Cell::new(false),
            outcome: RefCell::new(Some(sender)),
        });
        (running, receiver)
    }

    /// Retain the frame writer once the presentation has created it.
    pub(crate) fn attach(&self, ui: TUI) {
        let previous = self.ui.replace(Some(ui));
        drop(previous);
    }

    /// Stop the frame writer, then the theme watch, as the reference does.
    ///
    /// Stopping twice stops nothing the second time.
    pub(crate) fn stop(&self) {
        if self.stopped.replace(true) {
            return;
        }
        if let Some(ui) = self.ui.borrow().as_ref() {
            let _ = ui.stop();
        }
        if let Some(theme) = &self.theme {
            theme.stop_theme_watcher();
        }
    }

    /// Stop once, then deliver `value`; a second settlement does nothing.
    pub(crate) fn settle(self: &Rc<Self>, value: T) {
        if self.settled.replace(true) {
            return;
        }
        self.stop();
        if let Some(sender) = self.outcome.borrow_mut().take() {
            let _ = sender.send(value);
        }
    }
}

impl<T> Drop for Running<T> {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build a notification that settles the presentation with `value`.
pub(crate) fn settlement<T: Clone + 'static>(running: &Rc<Running<T>>, value: T) -> Rc<dyn Fn()> {
    let running = Rc::downgrade(running);
    Rc::new(move || {
        if let Some(running) = running.upgrade() {
            running.settle(value.clone());
        }
    })
}

/// Build the explicit-exit notification: stop, then end the process.
pub(crate) fn exit_notification<T: 'static>(
    running: &Rc<Running<T>>,
    exit: Rc<dyn Fn(i32)>,
) -> Rc<dyn Fn()> {
    let running: Weak<Running<T>> = Rc::downgrade(running);
    Rc::new(move || {
        if let Some(running) = running.upgrade() {
            running.stop();
        }
        exit(0);
    })
}

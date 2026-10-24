//! Observes scheduling at the existing host boundary.
use super::manual_runtime::{ManualRuntime, ms};
use maestro_tui::interface::{RenderCallback, RenderTimer, TuiRuntime};
use std::{cell::Cell, rc::Rc, time::Duration};
/// Counts only the animation continuations and coalesced writer requests.
#[derive(Default)]
pub struct Observer {
    /// Controlled underlying host.
    pub clock: ManualRuntime,
    /// Pending animation callbacks.
    pub timers: Rc<Cell<usize>>,
    /// Writer scheduling requests.
    pub redraws: Cell<usize>,
}
/// Cancellation accounting for one owned animation continuation.
struct Timer {
    /// Existing cancellation handle.
    inner: Box<dyn RenderTimer>,
    /// Shared pending count.
    count: Rc<Cell<usize>>,
    /// Whether this continuation is still pending.
    active: Rc<Cell<bool>>,
}
impl RenderTimer for Timer {
    fn cancel(&mut self) {
        if self.active.replace(false) {
            self.count.set(self.count.get() - 1);
        }
        self.inner.cancel();
    }
}
impl TuiRuntime for Observer {
    fn now(&self) -> Duration {
        self.clock.now()
    }
    fn schedule(&self, delay: Duration, callback: RenderCallback) -> Box<dyn RenderTimer> {
        if delay != ms(80) {
            self.redraws.set(self.redraws.get() + 1);
            return self.clock.schedule(delay, callback);
        }
        self.timers.set(self.timers.get() + 1);
        let active = Rc::new(Cell::new(true));
        let fired = active.clone();
        let count = self.timers.clone();
        let inner = self.clock.schedule(
            delay,
            Box::new(move || {
                fired.set(false);
                count.set(count.get() - 1);
                callback()
            }),
        );
        Box::new(Timer {
            inner,
            count: self.timers.clone(),
            active,
        })
    }
    fn spawn_local(&self, future: maestro_tui::interface::LocalFuture) {
        self.clock.spawn_local(future);
    }
    fn environment(&self, key: &str) -> Option<String> {
        self.clock.environment(key)
    }
    fn log_context(&self) -> maestro_tui::interface::LogContext {
        self.clock.log_context()
    }
    fn append_log(&self, path: &std::path::Path, contents: &str) -> std::io::Result<()> {
        self.clock.append_log(path, contents)
    }
    fn write_log(&self, path: &std::path::Path, contents: &str) -> std::io::Result<()> {
        self.clock.write_log(path, contents)
    }
}

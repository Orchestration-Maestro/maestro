//! Scheduling observation shared by component witnesses.
use super::manual_runtime::{ManualRuntime, ms};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// A scheduling observer delegating every host effect to the existing manual runtime.
#[derive(Clone, Default)]
pub struct Spy {
    /// Existing clock and scheduler.
    pub clock: ManualRuntime,
    /// Countdown callbacks and writer requests in invocation order.
    pub events: Rc<RefCell<Vec<Value>>>,
    /// Installed countdown delays.
    pub delays: Rc<RefCell<Vec<u64>>>,
    /// Pending countdown work only, not pending frames.
    pub pending: Rc<Cell<usize>>,
}
/// A counted countdown continuation.
struct SpyTimer {
    /// Original cancellation handle.
    timer: Box<dyn maestro_tui::interface::RenderTimer>,
    /// Whether this callback is still pending.
    active: Rc<Cell<bool>>,
    /// Number of pending countdown callbacks.
    pending: Rc<Cell<usize>>,
}
impl maestro_tui::interface::RenderTimer for SpyTimer {
    fn cancel(&mut self) {
        if self.active.replace(false) {
            self.pending.set(self.pending.get() - 1);
        }
        self.timer.cancel();
    }
}
impl maestro_tui::interface::TuiRuntime for Spy {
    fn now(&self) -> std::time::Duration {
        maestro_tui::interface::TuiRuntime::now(&self.clock)
    }
    fn schedule(
        &self,
        delay: std::time::Duration,
        callback: maestro_tui::interface::RenderCallback,
    ) -> Box<dyn maestro_tui::interface::RenderTimer> {
        if delay != ms(1000) {
            self.events.borrow_mut().push(serde_json::json!(["render"]));
            return self.clock.schedule(delay, callback);
        }
        self.delays.borrow_mut().push(1000);
        self.pending.set(self.pending.get() + 1);
        let active = Rc::new(Cell::new(true));
        let fired = active.clone();
        let pending = self.pending.clone();
        let timer = self.clock.schedule(
            delay,
            Box::new(move || {
                fired.set(false);
                pending.set(pending.get() - 1);
                callback()
            }),
        );
        Box::new(SpyTimer {
            timer,
            active,
            pending: self.pending.clone(),
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

//! One owned continuation for repeated runtime callbacks.
use super::{RenderTimer, TuiRuntime};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Repeats after each interval until this handle is dropped.
pub struct RepeatingTimer(Rc<State>);
/// Scheduled work retains this state only weakly.
struct State {
    /// Host supplying deferred callbacks.
    runtime: Rc<dyn TuiRuntime>,
    /// Delay before each invocation.
    interval: Duration,
    /// Caller notification, invoked without a state borrow.
    callback: Rc<dyn Fn()>,
    /// Pending continuation revoked on owner drop.
    timer: RefCell<Option<Box<dyn RenderTimer>>>,
}
impl RepeatingTimer {
    /// Schedules the first callback after `interval`, never inline.
    #[must_use]
    pub fn new(runtime: Rc<dyn TuiRuntime>, interval: Duration, callback: Rc<dyn Fn()>) -> Self {
        let state = Rc::new(State {
            runtime,
            interval,
            callback,
            timer: RefCell::default(),
        });
        state.arm();
        Self(state)
    }
}
impl State {
    /// Rearms before notifying so reentrant teardown cancels the next invocation.
    fn arm(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let timer = self.runtime.schedule(
            self.interval,
            Box::new(move || {
                if let Some(state) = weak.upgrade() {
                    state.arm();
                    (state.callback)();
                }
                Ok(())
            }),
        );
        let previous = self.timer.replace(Some(timer));
        drop(previous);
    }
}
impl Drop for RepeatingTimer {
    fn drop(&mut self) {
        let timer = self.0.timer.take();
        if let Some(mut timer) = timer {
            timer.cancel();
        }
    }
}

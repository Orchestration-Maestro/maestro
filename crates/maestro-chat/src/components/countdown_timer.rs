//! Whole-second countdown policy over the toolkit runtime.
use maestro_tui::TUI;
use maestro_tui::interface::{RepeatingTimer, TuiRuntime};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// A countdown whose last owner cancels its pending continuation.
pub struct CountdownTimer(Rc<State>);
/// Countdown state retained only weakly by scheduled work.
struct State {
    /// Current ceiling in seconds.
    remaining: Cell<f64>,
    /// Optional render request that does not retain its writer.
    request: Option<Rc<dyn Fn()>>,
    /// Existing scheduling owner.
    runtime: Rc<dyn TuiRuntime>,
    /// Count notification.
    on_tick: Rc<dyn Fn(f64)>,
    /// Expiration notification.
    on_expire: Rc<dyn Fn()>,
    /// Owned continuation.
    timer: RefCell<Option<RepeatingTimer>>,
}
impl CountdownTimer {
    /// Reports the initial ceiling immediately, then schedules one-second ticks.
    #[must_use]
    pub fn new(
        timeout_ms: f64,
        tui: Option<&TUI>,
        runtime: Rc<dyn TuiRuntime>,
        on_tick: Rc<dyn Fn(f64)>,
        on_expire: Rc<dyn Fn()>,
    ) -> Self {
        let state = Rc::new(State {
            remaining: Cell::new((timeout_ms / 1000.0).ceil()),
            request: tui.map(TUI::weak_render_request),
            runtime,
            on_tick,
            on_expire,
            timer: RefCell::default(),
        });
        (state.on_tick)(state.remaining.get());
        state.arm();
        Self(state)
    }
    /// Cancels future countdown work; callbacks retained by this owner remain installed.
    pub fn dispose(&self) {
        self.0.stop();
    }
}
impl State {
    /// Arms before notifying so a callback can dispose the next tick.
    fn arm(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let timer = RepeatingTimer::new(
            Rc::clone(&self.runtime),
            Duration::from_secs(1),
            Rc::new(move || {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                let remaining = state.remaining.get() - 1.0;
                state.remaining.set(remaining);
                (state.on_tick)(remaining);
                if let Some(request) = &state.request {
                    request();
                }
                if remaining <= 0.0 {
                    state.stop();
                    (state.on_expire)();
                }
            }),
        );
        let previous = self.timer.replace(Some(timer));
        drop(previous);
    }
    /// Shares explicit and last-owner teardown.
    fn stop(&self) {
        let timer = self.timer.take();
        drop(timer);
    }
}

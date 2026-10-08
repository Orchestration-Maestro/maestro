//! Coalescing of render requests under a minimum interval.

use std::io;
use std::rc::Rc;
use std::time::Duration;

use super::{RenderTimer, TUI};

/// The least time between two ordinary frames.
const MIN_RENDER_INTERVAL: Duration = Duration::from_millis(16);

/// Whether a frame is wanted and when the next one is due.
#[derive(Default)]
pub(super) struct Schedule {
    /// Whether the writer has been stopped.
    stopped: bool,
    /// Whether a frame has been requested and not yet started.
    render_requested: bool,
    /// The pending delayed frame.
    render_timer: Option<Box<dyn RenderTimer>>,
    /// The pending immediate frame a forced request queued.
    forced_timer: Option<Box<dyn RenderTimer>>,
    /// When the latest frame started.
    last_render_at: Duration,
}

impl Schedule {
    /// Records a request for a frame; false when the writer is stopped or, unless
    /// `force` is set, a frame is already requested.
    fn request(&mut self, force: bool) -> bool {
        if self.stopped || (self.render_requested && !force) {
            return false;
        }
        self.render_requested = true;
        true
    }

    /// Allows frames again.
    pub(super) fn restart(&mut self) {
        self.stopped = false;
    }

    /// Forbids frames, forgets the request and hands back the pending timers to cancel.
    pub(super) fn halt(&mut self) -> [Option<Box<dyn RenderTimer>>; 2] {
        self.stopped = true;
        self.render_requested = false;
        [self.render_timer.take(), self.forced_timer.take()]
    }
}

impl TUI {
    /// Asks for a frame. Ordinary requests coalesce under a minimum interval; a forced
    /// request forgets the retained frame and draws from scratch at once.
    pub fn request_render(&self, force: bool) {
        if force {
            self.request_forced_render();
        } else if self.shared.schedule.borrow_mut().request(false) {
            self.schedule_render();
        }
    }

    /// Forgets the retained frame and queues an immediate frame in place of any pending
    /// frame.
    fn request_forced_render(&self) {
        self.shared.screen.borrow_mut().forget_frame();
        let (stale, queued) = {
            let mut schedule = self.shared.schedule.borrow_mut();
            (
                [schedule.render_timer.take(), schedule.forced_timer.take()],
                schedule.request(true),
            )
        };
        for mut timer in stale.into_iter().flatten() {
            timer.cancel();
        }
        if queued {
            let weak = Rc::downgrade(&self.shared);
            let timer = self.shared.runtime.schedule(
                Duration::ZERO,
                Box::new(move || match weak.upgrade() {
                    Some(shared) => TUI { shared }.run_forced_render(),
                    None => Ok(()),
                }),
            );
            self.shared.schedule.borrow_mut().forced_timer = Some(timer);
        }
    }

    /// Queues a delayed frame unless one is queued.
    fn schedule_render(&self) {
        let now = self.shared.runtime.now();
        let delay = {
            let schedule = self.shared.schedule.borrow();
            if schedule.render_timer.is_some() {
                return;
            }
            MIN_RENDER_INTERVAL.saturating_sub(now.saturating_sub(schedule.last_render_at))
        };
        let weak = Rc::downgrade(&self.shared);
        let timer = self.shared.runtime.schedule(
            delay,
            Box::new(move || match weak.upgrade() {
                Some(shared) => TUI { shared }.run_scheduled_render(),
                None => Ok(()),
            }),
        );
        self.shared.schedule.borrow_mut().render_timer = Some(timer);
    }

    /// Claims the pending request for a frame that starts now; false when none is pending.
    fn claim_frame(&self) -> bool {
        let now = self.shared.runtime.now();
        let mut schedule = self.shared.schedule.borrow_mut();
        if schedule.stopped || !schedule.render_requested {
            return false;
        }
        schedule.render_requested = false;
        schedule.last_render_at = now;
        true
    }

    /// Runs the frame a delayed request was waiting for, then queues another when a
    /// component requested one meanwhile.
    fn run_scheduled_render(&self) -> io::Result<()> {
        let expired = self.shared.schedule.borrow_mut().render_timer.take();
        drop(expired);
        if !self.claim_frame() {
            return Ok(());
        }
        self.do_render()?;
        if self.shared.schedule.borrow().render_requested {
            self.schedule_render();
        }
        Ok(())
    }

    /// Runs the immediate frame a forced request queued.
    fn run_forced_render(&self) -> io::Result<()> {
        let expired = self.shared.schedule.borrow_mut().forced_timer.take();
        drop(expired);
        if self.claim_frame() {
            self.do_render()
        } else {
            Ok(())
        }
    }
}

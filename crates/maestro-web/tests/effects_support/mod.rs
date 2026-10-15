//! Controlled native-clock policy at the shared content-host seam.
#![cfg(test)]
use maestro_theme::BrandPresentation;
use maestro_web::{
    Attachment,
    browser::{attachment_content::*, timer::Timer},
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
/// Local owned work supplied to the adapter.
type Work = Pin<Box<dyn Future<Output = ()>>>;
/// One controlled producer registration.
struct Entry {
    /// Next due instant.
    due: i32,
    /// Repeat period, absent for one-shot callbacks.
    interval: Option<i32>,
    /// Owned delivery callback.
    callback: Rc<dyn Fn()>,
}
/// Deterministic clock; no toolkit frame or wall clock is involved.
#[derive(Default)]
pub struct Host {
    /// Current instant.
    now: Cell<i32>,
    /// Stable registration slots, revoked by the real Timer owner.
    entries: Rc<RefCell<Vec<Option<Entry>>>>,
    /// Total admissions, including revoked timers.
    pub admissions: Cell<usize>,
    /// Owned non-Send work.
    work: RefCell<Vec<Work>>,
    /// Next registration failure.
    pub failure: RefCell<Option<AttachmentUiError>>,
}
impl Host {
    /// Exhaust every due delivery, releasing the registry borrow before calling out.
    pub fn advance(&self, to: i32) {
        self.now.set(to);
        loop {
            let next = self.next_due(to);
            let Some(callback) = next else { break };
            callback();
        }
    }
    /// Admit the next due operation without running caller code under the borrow.
    fn next_due(&self, to: i32) -> Option<Rc<dyn Fn()>> {
        let mut entries = self.entries.borrow_mut();
        entries.iter_mut().find_map(|slot| {
            let entry = slot.as_mut()?;
            if entry.due > to {
                return None;
            }
            let callback = entry.callback.clone();
            if let Some(period) = entry.interval {
                entry.due += period;
            } else {
                *slot = None;
            }
            Some(callback)
        })
    }
    /// Poll queued work without cancelling it when an unrelated timer closes.
    pub fn run_work(&self) {
        let work = self.work.take();
        for mut future in work {
            assert!(matches!(
                future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(())
            ));
        }
    }
    /// Count live native registrations, not historical admissions.
    pub fn live(&self) -> usize {
        self.entries
            .borrow()
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }
}
impl AttachmentHost for Host {
    fn timer(
        &self,
        delay: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<Timer, AttachmentUiError> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        self.admissions.set(self.admissions.get() + 1);
        let index = self.entries.borrow().len();
        self.entries.borrow_mut().push(Some(Entry {
            due: self.now.get() + delay,
            interval: repeat.then_some(delay),
            callback,
        }));
        let entries = self.entries.clone();
        Ok(Timer::new(move || {
            let removed = entries.borrow_mut()[index].take();
            drop(removed);
        }))
    }
    fn spawn_local(&self, work: Work) {
        self.work.borrow_mut().push(work);
    }
    fn present(
        &self,
        _: AttachmentSurface<'_>,
        _: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        self.failure.take().map_or(Ok(()), Err)
    }
    fn take_escape(&self) -> bool {
        false
    }
    fn remove(&self, _: egui::Id) {}
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        Ok(false)
    }
    fn download(&self, _: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
}

//! Controlled native-clock policy at the shared content-host seam.
#![cfg(test)]
use maestro_theme::BrandPresentation;
use maestro_web::browser::{attachment_content::*, timer::Timer};
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
    /// Original byte download operands.
    pub downloads: RefCell<Vec<(String, String, Vec<u8>)>>,
    /// Exact clipboard admissions.
    pub copies: RefCell<Vec<String>>,
    /// Selected clipboard policy for each admission.
    pub modes: RefCell<Vec<ClipboardMode>>,
    /// Controlled native measurements keyed by the placed surface.
    layouts: RefCell<std::collections::HashMap<egui::Id, Vec<(egui::Id, egui::Rect)>>>,
    /// Published canvas control rectangles for pointer input.
    pub controls: RefCell<Vec<egui::Rect>>,
    /// Surface owners released by retained components.
    pub removed: RefCell<Vec<egui::Id>>,
    /// Separately resolved native operations.
    pub results: RefCell<Vec<Rc<Cell<Option<bool>>>>>,
    /// Current instant.
    now: Cell<i32>,
    /// Stable registration slots, revoked by the real Timer owner.
    entries: Rc<RefCell<Vec<Option<Entry>>>>,
    /// Total admissions, including revoked timers.
    pub admissions: Cell<usize>,
    /// Owned non-Send work.
    work: RefCell<Vec<Work>>,
    /// Next registration failure.
    pub failure: RefCell<Option<AttachmentHostError>>,
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
            if matches!(
                future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ) {
                self.work.borrow_mut().push(future);
            }
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
    fn copy_to_clipboard(
        &self,
        text: &str,
        mode: maestro_web::browser::attachment_content::ClipboardMode,
    ) -> Pin<Box<dyn Future<Output = bool>>> {
        self.copies.borrow_mut().push(text.into());
        self.modes.borrow_mut().push(mode);
        let result = Rc::new(Cell::new(None));
        self.results.borrow_mut().push(result.clone());
        Box::pin(std::future::poll_fn(move |_| {
            result.get().map_or(Poll::Pending, Poll::Ready)
        }))
    }

    fn reading_layout(
        &self,
        id: egui::Id,
    ) -> Result<maestro_web::browser::attachment_content::ReadingLayout, AttachmentHostError> {
        Ok(ReadingLayout {
            width: 0.,
            border: egui::epaint::MarginF32::ZERO,
            height: 64.,
            controls: self.layouts.borrow().get(&id).cloned().unwrap(),
        })
    }
    fn timer(
        &self,
        delay: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<Timer, AttachmentHostError> {
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
        surface: AttachmentSurface<'_>,
        _: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        self.failure.take().map_or(Ok(()), Err)?;
        if let AttachmentContent::Reading(reading) = surface.content {
            let controls: Vec<_> = reading
                .sizes
                .iter()
                .map(|(id, size)| (*id, egui::Rect::from_min_size(egui::pos2(100., 4.), *size)))
                .collect();
            if reading.phase == ReadingPhase::Publish {
                *self.controls.borrow_mut() = controls
                    .iter()
                    .map(|(_, rect)| rect.translate(surface.rect.min.to_vec2()))
                    .collect();
            }
            self.layouts.borrow_mut().insert(surface.id, controls);
        }
        Ok(())
    }
    fn remove(&self, id: egui::Id) {
        self.removed.borrow_mut().push(id);
        self.layouts.borrow_mut().remove(&id);
    }
    fn set_retained_visibility(
        &self,
        _: egui::Id,
        _: bool,
        _: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        Ok(false)
    }
    fn download_bytes(
        &self,
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> Result<(), AttachmentHostError> {
        self.downloads
            .borrow_mut()
            .push((name.into(), mime.into(), bytes.to_vec()));
        Ok(())
    }
    fn download(&self, _: maestro_web::SharedAttachment) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
}

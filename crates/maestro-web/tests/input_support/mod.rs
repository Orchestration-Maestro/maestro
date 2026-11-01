//! Controlled observations of the shared native content boundary.
#![cfg(test)]
use maestro_theme::BrandPresentation;
use maestro_web::{browser::attachment_content::*, browser::timer::Timer};
use std::{cell::RefCell, rc::Rc};
/// Presentation observer, called outside any host borrow.
pub type Inspect = dyn Fn(&AttachmentSurface<'_>, &BrandPresentation<'_>);
/// Operands-only conformance adapter, not a native DOM emulator.
pub struct Host {
    /// Observer of each immediate update.
    inspect: Box<Inspect>,
    /// Removed native identities.
    pub removed: RefCell<Vec<egui::Id>>,
    /// Ordered lifecycle effects for controlled dialog callers.
    pub effects: Rc<RefCell<Vec<&'static str>>>,
    /// Next named native effect failure.
    pub failure: RefCell<Option<&'static str>>,
}
impl Host {
    /// Inject a reached failure without replacing the caller's operation.
    fn effect(&self, name: &'static str) -> Result<(), AttachmentHostError> {
        self.effects.borrow_mut().push(name);
        if self.failure.borrow().as_ref() == Some(&name) {
            self.failure.take();
            return Err(AttachmentHostError::Host(name.into()));
        }
        Ok(())
    }
    /// Retain an observer for the public presentation boundary.
    pub fn new(
        inspect: impl Fn(&AttachmentSurface<'_>, &BrandPresentation<'_>) + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            inspect: Box::new(inspect),
            removed: RefCell::default(),
            effects: Rc::default(),
            failure: RefCell::default(),
        })
    }
}
impl AttachmentHost for Host {
    fn copy_to_clipboard(
        &self,
        _: &str,
        _: maestro_web::browser::attachment_content::ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        unreachable!("unexpected clipboard operation")
    }

    fn reading_layout(
        &self,
        _: egui::Id,
    ) -> Result<maestro_web::browser::attachment_content::ReadingLayout, AttachmentHostError> {
        Ok(ReadingLayout {
            width: 0.,
            border: egui::epaint::MarginF32::ZERO,
            height: 300.,
            controls: vec![],
        })
    }
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
        if let AttachmentContent::Reading(placement) = &s.content {
            self.effect(if placement.phase == ReadingPhase::Publish {
                "publish"
            } else {
                "measure"
            })?;
        }
        (self.inspect)(&s, p);
        Ok(())
    }
    fn remove(&self, id: egui::Id) {
        self.removed.borrow_mut().push(id);
        self.effects.borrow_mut().push("remove");
    }
    fn set_retained_visibility(
        &self,
        _: egui::Id,
        _: bool,
        _: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        Ok(())
    }
    fn set_overlay(&self, _: egui::Id, open: bool) -> Result<(), AttachmentHostError> {
        self.effect(if open { "admit" } else { "release" })
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        Ok(false)
    }
    fn download_bytes(&self, _: &[u8], _: &str, _: &str) -> Result<(), AttachmentHostError> {
        unreachable!("unexpected byte download")
    }
    fn download(&self, _: maestro_web::SharedAttachment) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
    fn timer(&self, _: i32, _: bool, _: Rc<dyn Fn()>) -> Result<Timer, AttachmentHostError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
}

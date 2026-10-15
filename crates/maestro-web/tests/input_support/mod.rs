//! Controlled observations of the shared native content boundary.
#![cfg(test)]
use maestro_theme::BrandPresentation;
use maestro_web::{Attachment, Input, browser::attachment_content::*, browser::timer::Timer};
use std::{cell::RefCell, rc::Rc};
/// Presentation observer, called outside any host borrow.
pub type Inspect = dyn Fn(&AttachmentSurface<'_>, &BrandPresentation<'_>);
/// Operands-only conformance adapter, not a native DOM emulator.
pub struct Host {
    /// Observer of each immediate update.
    inspect: Box<Inspect>,
    /// Removed native identities.
    pub removed: RefCell<Vec<egui::Id>>,
}
impl Host {
    /// Retain an observer for the public presentation boundary.
    pub fn new(
        inspect: impl Fn(&AttachmentSurface<'_>, &BrandPresentation<'_>) + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            inspect: Box::new(inspect),
            removed: RefCell::default(),
        })
    }
}
impl AttachmentHost for Host {
    fn reading_layout(
        &self,
        _: egui::Id,
    ) -> Result<maestro_web::browser::attachment_content::ReadingLayout, AttachmentUiError> {
        unreachable!("this suite does not create reading content")
    }
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        (self.inspect)(&s, p);
        Ok(())
    }
    fn remove(&self, id: egui::Id) {
        self.removed.borrow_mut().push(id);
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentUiError> {
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        Ok(false)
    }
    fn download_bytes(&self, _: &[u8], _: &str, _: &str) -> Result<(), AttachmentUiError> {
        unreachable!("unexpected byte download")
    }
    fn download(&self, _: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
    fn take_escape(&self) -> bool {
        false
    }
    fn timer(&self, _: i32, _: bool, _: Rc<dyn Fn()>) -> Result<Timer, AttachmentUiError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
}
/// Render one input in a caller-sized parent, returning actual painted text.
pub fn render(input: &mut Input, width: f32) -> Vec<String> {
    let mut frame = crate::attachment_support::Frame::default();
    frame.run(vec![], |ctx, i18n, presentation| {
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("input-parent"),
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                egui::pos2(10., 20.),
                egui::vec2(width, 400.),
            )),
        );
        input.render(&mut ui, i18n, presentation).unwrap();
    });
    frame.texts()
}

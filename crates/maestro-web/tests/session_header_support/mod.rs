//! Controlled reading boundary and public drawing harness.
#![cfg(test)]
use maestro_web::browser::attachment_content::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
/// Observed native-boundary order, with original identities retained.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Measure(egui::Id),
    Read(egui::Id),
    Publish(egui::Id),
    Remove(egui::Id),
}
/// A controlled failure at the reached boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Measure,
    Read,
    Publish,
}
/// Actual content handles submitted by the public caller.
#[derive(Default)]
pub struct Host {
    /// Ordered operations, never deduplicated or synthesized.
    pub events: RefCell<Vec<Event>>,
    /// Factories observed during admission, including failed publication.
    pub admissions: RefCell<Vec<Rc<ReadingContent>>>,
    /// Published identities and geometry under controlled transaction policy.
    pub published: RefCell<HashMap<egui::Id, (Rc<ReadingContent>, egui::Rect)>>,
    /// Candidate factories not yet published.
    pending: RefCell<HashMap<egui::Id, Rc<ReadingContent>>>,
    /// Native natural height supplied by this host.
    pub height: Cell<f32>,
    /// Selected reached failure, if any.
    pub failure: Cell<Option<Failure>>,
    /// Presentation properties supplied to the boundary.
    pub css: RefCell<Vec<std::collections::BTreeMap<String, String>>>,
}
impl AttachmentHost for Host {
    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        let AttachmentContent::Reading(reading) = s.content else {
            panic!("expected reading content")
        };
        assert!(s.occluders.is_empty());
        self.css.borrow_mut().push(p.css_properties());
        match reading.phase {
            ReadingPhase::Measure => {
                self.events.borrow_mut().push(Event::Measure(s.id));
                self.admissions
                    .borrow_mut()
                    .push(Rc::clone(&reading.content));
                if self.failure.get() == Some(Failure::Measure) {
                    return Err(AttachmentUiError::BorrowConflict);
                }
                self.pending.borrow_mut().insert(s.id, reading.content);
            }
            ReadingPhase::Publish => {
                self.events.borrow_mut().push(Event::Publish(s.id));
                if self.failure.get() == Some(Failure::Publish) {
                    return Err(AttachmentUiError::BorrowConflict);
                }
                let candidate = self.pending.borrow_mut().remove(&s.id).unwrap();
                assert!(Rc::ptr_eq(&candidate, &reading.content));
                self.published
                    .borrow_mut()
                    .insert(s.id, (candidate, s.rect));
            }
        }
        Ok(())
    }
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentUiError> {
        self.events.borrow_mut().push(Event::Read(id));
        if self.failure.get() == Some(Failure::Read) {
            return Err(AttachmentUiError::BorrowConflict);
        }
        Ok(ReadingLayout {
            height: self.height.get(),
        })
    }
    fn remove(&self, id: egui::Id) {
        self.events.borrow_mut().push(Event::Remove(id));
        self.published.borrow_mut().remove(&id);
        self.pending.borrow_mut().remove(&id);
    }
    fn set_overlay(&self, _: egui::Id, _: bool) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        unreachable!()
    }
    fn take_escape(&self) -> bool {
        unreachable!()
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentUiError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn download(&self, _: Rc<RefCell<maestro_web::Attachment>>) -> Result<(), AttachmentUiError> {
        unreachable!()
    }
}
/// Load a real owned offline document without provider work.
pub fn document(id: &serde_json::Value) -> maestro_web::export_html::template::SessionDocument {
    use base64::Engine as _;
    maestro_web::export_html::template::load_document(
        &base64::engine::general_purpose::STANDARD.encode(serde_json::json!({"entries":[],"header":{"id":id},"systemPrompt":{"unread":true},"tools":false}).to_string()), "", None,
    ).unwrap()
}
/// Load the repository pack through its normal operations.
pub fn pack() -> maestro_theme::BrandPack {
    maestro_theme::load_brand_pack(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
        &maestro_theme::theme::NativeThemeOperations,
    )
    .unwrap()
}
/// Render through the public operation in a caller-selected layout.
pub fn draw(
    state: &mut maestro_web::export_html::template::HeaderState,
    doc: &maestro_web::export_html::template::SessionDocument,
    presentation: &maestro_theme::BrandPresentation<'_>,
    layout: egui::Layout,
    size: egui::Vec2,
) -> (std::io::Result<()>, egui::FullOutput) {
    let context = egui::Context::default();
    let mut result = Ok(());
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            ..Default::default()
        },
        |ui| {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .layout(layout)
                    .max_rect(egui::Rect::from_min_size(ui.next_widget_position(), size)),
                |ui| {
                    ui.set_max_size(size);
                    let style = ui.style().clone();
                    result = maestro_web::export_html::template::render_header(
                        ui,
                        doc,
                        state,
                        maestro_web::export_html::template::HeaderView { presentation },
                    );
                    assert_eq!(ui.style().as_ref(), style.as_ref());
                },
            );
        },
    );
    output.textures_delta.clear();
    (result, output)
}

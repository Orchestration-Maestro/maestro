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
    /// Whether the controlled layout exposes native slots.
    pub controls: Cell<bool>,
    /// Current native slot translation in canvas points.
    pub offset: Cell<egui::Vec2>,
    /// Last actual sizes indexed by caller identity.
    pub sizes: RefCell<Vec<(egui::Id, egui::Vec2)>>,
    /// Actual slot identities for each admitted factory.
    pub slot_ids: RefCell<Vec<Vec<egui::Id>>>,
    /// Published transparent control holes.
    pub holes: RefCell<Vec<egui::Rect>>,
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
        self.css.borrow_mut().push(p.css_properties());
        match reading.phase {
            ReadingPhase::Measure => {
                self.events.borrow_mut().push(Event::Measure(s.id));
                *self.sizes.borrow_mut() = reading.sizes.to_vec();
                self.slot_ids
                    .borrow_mut()
                    .push(reading.sizes.iter().map(|(id, _)| *id).collect());
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
                *self.holes.borrow_mut() = s.occluders;
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
            controls: if self.controls.get() {
                self.sizes
                    .borrow()
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, (id, size))| {
                        (
                            *id,
                            egui::Rect::from_min_size(
                                egui::pos2(10. + f32::from(u16::try_from(i).unwrap()) * 150., 5.)
                                    + self.offset.get(),
                                *size,
                            ),
                        )
                    })
                    .collect()
            } else {
                vec![]
            },
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
    fn download_bytes(&self, _: &[u8], _: &str, _: &str) -> Result<(), AttachmentUiError> {
        unreachable!("unexpected byte download")
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
#[cfg(not(target_arch = "wasm32"))]
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
) -> (
    std::io::Result<Vec<maestro_web::export_html::template::HeaderAction>>,
    egui::FullOutput,
) {
    draw_view(
        state,
        doc,
        &maestro_web::export_html::template::HeaderView {
            presentation,
            localize_date: &|_| Ok("native date".into()),
        },
        layout,
        size,
    )
}
/// Forward the caller's date function through the public render boundary.
pub fn draw_view(
    state: &mut maestro_web::export_html::template::HeaderState,
    doc: &maestro_web::export_html::template::SessionDocument,
    view: &maestro_web::export_html::template::HeaderView<'_>,
    layout: egui::Layout,
    size: egui::Vec2,
) -> (
    std::io::Result<Vec<maestro_web::export_html::template::HeaderAction>>,
    egui::FullOutput,
) {
    let context = egui::Context::default();
    let mut result = Ok(vec![]);
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
                        maestro_web::export_html::template::HeaderView {
                            presentation: view.presentation,
                            localize_date: view.localize_date,
                        },
                    );
                    assert_eq!(ui.style().as_ref(), style.as_ref());
                },
            );
        },
    );
    output.textures_delta.clear();
    (result, output)
}

/// Reusable canvas frames for actual control state and pointer observations.
pub struct Canvas<'a> {
    /// Controlled native slots and published holes.
    pub host: Rc<Host>,
    /// Feature lifecycle across frames.
    state: maestro_web::export_html::template::HeaderState,
    /// Retained input/focus state.
    context: egui::Context,
    /// Literal source document.
    document: maestro_web::export_html::template::SessionDocument,
    /// Selected style for every frame.
    presentation: &'a maestro_theme::BrandPresentation<'a>,
}
impl<'a> Canvas<'a> {
    /// Create a full-size canvas with three controlled slots.
    pub fn new(presentation: &'a maestro_theme::BrandPresentation<'a>) -> Self {
        let host = Rc::new(Host::default());
        host.height.set(300.);
        host.controls.set(true);
        Self {
            state: maestro_web::export_html::template::HeaderState::new(host.clone()),
            host,
            context: egui::Context::default(),
            document: document(&serde_json::json!("controls")),
            presentation,
        }
    }
    /// Drive an input frame and retain its complete observable actions and paint.
    pub fn frame(
        &mut self,
        events: Vec<egui::Event>,
    ) -> (
        Vec<maestro_web::export_html::template::HeaderAction>,
        egui::FullOutput,
    ) {
        let mut actions = Vec::new();
        let mut output = self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                actions = maestro_web::export_html::template::render_header(
                    ui,
                    &self.document,
                    &mut self.state,
                    maestro_web::export_html::template::HeaderView {
                        presentation: self.presentation,
                        localize_date: &|_| unreachable!(),
                    },
                )
                .unwrap();
            },
        );
        output.textures_delta.clear();
        (actions, output)
    }
}
/// One pointer transition at a supplied canvas point.
pub fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

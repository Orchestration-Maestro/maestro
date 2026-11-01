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
    /// Retained view policies, independent from publication.
    pub retained: RefCell<HashMap<egui::Id, (bool, Option<egui::Id>)>>,
    /// Current containing modal identities.
    pub overlays: RefCell<Vec<egui::Id>>,
    /// Callback at the retained-policy boundary, invoked without a host borrow.
    pub on_visibility: RefCell<Option<Rc<dyn Fn()>>>,
    /// Controlled retained-policy failure.
    pub fail_visibility: Cell<bool>,
    /// Original byte downloads at the reached platform boundary.
    pub downloads: RefCell<Vec<(String, String, Vec<u8>)>>,
    /// Reached platform failure, independent of decoding.
    pub fail_download: Cell<bool>,
    /// Ordered operations, never deduplicated or synthesized.
    pub events: RefCell<Vec<Event>>,
    /// Factories observed during admission, including failed publication.
    pub admissions: RefCell<Vec<Rc<ReadingContent>>>,
    /// Published identities and geometry under controlled transaction policy.
    pub published: RefCell<HashMap<egui::Id, (Rc<ReadingContent>, egui::Rect)>>,
    /// Containing modal submitted with the latest native publication.
    pub publication_modals: RefCell<HashMap<egui::Id, Option<egui::Id>>>,
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
    fn copy_to_clipboard(
        &self,
        _: &str,
        _: maestro_web::browser::attachment_content::ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>> {
        unreachable!("unexpected clipboard operation")
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
        if self.fail_download.get() {
            Err(AttachmentHostError::Host("save failed".into()))
        } else {
            Ok(())
        }
    }

    fn present(
        &self,
        s: AttachmentSurface<'_>,
        p: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError> {
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
                    return Err(AttachmentHostError::BorrowConflict);
                }
                self.pending.borrow_mut().insert(s.id, reading.content);
            }
            ReadingPhase::Publish => {
                self.events.borrow_mut().push(Event::Publish(s.id));
                *self.holes.borrow_mut() = s.occluders;
                if self.failure.get() == Some(Failure::Publish) {
                    return Err(AttachmentHostError::BorrowConflict);
                }
                let candidate = self.pending.borrow_mut().remove(&s.id).unwrap();
                assert!(Rc::ptr_eq(&candidate, &reading.content));
                self.published
                    .borrow_mut()
                    .insert(s.id, (candidate, s.rect));
                self.publication_modals
                    .borrow_mut()
                    .insert(s.id, reading.modal);
            }
        }
        Ok(())
    }
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentHostError> {
        self.events.borrow_mut().push(Event::Read(id));
        if self.failure.get() == Some(Failure::Read) {
            return Err(AttachmentHostError::BorrowConflict);
        }
        Ok(ReadingLayout {
            width: 0.,
            border: egui::epaint::MarginF32::ZERO,
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
        self.retained.borrow_mut().remove(&id);
    }
    fn set_retained_visibility(
        &self,
        id: egui::Id,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError> {
        self.retained.borrow_mut().insert(id, (visible, modal));
        let callback = self.on_visibility.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
        if self.fail_visibility.get() {
            Err(AttachmentHostError::BorrowConflict)
        } else {
            Ok(())
        }
    }
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentHostError> {
        let mut overlays = self.overlays.borrow_mut();
        if !visible {
            overlays.retain(|i| *i != id);
        } else if !overlays.contains(&id) {
            overlays.push(id);
        }
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentHostError> {
        unreachable!()
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentHostError> {
        unreachable!()
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!()
    }
    fn download(&self, _: maestro_web::SharedAttachment) -> Result<(), AttachmentHostError> {
        unreachable!()
    }
}
/// Load a real owned offline document without provider work.
/// # Panics
/// Panics when controlled fixture input is invalid.
#[must_use]
pub fn document(id: &serde_json::Value) -> maestro_web::export_html::template::SessionDocument {
    use base64::Engine as _;
    maestro_web::export_html::template::load_document(
        &base64::engine::general_purpose::STANDARD.encode(serde_json::json!({"entries":[],"header":{"id":id},"systemPrompt":"admitted prompt","tools":false}).to_string()), "", None,
    ).unwrap()
}
/// Load the repository pack through target-appropriate operations.
/// # Panics
/// Panics when controlled fixture input is invalid.
#[must_use]
pub fn pack() -> maestro_theme::BrandPack {
    crate::support::brand::pack(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/brand/brand.json"
    ))
    .unwrap()
}
/// Render through the public operation in a caller-selected layout.
/// # Panics
/// Panics if drawing mutates the caller style.
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
/// # Panics
/// Panics if drawing mutates the caller style.
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

/// Compare the painted text, in drawing order, with the recorded button labels.
/// # Panics
/// Panics when the painted labels differ from the recorded ones.
pub fn assert_button_labels(shapes: &[egui::epaint::ClippedShape], expected: &serde_json::Value) {
    let labels: Vec<_> = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect();
    let buttons: Vec<_> = expected["buttons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|button| button["text"].as_str().unwrap())
        .collect();
    assert_eq!(labels, buttons);
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
    #[must_use]
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
        self.frame_with(events, true, |_| {})
    }
    /// Draw the caller's enabled state and real foreground surfaces in the same context.
    /// # Panics
    /// Panics if controlled header drawing fails.
    pub fn frame_with(
        &mut self,
        events: Vec<egui::Event>,
        enabled: bool,
        mut foreground: impl FnMut(&egui::Context),
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
                if !enabled {
                    ui.disable();
                }
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
                foreground(ui.ctx());
            },
        );
        output.textures_delta.clear();
        (actions, output)
    }
}
pub use crate::support::pointer::pointer;

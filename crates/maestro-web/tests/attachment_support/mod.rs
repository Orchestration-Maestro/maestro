//! Controlled attachment effects and genuine native drawing frames.
#![cfg(test)]
use egui::{Context, Event, Pos2, RawInput, Rect, Shape, Vec2};
use maestro_app::browser::utils::attachment_utils::AttachmentType;
use maestro_theme::{BrandMode, BrandPack};
use maestro_web::{Attachment, I18n, browser::attachment_content::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// One observed content placement.
#[derive(Clone)]
pub enum Content {
    /// Native reading factory and its transaction phase.
    Reading(Rc<ReadingContent>, ReadingPhase),
    /// Browser image operands.
    Image(String, String, ImageFit),
    /// Literal text operands.
    Text(String, TextView),
    /// Pending document preview.
    Preview(FileType, Rc<RefCell<Attachment>>, bool),
}
/// Recorded surface and presentation.
#[derive(Clone)]
pub struct Surface {
    /// Component identity.
    pub id: egui::Id,
    /// Full content rectangle.
    pub rect: Rect,
    /// Clipping rectangle.
    pub clip: Rect,
    /// Density.
    pub density: f32,
    /// Image opacity.
    pub opacity: f32,
    /// Controls retained above native content.
    pub occluders: Vec<Rect>,
    /// Owned observed content.
    pub content: Content,
    /// Actual projected brand properties.
    pub css: std::collections::BTreeMap<String, String>,
}
/// Recording platform boundary.
#[derive(Default)]
pub struct Host {
    /// Presentation history.
    pub surfaces: RefCell<Vec<Surface>>,
    /// Ordered lifecycle effects.
    pub effects: RefCell<Vec<String>>,
    /// Downloaded original handles.
    pub downloads: RefCell<Vec<Rc<RefCell<Attachment>>>>,
    /// Platform hover capability.
    pub hover: Cell<bool>,
    /// Controlled presentation failure.
    pub fail: RefCell<Option<String>>,
    /// Controlled download failure.
    pub fail_download: Cell<bool>,
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
        p: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError> {
        let content = match s.content {
            AttachmentContent::Reading(reading) => Content::Reading(reading.content, reading.phase),
            AttachmentContent::Input { .. } => {
                unreachable!("attachment suite does not create inputs")
            }
            AttachmentContent::Image { source, alt, fit } => {
                Content::Image(source.into(), alt.into(), fit)
            }
            AttachmentContent::Text { text, view } => Content::Text(text.into(), view),
            AttachmentContent::Preview {
                file_type,
                attachment,
                reload,
            } => Content::Preview(file_type, attachment, reload),
        };
        self.surfaces.borrow_mut().push(Surface {
            id: s.id,
            rect: s.rect,
            clip: s.clip_rect,
            density: s.pixels_per_point,
            opacity: s.opacity,
            occluders: s.occluders,
            content,
            css: p.css_properties(),
        });
        if let Some(message) = self.fail.borrow_mut().take() {
            return Err(AttachmentUiError::Presentation(message));
        }
        Ok(())
    }
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentUiError> {
        self.effects
            .borrow_mut()
            .push(format!("modal:{id:?}:{visible}"));
        Ok(())
    }
    fn hover_capable(&self) -> Result<bool, AttachmentUiError> {
        Ok(self.hover.get())
    }
    fn remove(&self, id: egui::Id) {
        self.effects.borrow_mut().push(format!("remove:{id:?}"));
    }
    fn take_escape(&self) -> bool {
        false
    }
    fn timer(
        &self,
        _: i32,
        _: bool,
        _: Rc<dyn Fn()>,
    ) -> Result<maestro_web::browser::timer::Timer, AttachmentUiError> {
        unreachable!("attachment suite does not arm timers")
    }
    fn spawn_local(&self, _: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>) {
        unreachable!("attachment suite does not dispatch futures")
    }
    fn download_bytes(&self, _: &[u8], _: &str, _: &str) -> Result<(), AttachmentUiError> {
        unreachable!("unexpected byte download")
    }
    fn download(&self, a: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError> {
        self.downloads.borrow_mut().push(a);
        if self.fail_download.get() {
            Err(AttachmentUiError::Host("download rejected".into()))
        } else {
            Ok(())
        }
    }
}
/// A literal original attachment independent of its preview.
#[must_use]
pub fn attachment() -> Rc<RefCell<Attachment>> {
    Rc::new(RefCell::new(Attachment {
        id: "same".into(),
        r#type: AttachmentType::Document,
        file_name: "note.txt".into(),
        mime_type: "text/plain".into(),
        size: 3,
        content: "QUJD".into(),
        extracted_text: Some("<literal>\n\ttext".into()),
        preview: None,
    }))
}
/// Shared native frame owner.
pub struct Frame {
    /// Actual egui context with test-only default fonts.
    pub ctx: Context,
    /// Caller-selected pack.
    pub pack: BrandPack,
    /// Caller-selected mode.
    pub mode: BrandMode,
    /// Localized text context.
    pub i18n: I18n,
    /// Last emitted shapes.
    pub shapes: Vec<egui::epaint::ClippedShape>,
    /// Raster images actually submitted to the renderer.
    pub images: Vec<egui::ColorImage>,
    /// Canvas frame bounds.
    pub screen: Rect,
    /// Native viewport density.
    pub density: f32,
}
impl Default for Frame {
    fn default() -> Self {
        let host = Rc::new(crate::support::Host::default());
        Self {
            ctx: Context::default(),
            pack: maestro_theme::load_brand_pack(
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json"),
                &maestro_theme::theme::NativeThemeOperations,
            )
            .unwrap(),
            mode: BrandMode::Dark,
            i18n: I18n::new(host).unwrap(),
            shapes: vec![],
            images: vec![],
            screen: Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.)),
            density: 1.,
        }
    }
}
impl Frame {
    /// Drive a controlled input frame, retaining genuine drawing output.
    /// # Panics
    /// Panics if the controlled pack or root viewport is invalid.
    pub fn run<T>(
        &mut self,
        events: Vec<Event>,
        draw: impl FnOnce(&Context, &I18n, &maestro_theme::BrandPresentation<'_>) -> T,
    ) -> T {
        let mut input = RawInput {
            screen_rect: Some(self.screen),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.density);
        self.ctx.begin_pass(input);
        let value = draw(
            &self.ctx,
            &self.i18n,
            &self.pack.presentation(self.mode).unwrap(),
        );
        let mut output = self.ctx.end_pass();
        self.images = output
            .textures_delta
            .set
            .values()
            .flatten()
            .filter(|delta| delta.image.size() == [20, 20])
            .map(|delta| match &delta.image {
                egui::ImageData::Color(image) => (**image).clone(),
            })
            .collect();
        output.textures_delta.clear();
        self.shapes = output.shapes;
        value
    }
    /// Drive the retained overlay.
    /// # Errors
    /// Returns the widget failure.
    pub fn overlay(
        &mut self,
        o: &mut maestro_web::AttachmentOverlay,
        events: Vec<Event>,
    ) -> Result<bool, AttachmentUiError> {
        self.run(events, |ctx, i, p| o.render(ctx, i, p))
    }
    /// Draw one tile at a deterministic position.
    /// # Errors
    /// Returns the widget failure.
    pub fn tile(
        &mut self,
        t: &mut maestro_web::AttachmentTile,
        events: Vec<Event>,
    ) -> Result<Option<maestro_web::AttachmentOverlay>, AttachmentUiError> {
        self.run(events, |ctx, i, p| {
            let mut ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("root"),
                egui::UiBuilder::new()
                    .max_rect(ctx.content_rect())
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            t.render(&mut ui, i, p)
        })
    }
    /// Complete visible text in painter order.
    #[must_use]
    pub fn texts(&self) -> Vec<String> {
        fn visit(s: &Shape, out: &mut Vec<String>) {
            match s {
                Shape::Text(t) => out.push(t.galley.job.text.clone()),
                Shape::Vec(v) => v.iter().for_each(|shape| visit(shape, out)),
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            visit(&s.shape, &mut out);
        }
        out
    }
    /// Find the exact visible label's rectangle.
    /// # Panics
    /// Panics when the requested label was not drawn.
    #[must_use]
    pub fn label(&self, label: &str) -> Rect {
        self.shapes
            .iter()
            .find_map(|s| match &s.shape {
                Shape::Text(t) if t.galley.job.text == label => {
                    Some(Rect::from_min_size(t.pos, t.galley.size()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing label {label}"))
    }
}
/// A complete primary click at a supplied point.
#[must_use]
pub fn click(pos: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

/// Original typed record input.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    /// Original identifier.
    id: String,
    /// Image or document.
    r#type: String,
    /// Literal name.
    file_name: String,
    /// Literal MIME.
    mime_type: String,
    /// Original size.
    size: usize,
    /// Original bytes.
    content: String,
    /// Optional document text.
    extracted_text: Option<String>,
    /// Optional thumbnail.
    preview: Option<String>,
}
impl Input {
    /// Construct the existing application-owned record.
    /// # Panics
    /// Panics on an unknown fixture discriminant.
    #[must_use]
    pub fn record(&self) -> Rc<RefCell<maestro_web::Attachment>> {
        Rc::new(RefCell::new(maestro_web::Attachment {
            id: self.id.clone(),
            r#type: match self.r#type.as_str() {
                "image" => maestro_app::browser::utils::attachment_utils::AttachmentType::Image,
                "document" => {
                    maestro_app::browser::utils::attachment_utils::AttachmentType::Document
                }
                _ => panic!("unknown fixture type"),
            },
            file_name: self.file_name.clone(),
            mime_type: self.mime_type.clone(),
            size: self.size,
            content: self.content.clone(),
            extracted_text: self.extracted_text.clone(),
            preview: self.preview.clone(),
        }))
    }
}
/// Visible icon-control rectangles in right-to-left order.
impl Frame {
    /// Locate a control through its actual painted glyph bounds.
    /// # Panics
    /// Panics when the requested control was not drawn.
    #[must_use]
    pub fn control(&self, index: usize) -> egui::Pos2 {
        let mut bounds: Vec<_> = self
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
                _ => None,
            })
            .collect();
        bounds.sort_by(|a, b| b.center().x.total_cmp(&a.center().x));
        bounds[index].center()
    }
}

impl Frame {
    /// Read selected indices from the actual painted label backgrounds.
    /// # Panics
    /// Panics if a requested label was not drawn.
    #[must_use]
    pub fn selected(&self, labels: &[String], fill: egui::Color32) -> Vec<usize> {
        labels.iter().enumerate().filter_map(|(index,label)|{let center=self.label(label).center();self.shapes.iter().any(|s|matches!(&s.shape,Shape::Rect(rect) if rect.rect.contains(center)&&rect.fill==fill)).then_some(index)}).collect()
    }
}

//! Call-scoped content placement with retained platform ownership.
use crate::{
    Attachment, I18nError,
    icons::{self, IconError, IconSize},
};
use maestro_theme::BrandPresentation;
use num_traits::ToPrimitive as _;
use std::{cell::RefCell, fmt, io, rc::Rc};
#[cfg(target_arch = "wasm32")]
pub(crate) mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserAttachmentHost;
/// Detected attachment presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileType {
    /// Original image.
    Image,
    /// Portable document.
    Pdf,
    /// Word document.
    Docx,
    /// Presentation text.
    Pptx,
    /// Spreadsheet.
    Excel,
    /// Plain text.
    Text,
}
/// Native image fitting policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFit {
    /// Fill the thumbnail.
    Cover,
    /// Preserve the complete image.
    Contain,
}
/// Literal text presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextView {
    /// Ordinary text document.
    Plain,
    /// Selected extracted text.
    Extracted,
    /// Presentation slide text.
    Presentation,
}
pub(crate) mod reading;
pub use reading::{ReadingContent, ReadingLayout, ReadingPhase, ReadingPlacement};
#[cfg(target_arch = "wasm32")]
pub use reading::{ReadingMount, ReadingRoot};
/// Borrowed operands for one platform content update.
pub enum AttachmentContent<'a> {
    /// Owned native reading content.
    Reading(ReadingPlacement<'a>),
    /// Browser-native editable input.
    Input {
        /// Live native field configuration.
        props: &'a crate::components::input::InputProps,
        /// Localized native accessible name, including the required marker.
        accessible_label: &'a str,
        /// Assign the caller's changed value, preserving native edits otherwise.
        value_changed: bool,
        /// Containing modal, when placed in a foreground layer.
        modal: Option<egui::Id>,
        /// Drawing context notified by native keyboard activity.
        context: &'a egui::Context,
    },
    /// Browser-native image.
    Image {
        /// Complete image URL.
        source: &'a str,
        /// Literal alternative text.
        alt: &'a str,
        /// Fitting policy.
        fit: ImageFit,
    },
    /// Selectable literal text.
    Text {
        /// Unescaped source text.
        text: &'a str,
        /// Text layout.
        view: TextView,
    },
    /// Format-owned document container.
    Preview {
        /// Selected renderer.
        file_type: FileType,
        /// Original mutable record.
        attachment: Rc<RefCell<Attachment>>,
        /// Whether the renderer must restart.
        reload: bool,
    },
}
/// A component's content rectangle in canvas points.
pub struct AttachmentSurface<'a> {
    /// Component identity, independent of attachment identity.
    pub id: egui::Id,
    /// Full content bounds in canvas points.
    pub rect: egui::Rect,
    /// Visible clip bounds in canvas points, without resizing content.
    pub clip_rect: egui::Rect,
    /// Canvas physical pixels per point.
    pub pixels_per_point: f32,
    /// Native content opacity.
    pub opacity: f32,
    /// Disjoint control rectangles in canvas points to exclude from native content.
    pub occluders: Vec<egui::Rect>,
    /// Content operands copied or retained by the host.
    pub content: AttachmentContent<'a>,
}
/// Attachment drawing or effect failure.
#[derive(Debug)]
pub enum AttachmentUiError {
    /// Localization failed.
    I18n(I18nError),
    /// Glyph rendering failed.
    Icon(IconError),
    /// Browser effect failed.
    Host(String),
    /// File presentation failed.
    Presentation(String),
    /// Caller holds an incompatible record borrow.
    BorrowConflict,
}
impl fmt::Display for AttachmentUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I18n(e) => e.fmt(f),
            Self::Icon(e) => e.fmt(f),
            Self::Host(e) | Self::Presentation(e) => f.write_str(e),
            Self::BorrowConflict => f.write_str("Attachment is already borrowed"),
        }
    }
}
impl std::error::Error for AttachmentUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I18n(e) => Some(e),
            Self::Icon(e) => Some(e),
            _ => None,
        }
    }
}
impl From<I18nError> for AttachmentUiError {
    fn from(e: I18nError) -> Self {
        Self::I18n(e)
    }
}
impl From<io::Error> for AttachmentUiError {
    fn from(error: io::Error) -> Self {
        Self::Presentation(error.to_string())
    }
}
/// Replaceable browser content and download effects.
pub trait AttachmentHost {
    /// Measure the admitted reading root in the current canvas frame.
    /// # Errors
    /// Returns missing-surface or native geometry failures.
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentUiError>;
    /// Update one retained content element.
    /// # Errors
    /// Returns platform or file-presentation failures.
    fn present(
        &self,
        surface: AttachmentSurface<'_>,
        presentation: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentUiError>;
    /// Change modal ownership, preserving hidden surfaces.
    /// # Errors
    /// Returns platform failures.
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentUiError>;
    /// Query the platform hover media capability.
    /// # Errors
    /// Returns platform failures.
    fn hover_capable(&self) -> Result<bool, AttachmentUiError>;
    /// Release the named surface.
    fn remove(&self, id: egui::Id);
    /// Consume Escape delivered by a native input outside canvas focus.
    fn take_escape(&self) -> bool;
    /// Register a native callback whose handle revokes delivery on drop.
    /// # Errors
    /// Returns native timer registration failures.
    fn timer(
        &self,
        delay_ms: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<crate::browser::timer::Timer, AttachmentUiError>;
    /// Dispatch owned browser-local asynchronous work.
    fn spawn_local(&self, future: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>);
    /// Download supplied bytes synchronously without attachment decoding.
    /// # Errors
    /// Returns platform failures.
    fn download_bytes(&self, bytes: &[u8], name: &str, mime: &str)
    -> Result<(), AttachmentUiError>;
    /// Download the original record.
    /// # Errors
    /// Returns decoding or platform failures.
    fn download(&self, attachment: Rc<RefCell<Attachment>>) -> Result<(), AttachmentUiError>;
}
/// Convert brand measurements for the drawing library.
pub(crate) fn points(n: f64) -> f32 {
    n.to_f32().unwrap_or_default()
}
/// Resolve a caller-selected color.
pub(crate) fn color(p: &BrandPresentation<'_>, role: &str) -> io::Result<egui::Color32> {
    egui::Color32::from_hex(p.colors.get(role).copied().unwrap_or(""))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))
}
/// Apply the caller's control roles without installing fonts.
pub(crate) fn style(ui: &mut egui::Ui, p: &BrandPresentation<'_>) -> io::Result<()> {
    let v = ui.visuals_mut();
    v.override_text_color = None;
    v.panel_fill = color(p, "background")?;
    v.selection.bg_fill = color(p, "accent")?;
    v.selection.stroke.color = color(p, "on-accent")?;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.noninteractive,
    ] {
        w.bg_fill = color(p, "panel")?;
        w.weak_bg_fill = w.bg_fill;
        w.fg_stroke.color = color(p, "text")?;
        w.bg_stroke.color = color(p, "border")?;
        w.corner_radius = egui::CornerRadius::from(points(p.radii["control"]));
    }
    for (name, kind) in &[
        (egui::TextStyle::Body, "body"),
        (egui::TextStyle::Button, "body"),
        (egui::TextStyle::Small, "body"),
        (egui::TextStyle::Monospace, "mono"),
    ] {
        let t = &p.typography[*kind];
        let family = &p.fonts[&t.font].family;
        let family = if ui.ctx().fonts(|f| {
            f.families()
                .contains(&egui::FontFamily::Name(family.clone().into()))
        }) {
            egui::FontFamily::Name(family.clone().into())
        } else if *kind == "mono" {
            egui::FontFamily::Monospace
        } else {
            egui::FontFamily::Proportional
        };
        ui.style_mut()
            .text_styles
            .insert(name.clone(), egui::FontId::new(points(t.size), family));
    }
    Ok(())
}
/// Paint a catalog glyph into a supplied rectangle.
pub(crate) fn glyph(
    ui: &egui::Ui,
    rect: egui::Rect,
    name: &str,
    tint: egui::Color32,
) -> Result<(), AttachmentUiError> {
    let catalog = icons::icons().map_err(|e| AttachmentUiError::Host(e.to_string()))?;
    let node = catalog
        .get(name)
        .ok_or_else(|| AttachmentUiError::Host(format!("Missing glyph: {name}")))?;
    let image = icons::icon(node, IconSize::Md, tint, ui.ctx().pixels_per_point())
        .map_err(AttachmentUiError::Icon)?;
    let texture = ui.ctx().load_texture(
        format!("{name}:{tint:?}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    ui.painter().image(
        texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
        egui::Color32::WHITE,
    );
    Ok(())
}

/// Allocate an identity independent of attachment contents and addresses.
pub(crate) fn identity() -> egui::Id {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    egui::Id::new((
        "attachment",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ))
}

/// Draw an icon-only ghost control with the supplied hover and focus roles.
pub(crate) fn ghost(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    name: &str,
) -> Result<egui::Response, AttachmentUiError> {
    ghost_sized(ui, p, name, egui::vec2(32., 32.))
}
/// Draw a ghost control with a centered sixteen-point glyph.
pub(crate) fn ghost_sized(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    name: &str,
    size: egui::Vec2,
) -> Result<egui::Response, AttachmentUiError> {
    let response = ui
        .allocate_response(size, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() || response.has_focus() {
        ui.painter().rect(
            response.rect,
            egui::CornerRadius::from(points(p.radii["control"])),
            color(p, "panel")?,
            egui::Stroke::new(
                1.,
                color(
                    p,
                    if response.has_focus() {
                        "accent"
                    } else {
                        "border"
                    },
                )?,
            ),
            egui::StrokeKind::Inside,
        );
    }
    glyph(
        ui,
        egui::Rect::from_center_size(response.rect.center(), egui::vec2(16., 16.)),
        name,
        color(p, "text")?,
    )?;
    Ok(response)
}
#[cfg(any(target_arch = "wasm32", test))]
mod placement;
#[cfg(test)]
mod tests;

/// Supply the border on the native owner that would otherwise cover canvas paint.
#[cfg(any(target_arch = "wasm32", test))]
fn content_border(content: &AttachmentContent<'_>) -> &'static str {
    match content {
        AttachmentContent::Image {
            fit: ImageFit::Cover,
            ..
        }
        | AttachmentContent::Preview {
            file_type: FileType::Pdf | FileType::Docx,
            ..
        } => "border:1px solid var(--maestro-color-border);",
        _ => "",
    }
}
/// Select retained surfaces whose placement or modal visibility changed.
#[cfg(any(target_arch = "wasm32", test))]
fn style_targets(
    ids: impl Iterator<Item = egui::Id>,
    placed: Option<egui::Id>,
    previous: Option<egui::Id>,
    current: Option<egui::Id>,
) -> impl Iterator<Item = egui::Id> {
    ids.take(if previous == current { 0 } else { usize::MAX })
        .chain(placed.filter(move |_| previous == current))
}

#[cfg(target_arch = "wasm32")]
mod input;

#[cfg(any(target_arch = "wasm32", test))]
/// One modal visibility projection shared by all native content.
fn visibility_style(id: egui::Id, modal: Option<egui::Id>, top: Option<egui::Id>) -> &'static str {
    match top {
        None => "visibility:visible;z-index:1;",
        Some(top) if top == id || modal == Some(top) => "visibility:visible;z-index:51;",
        Some(_) => "visibility:hidden;z-index:1;",
    }
}

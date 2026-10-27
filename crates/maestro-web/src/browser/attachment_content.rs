//! Call-scoped content placement with retained platform ownership.
use crate::icons::{self, IconSize};
use maestro_theme::BrandPresentation;
use num_traits::ToPrimitive as _;
use std::{io, rc::Rc};
#[cfg(target_arch = "wasm32")]
pub(crate) mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserAttachmentHost;
mod control_error;
pub use control_error::ControlError;
mod placement_scope;
pub use placement_scope::PlacementScope;

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
#[cfg(target_arch = "wasm32")]
pub use reading::{ReadingCommit, ReadingMount, ReadingRoot};
pub use reading::{ReadingContent, ReadingLayout, ReadingPhase, ReadingPlacement};
/// Borrowed operands for one platform content update.
pub enum AttachmentContent<'a> {
    /// Owned native reading content.
    Reading(ReadingPlacement<'a>),
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
        attachment: crate::SharedAttachment,
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
/// Native content or platform effect failure.
#[derive(Debug)]
pub enum AttachmentHostError {
    /// Native effect failed.
    Host(String),
    /// Native content presentation failed.
    Presentation(String),
    /// Caller holds an incompatible native reference borrow.
    BorrowConflict,
}
impl std::fmt::Display for AttachmentHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host(error) | Self::Presentation(error) => std::fmt::Display::fmt(error, f),
            Self::BorrowConflict => f.write_str("Attachment is already borrowed"),
        }
    }
}
impl std::error::Error for AttachmentHostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
/// Native clipboard failure policy selected by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardMode {
    /// Use only the native write API.
    Direct,
    /// Try temporary selection when native writing fails.
    Fallback,
}
/// Replaceable browser content, download and clipboard effects.
pub trait AttachmentHost {
    /// Activate a row's placement lifetime, restoring nested activation on drop.
    /// Hosts without retained native surfaces can keep the empty default.
    fn enter_placement_scope(&self, _scope: egui::Id) -> PlacementScope {
        PlacementScope::default()
    }
    /// Detach this scope's surfaces without releasing their retained owners.
    /// Hosts without retained native surfaces need no revocation.
    fn hide_placement_scope(&self, _scope: egui::Id) {}

    /// Begin copying literal text, owning native operands before returning local work.
    /// The result reports success under the selected policy, without UI feedback.
    fn copy_to_clipboard(
        &self,
        text: &str,
        mode: ClipboardMode,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool>>>;

    /// Measure the admitted reading root in the current canvas frame.
    /// # Errors
    /// Returns missing-surface or native geometry failures.
    fn reading_layout(&self, id: egui::Id) -> Result<ReadingLayout, AttachmentHostError>;
    /// Update one retained content element.
    /// # Errors
    /// Returns platform or file-presentation failures.
    fn present(
        &self,
        surface: AttachmentSurface<'_>,
        presentation: &BrandPresentation<'_>,
    ) -> Result<(), AttachmentHostError>;
    /// Record retained visibility independently from content publication.
    /// # Errors
    /// Returns platform failures without releasing the retained surface.
    fn set_retained_visibility(
        &self,
        id: egui::Id,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentHostError>;
    /// Change modal admission, releasing ordinary modal-owned children on close.
    /// # Errors
    /// Returns platform failures.
    fn set_overlay(&self, id: egui::Id, visible: bool) -> Result<(), AttachmentHostError>;
    /// Query the platform hover media capability.
    /// # Errors
    /// Returns platform failures.
    fn hover_capable(&self) -> Result<bool, AttachmentHostError>;
    /// Release the named surface.
    fn remove(&self, id: egui::Id);
    /// Register a native callback whose handle revokes delivery on drop.
    /// # Errors
    /// Returns native timer registration failures.
    fn timer(
        &self,
        delay_ms: i32,
        repeat: bool,
        callback: Rc<dyn Fn()>,
    ) -> Result<crate::browser::timer::Timer, AttachmentHostError>;
    /// Dispatch owned browser-local asynchronous work.
    fn spawn_local(&self, future: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>);
    /// Download supplied bytes synchronously without attachment decoding.
    /// # Errors
    /// Returns platform failures.
    fn download_bytes(
        &self,
        bytes: &[u8],
        name: &str,
        mime: &str,
    ) -> Result<(), AttachmentHostError>;
    /// Download the original record.
    /// # Errors
    /// Returns decoding or platform failures.
    fn download(&self, attachment: crate::SharedAttachment) -> Result<(), AttachmentHostError>;
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
) -> Result<(), ControlError> {
    let node = glyph_node(name)?;
    paint_icon(ui, rect, &node, (IconSize::Md, tint, 0.))
}
/// Resolve catalog geometry at the existing named-control error boundary.
/// # Errors
/// Returns the catalog or missing-glyph control failure.
pub(crate) fn glyph_node(name: &str) -> Result<icons::IconNode, ControlError> {
    icons::icon_node(name)
        .map_err(|error| ControlError::Host(error.to_string()))?
        .ok_or_else(|| ControlError::Host(format!("Missing glyph: {name}")))
}
/// Paint supplied geometry with a caller-selected size, tint and rotation.
pub(crate) fn paint_icon(
    ui: &egui::Ui,
    rect: egui::Rect,
    node: &icons::IconNode,
    (size, tint, rotation): (IconSize, egui::Color32, f32),
) -> Result<(), ControlError> {
    let image =
        icons::icon(node, size, tint, ui.ctx().pixels_per_point()).map_err(ControlError::Icon)?;
    let texture = ui.ctx().load_texture(
        format!("{node:?}:{size:?}:{tint:?}"),
        image,
        egui::TextureOptions::LINEAR,
    );
    egui::Image::new((texture.id(), rect.size()))
        .rotate(rotation, egui::vec2(0.5, 0.5))
        .paint_at(ui, rect);
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

/// Default icon-control extent in logical points.
pub(crate) const GHOST_SIZE: egui::Vec2 = egui::vec2(32., 32.);

/// Draw an icon-only ghost control with the supplied hover and focus roles.
pub(crate) fn ghost(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    name: &str,
) -> Result<egui::Response, ControlError> {
    ghost_sized(ui, p, name, GHOST_SIZE)
}
/// Draw a ghost control with a centered sixteen-point glyph.
pub(crate) fn ghost_sized(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    name: &str,
    size: egui::Vec2,
) -> Result<egui::Response, ControlError> {
    ghost_labeled(ui, p, (name, None), size)
}
/// Highlight treatment for the shared ghost-control painter.
#[derive(Clone, Copy)]
pub(crate) enum GhostHover {
    /// Ordinary text on the existing ghost surface.
    Text,
    /// Accent foreground on an accent-filled surface.
    Accent,
}
/// Draw the same ghost control with an optional adjacent label.
pub(crate) fn ghost_labeled(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    content: (&str, Option<&str>),
    size: egui::Vec2,
) -> Result<egui::Response, ControlError> {
    let font = egui::TextStyle::Button.resolve(ui.style());
    ghost_labeled_tinted(
        ui,
        p,
        content,
        (size, 0., font),
        (
            color(p, "text").map_err(|error| ControlError::Presentation(error.to_string()))?,
            GhostHover::Text,
        ),
    )
}
/// Draw a shared ghost control with caller-selected idle tint and highlight treatment.
pub(crate) fn ghost_labeled_tinted(
    ui: &mut egui::Ui,
    p: &BrandPresentation<'_>,
    (name, label): (&str, Option<&str>),
    (size, label_padding, font): (egui::Vec2, f32, egui::FontId),
    (tint, hover): (egui::Color32, GhostHover),
) -> Result<egui::Response, ControlError> {
    let response = ui
        .allocate_response(size, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let highlighted = response.hovered() || response.has_focus();
    let accent = matches!(hover, GhostHover::Accent);
    let foreground = if highlighted {
        color(p, if accent { "on-accent" } else { "text" })
            .map_err(|error| ControlError::Presentation(error.to_string()))?
    } else {
        tint
    };
    if highlighted {
        ui.painter().rect(
            response.rect,
            egui::CornerRadius::from(points(p.radii["control"])),
            color(p, if accent { "accent" } else { "panel" })
                .map_err(|error| ControlError::Presentation(error.to_string()))?,
            egui::Stroke::new(
                1.,
                color(
                    p,
                    if response.has_focus() {
                        "accent"
                    } else {
                        "border"
                    },
                )
                .map_err(|error| ControlError::Presentation(error.to_string()))?,
            ),
            egui::StrokeKind::Inside,
        );
    }
    let center = label.map_or(response.rect.center(), |_| {
        egui::pos2(
            response.rect.left() + GHOST_SIZE.x / 2.,
            response.rect.center().y,
        )
    });
    glyph(
        ui,
        egui::Rect::from_center_size(center, egui::vec2(16., 16.)),
        name,
        foreground,
    )?;
    if let Some(label) = label {
        ui.painter().text(
            response.rect.right_center() - egui::vec2(label_padding, 0.),
            egui::Align2::RIGHT_CENTER,
            label,
            font,
            if accent {
                foreground
            } else {
                color(p, "text").map_err(|error| ControlError::Presentation(error.to_string()))?
            },
        );
    }
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

#[cfg(any(target_arch = "wasm32", test))]
/// One modal visibility projection shared by all native content.
fn visibility_style(id: egui::Id, modal: Option<egui::Id>, top: Option<egui::Id>) -> &'static str {
    match top {
        None => "visibility:visible;z-index:1;",
        Some(top) if top == id || modal == Some(top) => "visibility:visible;z-index:51;",
        Some(_) => "visibility:hidden;z-index:1;",
    }
}

/// Paint the shared two-caster panel shadow outside the reading viewport.
pub(crate) fn panel_shadow(ui: &egui::Ui, rect: egui::Rect, radius: f32, border: egui::Color32) {
    for (offset, blur, shrink) in [(1, 3, 0.), (8, 10, 1.)] {
        ui.painter().add(
            egui::epaint::Shadow {
                offset: [0, offset],
                blur,
                spread: 0,
                color: border.gamma_multiply(0.1),
            }
            .as_shape(rect.shrink(shrink), radius),
        );
    }
}

/// Round and clamp only the final encoded color channel.
pub(crate) fn channel(value: f32) -> u8 {
    value.round().clamp(0., 255.).to_u8().unwrap_or_default()
}

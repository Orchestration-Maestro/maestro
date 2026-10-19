//! Dialog templates with retained native children and canvas controls.
mod ui_error;
use super::dialog_base::{DialogBase, DialogContext};
use crate::browser::attachment_content::{ReadingContent, ReadingLayout};
use maestro_theme::BrandPresentation;
use std::rc::Rc;
pub use ui_error::DialogUiError;
/// Caller-owned controls painted against the complete native measurement.
pub type RenderContent = dyn Fn(
    &mut egui::Ui,
    egui::Id,
    &ReadingLayout,
    &BrandPresentation<'_>,
) -> Result<(), DialogUiError>;
/// Owned native body with its canvas control painter.
#[derive(Clone)]
pub struct DialogBody {
    /// Native reading factory.
    pub reading: Rc<ReadingContent>,
    /// Actual canvas control dimensions.
    pub control_sizes: Vec<(egui::Id, egui::Vec2)>,
    /// Owned canvas control painter.
    pub render_content: Rc<RenderContent>,
}
/// Live standalone dialog operands.
#[derive(Clone)]
pub struct DialogProps {
    /// Whether the shell is mounted.
    pub is_open: bool,
    /// Optional notification after removal.
    pub on_close: Option<Rc<dyn Fn()>>,
    /// Owned mixed content.
    pub children: DialogBody,
    /// Literal native CSS width.
    pub width: String,
    /// Literal native CSS height.
    pub height: String,
    /// Caller panel classes.
    pub class_name: String,
    /// Caller outside-region classes; full-viewport paint is separate.
    pub backdrop_class_name: String,
}
impl DialogProps {
    /// Supply standalone dimensions and empty classes.
    #[must_use]
    pub fn new(is_open: bool, children: DialogBody) -> Self {
        Self {
            is_open,
            on_close: None,
            children,
            width: "min(600px, 90vw)".into(),
            height: "auto".into(),
            class_name: String::new(),
            backdrop_class_name: String::new(),
        }
    }
}
/// Retain a standalone shell, eagerly admitting open props.
/// # Errors
/// Returns reached platform failures.
pub fn dialog(context: &DialogContext, props: DialogProps) -> Result<DialogBase, DialogUiError> {
    let owner = DialogBase::new(context.clone(), props.children.clone());
    owner.set_props(props)?;
    Ok(owner)
}

/// One frame's owned operands, never borrowed across native or caller effects.
struct Placement<'a> {
    /// Retained handle for this frame only.
    state: &'a Rc<super::dialog_base::State>,
    /// Mount identity captured before any draw effects.
    mount: u64,
    /// Live caller snapshot.
    props: DialogProps,
    /// Caller-selected colors and typography.
    presentation: &'a BrandPresentation<'a>,
}
impl Placement<'_> {
    /// Reuse the retained body identity, adding semantics only on the browser target.
    fn reading(&self) -> Rc<ReadingContent> {
        #[cfg(target_arch = "wasm32")]
        {
            browser::prepare(self.state, &self.props.children.reading)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Rc::clone(&self.props.children.reading)
        }
    }
    /// Stage or publish this body's native placement.
    fn place(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        phase: crate::browser::attachment_content::ReadingPhase,
        occluders: Vec<egui::Rect>,
    ) -> Result<(), DialogUiError> {
        use crate::browser::attachment_content::{
            AttachmentContent, AttachmentSurface, ReadingPlacement,
        };
        self.state
            .context
            .host()
            .present(
                AttachmentSurface {
                    id: self.state.id,
                    rect,
                    clip_rect: ui.clip_rect(),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    opacity: 1.,
                    occluders,
                    content: AttachmentContent::Reading(ReadingPlacement {
                        intrinsic_width: false,
                        content: self.reading(),
                        phase,
                        sizes: &self.props.children.control_sizes,
                        modal: Some(self.state.id),
                        context: ui.ctx(),
                    }),
                },
                self.presentation,
            )
            .map_err(DialogUiError::Host)
    }
    /// Measure first, then use the complete snapshot for native placement and controls.
    fn render(&self, ui: &mut egui::Ui, close_name: &str) -> Result<bool, DialogUiError> {
        use crate::browser::attachment_content::{self as content, ReadingPhase};
        #[cfg(target_arch = "wasm32")]
        browser::configure(self.state, close_name, &self.props.class_name)?;
        self.place(ui, ui.ctx().content_rect(), ReadingPhase::Measure, vec![])?;
        let rect = self
            .state
            .context
            .host()
            .dialog_bounds(self.state.id, [&self.props.width, &self.props.height])?;
        let layout = self.state.context.host().reading_layout(self.state.id)?;
        let mut holes: Vec<_> = layout
            .controls
            .iter()
            .map(|(_, slot)| slot.translate(rect.min.to_vec2()))
            .collect();
        holes.push(close_rect(rect));
        self.place(ui, rect, ReadingPhase::Publish, holes)?;
        #[cfg(target_arch = "wasm32")]
        browser::commit(self.state);
        let radius = content::points(self.presentation.radii["panel"]);
        let border = content::color(self.presentation, "border")
            .map_err(|error| DialogUiError::Presentation(error.to_string()))?;
        content::panel_shadow(ui, rect, radius, border);
        ui.painter().rect(
            rect,
            radius,
            content::color(self.presentation, "background")
                .map_err(|error| DialogUiError::Presentation(error.to_string()))?,
            egui::Stroke::new(1., border),
            egui::StrokeKind::Inside,
        );
        let mut body = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        body.set_clip_rect(rect.intersect(ui.clip_rect()));
        (self.props.children.render_content)(&mut body, self.state.id, &layout, self.presentation)?;
        if self.state.mount.get() != self.mount {
            return Ok(false);
        }
        self.controls(ui, rect, close_name)
    }
    /// Draw the sixteen-point control and admit only full two-endpoint gestures.
    fn controls(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        name: &str,
    ) -> Result<bool, DialogUiError> {
        use crate::browser::attachment_content::reading::controls::pointer_actions;
        let close = close_rect(rect);
        let response = ui
            .interact(
                close.intersect(ui.clip_rect()),
                self.state.id.with("close"),
                egui::Sense::click(),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
        #[cfg(target_arch = "wasm32")]
        let native_focus = browser::focused(self.state);
        #[cfg(not(target_arch = "wasm32"))]
        let native_focus = false;
        self.paint_control(ui, close, &response, native_focus)?;
        let screen = ui.ctx().content_rect();
        let mut slots = vec![(true, close.intersect(ui.clip_rect()), ui.layer_id())];
        for outside in outside_rects(screen, rect) {
            if outside.is_positive() {
                slots.push((false, outside.intersect(ui.clip_rect()), ui.layer_id()));
            }
        }
        let events = ui.input(|input| input.events.clone());
        let mut pressed = self.state.pressed.take();
        let clicked = !pointer_actions(&events, &mut pressed, &slots, ui.ctx()).is_empty();
        *self.state.pressed.borrow_mut() = pressed;
        Ok(clicked || (response.clicked() && !response.clicked_by(egui::PointerButton::Primary)))
    }
    /// Sample mounted properties once, then paint without retaining their borrow.
    fn paint_control(
        &self,
        ui: &mut egui::Ui,
        close: egui::Rect,
        response: &egui::Response,
        native_focus: bool,
    ) -> Result<(), DialogUiError> {
        let operands = close::Operands::resolve(self.presentation, response, native_focus)?;
        let now = ui.input(|input| input.time);
        let paint = {
            let mut mounted = self.state.transitions.borrow_mut();
            mounted
                .get_or_insert_with(|| close::CloseTransitions::new(&operands))
                .sample(now, &operands)
        };
        paint.draw(
            ui,
            close,
            self.presentation,
            &mut self.state.ring.borrow_mut(),
        )
    }
}
/// Paint one retained shell without borrowing its live props across effects.
pub(super) fn draw<F: ?Sized>(
    state: &Rc<super::dialog_base::State>,
    i18n: &crate::I18n<F>,
    presentation: &BrandPresentation<'_>,
) -> Result<(), DialogUiError> {
    let mount = state.mount.get();
    let props = state.props.borrow().clone();
    let name = i18n.text("Close")?;
    let context = state.context.drawing();
    let screen = context.content_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, state.id);
    context.memory_mut(|memory| memory.set_modal_layer(layer));
    let result = egui::Area::new(state.id)
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .fade_in(false)
        .show(context, |area| {
            area.set_min_size(screen.size());
            // The viewport is known; do not hide its first paint behind Area's sizing pass.
            let mut ui = egui::Ui::new(
                context.clone(),
                state.id.with("shell"),
                egui::UiBuilder::new().layer_id(layer).max_rect(screen),
            );
            Placement {
                state,
                mount,
                props,
                presentation,
            }
            .render(&mut ui, &name)
        })
        .inner?;
    if result {
        state.close(true)?;
    }
    Ok(())
}

/// Literal heading and optional description operands.
pub struct DialogHeaderProps {
    /// Heading text, including an empty heading.
    pub title: String,
    /// Only absent or empty descriptions omit the paragraph.
    pub description: Option<String>,
    /// Literal wrapper classes.
    pub class_name: String,
}
/// Ordered native content operands.
pub struct DialogContentProps {
    /// Owned arbitrary native children.
    pub children: Vec<Rc<ReadingContent>>,
    /// Literal wrapper classes.
    pub class_name: String,
}
/// Retain a literal native heading factory.
#[must_use]
pub fn dialog_header(props: DialogHeaderProps) -> Rc<ReadingContent> {
    #[cfg(not(target_arch = "wasm32"))]
    drop(props);
    Rc::new(ReadingContent {
        #[cfg(target_arch = "wasm32")]
        mount: Rc::new(move |document| browser::header(document, &props)),
    })
}
/// Retain ordered mixed native children with their mounted guards.
#[must_use]
pub fn dialog_content(props: DialogContentProps) -> Rc<ReadingContent> {
    #[cfg(not(target_arch = "wasm32"))]
    drop(props);
    Rc::new(ReadingContent {
        #[cfg(target_arch = "wasm32")]
        mount: Rc::new(move |document| browser::content(document, &props)),
    })
}
#[cfg(target_arch = "wasm32")]
pub(super) mod browser;

/// Source control extent measured from the panel's inner border.
fn close_rect(panel: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(panel.right() - 33., panel.top() + 17.),
        egui::vec2(16., 16.),
    )
}

/// Disjoint outside regions preserve child/content click propagation.
fn outside_rects(screen: egui::Rect, rect: egui::Rect) -> [egui::Rect; 4] {
    [
        egui::Rect::from_min_max(screen.min, egui::pos2(screen.right(), rect.top())),
        egui::Rect::from_min_max(egui::pos2(screen.left(), rect.bottom()), screen.max),
        egui::Rect::from_min_max(
            egui::pos2(screen.left(), rect.top()),
            egui::pos2(rect.left(), rect.bottom()),
        ),
        egui::Rect::from_min_max(
            egui::pos2(rect.right(), rect.top()),
            egui::pos2(screen.right(), rect.bottom()),
        ),
    ]
}

/// Mounted Close property values and shadow composition.
pub(super) mod close;

/// Fractional raster coverage for Close decoration.
pub(super) mod ring;

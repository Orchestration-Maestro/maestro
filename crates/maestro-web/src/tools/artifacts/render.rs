//! Panel shell, ordered tabs and selected-view drawing.
use super::{ArtifactPresentation, ArtifactView, ArtifactsPanel, AttachmentUiError, with_element};
use crate::browser::attachment_content::{color, ghost, points, style};
impl ArtifactsPanel {
    /// Draw retained views using the caller's current map and selected filename.
    /// # Errors
    /// Returns localization, view, borrow, branding or native host failures.
    pub fn render<P>(
        &self,
        ui: &mut egui::Ui,
        caller: &maestro_app::browser::tools::artifacts::ArtifactsPanel<P>,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError>
    where
        P: Clone + std::ops::Deref,
        P::Target: ArtifactPresentation,
    {
        *self.context.borrow_mut() = Some(ui.ctx().clone());
        let records: Vec<_> = caller
            .artifacts()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        let names: Vec<_> = records
            .iter()
            .map(|record| {
                record
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .filename
                    .clone()
            })
            .collect();
        let active = caller.active_filename();
        let shown = self.connected.get() && !self.collapsed() && !names.is_empty();
        self.visibility(active.as_deref(), shown)?;
        if !shown {
            let mut hidden = ui.new_child(egui::UiBuilder::new().invisible().disabled());
            self.header(&mut hidden, active.as_deref(), view)?;
            return Ok(());
        }
        let selected = if self.overlay() {
            egui::Modal::new(self.id)
                .area(
                    egui::Modal::default_area(self.id)
                        .fade_in(false)
                        .anchor(egui::Align2::LEFT_TOP, egui::Vec2::ZERO),
                )
                .frame(egui::Frame::NONE)
                .backdrop_color(egui::Color32::TRANSPARENT)
                .show(ui.ctx(), |ui| {
                    ui.set_min_size(ui.ctx().content_rect().size());
                    ui.set_max_size(ui.ctx().content_rect().size());
                    self.shell(ui, &names, active.as_deref(), view)
                })
                .inner?
        } else {
            self.shell(ui, &names, active.as_deref(), view)?
        };
        if let Some(filename) = selected {
            caller.show_artifact(&filename);
        }
        Ok(())
    }
    /// Evaluate only a truthy selected installed view's header.
    fn header(
        &self,
        ui: &mut egui::Ui,
        active: Option<&str>,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        if let Some(element) = active
            .filter(|name| !name.is_empty())
            .and_then(|name| self.element(name))
        {
            with_element(&element, |element| element.get_header_buttons(ui, view))?;
        }
        Ok(())
    }
    /// Draw the bounded shell and return an activation after child borrows end.
    fn shell(
        &self,
        ui: &mut egui::Ui,
        names: &[String],
        active: Option<&str>,
        view: ArtifactView<'_>,
    ) -> Result<Option<String>, AttachmentUiError> {
        style(ui, view.presentation)?;
        let bounds = ui.available_rect_before_wrap();
        self.background(ui, bounds, view)?;
        let header = egui::Rect::from_min_size(bounds.min, egui::vec2(bounds.width(), 40.));
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        bar.spacing_mut().item_spacing.x = points(view.presentation.spacing["xs"]);
        let close = ghost(&mut bar, view.presentation, "X")?;
        let title = view.i18n.text("Close artifacts")?;
        close.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, bar.is_enabled(), &title)
        });
        let close = close.on_hover_text(title);
        self.header(&mut bar, active, view)?;
        let tab_bounds = bar.available_rect_before_wrap();
        let mut tab_ui = bar.new_child(
            egui::UiBuilder::new()
                .max_rect(tab_bounds)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let clicked = tabs(self, &mut tab_ui, names, active, view)?;
        ui.painter().hline(
            header.x_range(),
            header.bottom(),
            egui::Stroke::new(1., color(view.presentation, "border")?),
        );
        if let Some(element) = active.and_then(|name| self.element(name)) {
            let rect =
                egui::Rect::from_min_max(egui::pos2(bounds.left(), header.bottom()), bounds.max);
            let mut body = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            body.set_clip_rect(rect.intersect(ui.clip_rect()));
            with_element(&element, |element| element.render(&mut body, view))?;
        }
        ui.advance_cursor_after_rect(bounds);
        if close.clicked() {
            let callback = self.on_close.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(clicked)
    }
    /// Draw opaque panel paint and two visible shadow casters, never a backdrop.
    fn background(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        let border = color(view.presentation, "border")?;
        for (offset, blur, shrink) in [(1, 3, 0.), (8, 10, 1.)] {
            ui.painter().add(
                egui::epaint::Shadow {
                    offset: [0, offset],
                    blur,
                    spread: 0,
                    color: border.gamma_multiply(0.1),
                }
                .as_shape(rect.shrink(shrink), 0.),
            );
        }
        ui.painter()
            .rect_filled(rect, 0., color(view.presentation, "background")?);
        if !self.overlay() {
            ui.painter()
                .vline(rect.left(), rect.y_range(), egui::Stroke::new(1., border));
        }
        Ok(())
    }
}
/// Scroll literal ordered labels and center the first matching requested tab.
fn tabs(
    panel: &ArtifactsPanel,
    ui: &mut egui::Ui,
    names: &[String],
    active: Option<&str>,
    view: ArtifactView<'_>,
) -> Result<Option<String>, AttachmentUiError> {
    let request = panel.scroll.borrow_mut().take();
    let mut clicked = None;
    let mut scrolled = false;
    egui::ScrollArea::horizontal()
        .id_salt(panel.id.with("tabs"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.;
            for name in names {
                let response = tab_button(ui, name, active == Some(name.as_str()), view)?;
                if response.clicked() {
                    clicked = Some(name.clone());
                }
                if !scrolled && request.as_deref() == Some(name.as_str()) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    scrolled = true;
                }
            }
            Ok::<_, AttachmentUiError>(clicked)
        })
        .inner
}
/// Draw one literal tab with caller-owned active, hover and focus roles.
fn tab_button(
    ui: &mut egui::Ui,
    name: &str,
    selected: bool,
    view: ArtifactView<'_>,
) -> Result<egui::Response, AttachmentUiError> {
    ui.scope(|ui| {
        let accent = color(view.presentation, "accent")?;
        let resting = color(view.presentation, if selected { "accent" } else { "muted" })?;
        let hovered = color(view.presentation, if selected { "accent" } else { "text" })?;
        ui.visuals_mut().widgets.inactive.fg_stroke.color = resting;
        ui.visuals_mut().widgets.hovered.fg_stroke.color = hovered;
        ui.visuals_mut().widgets.active.fg_stroke.color = hovered;
        ui.spacing_mut().button_padding = egui::vec2(
            points(view.presentation.spacing["md"]),
            points(view.presentation.spacing["sm"]),
        );
        let button = egui::Button::new(egui::RichText::new(name).monospace().small())
            .frame(false)
            .min_size(egui::vec2(0., 40.))
            .wrap_mode(egui::TextWrapMode::Extend);
        let response = ui.add(button);
        if selected {
            ui.painter().hline(
                response.rect.x_range(),
                response.rect.bottom() - 1.,
                egui::Stroke::new(2., accent),
            );
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect,
                0.,
                egui::Stroke::new(1., accent),
                egui::StrokeKind::Inside,
            );
        }
        Ok(response)
    })
    .inner
}

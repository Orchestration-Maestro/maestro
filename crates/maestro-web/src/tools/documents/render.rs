//! Panel shell, ordered tabs and selected-view drawing.
use super::{ArtifactPresentation, ArtifactView, ArtifactsPanel, with_element};
use crate::browser::attachment_content::{GHOST_SIZE, color, ghost, points, style};
use crate::tools::documents::ArtifactUiError;
impl ArtifactsPanel {
    /// Draw retained views using the caller's current map and selected filename.
    /// # Errors
    /// Returns localization, view, borrow, branding or native host failures.
    pub fn render<P>(
        &self,
        ui: &mut egui::Ui,
        caller: &maestro_app::browser::tools::documents::ArtifactsPanel<P>,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError>
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
    ) -> Result<(), ArtifactUiError> {
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
    ) -> Result<Option<String>, ArtifactUiError> {
        style(ui, view.presentation)
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        let bounds = ui.available_rect_before_wrap();
        self.background(ui, bounds, view)?;
        let content_left = bounds.left() + if self.overlay() { 0. } else { 1. };
        let header = egui::Rect::from_min_max(
            egui::pos2(content_left, bounds.top()),
            egui::pos2(bounds.right(), bounds.top() + 40.),
        );
        let width_id = self.id.with("controls-width");
        let spacing = points(view.presentation.spacing["xs"]);
        let minimum = GHOST_SIZE.x + spacing + crate::components::download_button::SIZE.x;
        let width = ui.ctx().data_mut(|data| {
            let (name, measured) =
                data.get_temp_mut_or::<(Option<String>, f32)>(width_id, (None, minimum));
            if name.as_deref() == active {
                measured.max(minimum)
            } else {
                minimum
            }
        });
        let controls_left = (header.right() - width).max(header.left());
        let mut tab_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header.with_max_x(controls_left))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let scroll = self.scroll.borrow().clone();
        let clicked = tabs(self, &mut tab_ui, names, active, view)?;
        let (close, measured) =
            self.controls(ui, header.with_min_x(controls_left), active, view)?;
        ui.ctx()
            .data_mut(|data| data.insert_temp(width_id, (active.map(str::to_owned), measured)));
        if (measured.max(minimum) - width).abs() > 0.5 {
            *self.scroll.borrow_mut() = scroll;
            ui.ctx()
                .request_discard("artifact header controls changed width");
        }
        let border = color(view.presentation, "border")
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        ui.painter().hline(
            header.x_range(),
            header.bottom() + 0.5,
            egui::Stroke::new(1., border),
        );
        self.draw_body(
            ui,
            active,
            (content_left, header.bottom() + 1., bounds.max),
            view,
        )?;
        ui.advance_cursor_after_rect(bounds);
        if close.clicked() {
            let callback = self.on_close.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(clicked)
    }
    /// Draw the selected retained view below the header.
    fn draw_body(
        &self,
        ui: &mut egui::Ui,
        active: Option<&str>,
        bounds: (f32, f32, egui::Pos2),
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        if let Some(element) = active.and_then(|name| self.element(name)) {
            let rect = egui::Rect::from_min_max(egui::pos2(bounds.0, bounds.1), bounds.2);
            let mut body = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            body.set_clip_rect(rect.intersect(ui.clip_rect()));
            with_element(&element, |element| element.render(&mut body, view))?;
        }
        Ok(())
    }
    /// Draw caller controls once per toolkit pass, followed by the close control.
    fn controls(
        &self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        active: Option<&str>,
        view: ArtifactView<'_>,
    ) -> Result<(egui::Response, f32), ArtifactUiError> {
        let gap = points(view.presentation.spacing["xs"]);
        let close_rect = rect.with_min_x((rect.right() - GHOST_SIZE.x).max(rect.left()));
        let header_rect = rect.with_max_x((close_rect.left() - gap).max(rect.left()));
        let mut bar = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        bar.set_clip_rect(header_rect.intersect(ui.clip_rect()));
        bar.spacing_mut().item_spacing.x = gap;
        self.header(&mut bar, active, view)?;
        let measured = bar.min_rect().width() + gap + GHOST_SIZE.x;
        let mut close_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(close_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let close = ghost(&mut close_ui, view.presentation, "X")?;
        let title = view.i18n.text("Close artifacts")?;
        close.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, bar.is_enabled(), &title)
        });
        Ok((close.on_hover_text(title), measured))
    }
    /// Draw opaque panel paint and two visible shadow casters, never a backdrop.
    fn background(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        let border = color(view.presentation, "border")
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        crate::browser::attachment_content::panel_shadow(ui, rect, 0., border);
        ui.painter().rect_filled(
            rect,
            0.,
            color(view.presentation, "background")
                .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?,
        );
        if !self.overlay() {
            ui.painter().vline(
                rect.left() + 0.5,
                rect.y_range(),
                egui::Stroke::new(1., border),
            );
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
) -> Result<Option<String>, ArtifactUiError> {
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
            Ok::<_, ArtifactUiError>(clicked)
        })
        .inner
}
/// Draw one literal tab with caller-owned active, hover and focus roles.
fn tab_button(
    ui: &mut egui::Ui,
    name: &str,
    selected: bool,
    view: ArtifactView<'_>,
) -> Result<egui::Response, ArtifactUiError> {
    ui.scope(|ui| {
        let accent = color(view.presentation, "accent")
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        let resting = color(view.presentation, if selected { "accent" } else { "muted" })
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        let hovered = color(view.presentation, if selected { "accent" } else { "text" })
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        ui.visuals_mut().widgets.inactive.fg_stroke.color = resting;
        ui.visuals_mut().widgets.hovered.fg_stroke.color = hovered;
        ui.visuals_mut().widgets.active.fg_stroke.color = hovered;
        ui.spacing_mut().button_padding = egui::vec2(
            points(view.presentation.spacing["md"]),
            points(view.presentation.spacing["sm"]),
        );
        let display = name
            .split([' ', '\t', '\n', '\r', '\u{c}'])
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let label = egui::RichText::new(display)
            .monospace()
            .size(egui::TextStyle::Small.resolve(ui.style()).size);
        let button = egui::Button::new(label)
            .frame(true)
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
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

#[cfg(test)]
mod tests;

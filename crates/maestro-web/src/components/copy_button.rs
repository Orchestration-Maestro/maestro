//! Retained original-text copy control with success feedback.
use crate::{
    I18n, I18nError,
    browser::{
        attachment_content::{self, AttachmentHost, AttachmentUiError},
        copy_feedback::CopyFeedback,
    },
};
use std::rc::Rc;

/// Copy arbitrary original text through the supplied browser effects.
pub struct CopyButton {
    /// Original text captured synchronously at activation.
    pub text: String,
    /// Tooltip and accessible name; an explicit empty title stays empty.
    pub title: String,
    /// Show the localized success text alongside the check glyph.
    pub show_text: bool,
    /// Replaceable platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable slot and canvas identity.
    pub(crate) id: egui::Id,
    /// Weak completion and cancel-on-drop feedback.
    feedback: CopyFeedback,
    /// Primary gesture admitted in this control's visible slot.
    pressed: Option<()>,
    /// Restoring interval for this retained control.
    feedback_duration: i32,
    /// Console controls use muted idle and accent hover roles.
    pub(crate) hover: attachment_content::GhostHover,
}
impl CopyButton {
    /// Retain the host and initialize empty text with the localized copy title.
    /// # Errors
    /// Returns a localization failure for the default title.
    pub fn new<F: ?Sized>(host: Rc<dyn AttachmentHost>, i18n: &I18n<F>) -> Result<Self, I18nError> {
        Ok(Self {
            text: String::new(),
            title: i18n.text("Copy")?,
            show_text: false,
            host,
            id: attachment_content::identity(),
            feedback: CopyFeedback::default(),
            pressed: None,
            feedback_duration: 2000,
            hover: attachment_content::GhostHover::Text,
        })
    }
    /// Select the restoring interval without adding another timer owner.
    pub(crate) fn set_feedback_duration(&mut self, delay: i32) {
        self.feedback_duration = delay;
    }
    /// Whether successful copy feedback is currently active.
    pub(crate) fn copied(&self) -> bool {
        self.feedback.copied()
    }
    /// Draw and activate the original-text control.
    /// # Errors
    /// Returns reached localization, glyph or selected-pack presentation errors.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<egui::Response, AttachmentUiError> {
        let rect = ui.allocate_space(self.size(ui, i18n)?).1;
        self.render_at(ui, rect, i18n, presentation)
    }
    /// Measure the same glyph and optional success text used for painting.
    pub(crate) fn size<F: ?Sized>(
        &self,
        ui: &egui::Ui,
        i18n: &I18n<F>,
    ) -> Result<egui::Vec2, I18nError> {
        let mut size = attachment_content::GHOST_SIZE;
        if self.feedback.copied() && self.show_text {
            let text = i18n.text("Copied!")?;
            size.x += ui
                .painter()
                .layout_no_wrap(
                    text,
                    egui::TextStyle::Button.resolve(ui.style()),
                    egui::Color32::WHITE,
                )
                .size()
                .x;
        }
        Ok(size)
    }
    /// Paint a full measured slot; only its visible intersection admits pointer endpoints.
    pub(crate) fn render_at<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<egui::Response, AttachmentUiError> {
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(self.id).max_rect(rect));
        attachment_content::style(&mut child, presentation)?;
        let copied = self.feedback.copied();
        let label = (copied && self.show_text)
            .then(|| i18n.text("Copied!"))
            .transpose()?;
        let response = attachment_content::ghost_labeled_tinted(
            &mut child,
            presentation,
            (if copied { "Check" } else { "Copy" }, label.as_deref()),
            rect.size(),
            (
                attachment_content::color(
                    presentation,
                    match self.hover {
                        attachment_content::GhostHover::Text => "text",
                        attachment_content::GhostHover::Accent => "muted",
                    },
                )?,
                self.hover,
            ),
        )?
        .on_hover_text(&self.title);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &self.title)
        });
        let visible = rect.intersect(ui.clip_rect());
        let slots = if response.enabled() && visible.is_positive() {
            vec![((), visible, response.layer_id)]
        } else {
            vec![]
        };
        let events = ui.input(|input| input.events.clone());
        let pointer = !attachment_content::reading::controls::pointer_actions(
            &events,
            &mut self.pressed,
            &slots,
            ui.ctx(),
        )
        .is_empty();
        if pointer || (response.clicked() && !response.clicked_by(egui::PointerButton::Primary)) {
            self.feedback.copy(
                &self.host,
                (&self.text, attachment_content::ClipboardMode::Direct),
                self.feedback_duration,
                ui.ctx(),
            );
        }
        Ok(response)
    }
}

//! Literal console output with retained native scrolling and owned copy feedback.
use crate::components::code_block::ReadingUiError;
use crate::{
    I18n,
    browser::attachment_content::{
        self, AttachmentHost, ReadingContent,
        reading::controlled::{ControlledReading, ReadingHeight},
    },
    components::{code_block, copy_button::CopyButton},
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
mod browser;

/// Console output's semantic color.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum ConsoleVariant {
    /// Ordinary tool output.
    #[default]
    Default,
    /// Failed tool output.
    Error,
}
/// Complete native update operands, excluding presentation-only changes.
#[derive(PartialEq, Eq)]
struct Prepared {
    /// Literal preformatted text.
    content: String,
    /// Selected text role.
    variant: ConsoleVariant,
    /// Localized native header text.
    label: String,
    /// Copy feedback participates in actual console updates.
    copied: bool,
}
/// Console body and its independent retained platform lifetime.
pub struct ConsoleBlock {
    /// Literal output, never parsed as markup or encoded text.
    pub content: String,
    /// Selected semantic text color.
    pub variant: ConsoleVariant,
    /// Replaceable native effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Successfully published native operands.
    prepared: Option<Rc<Prepared>>,
    /// Successfully published factory.
    reading: Option<Rc<ReadingContent>>,
    /// Weak mounted owner, never a factory backedge.
    mounted: Rc<RefCell<Weak<browser::Mounted>>>,
    /// Shared canvas copy control and restoring timer.
    copy: Option<CopyButton>,
}
impl ConsoleBlock {
    /// Start with empty ordinary output using the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            content: String::new(),
            variant: ConsoleVariant::Default,
            host,
            id: attachment_content::identity(),
            prepared: None,
            reading: None,
            mounted: Rc::default(),
            copy: None,
        }
    }
    /// Draw the native console and its canvas copy control.
    /// # Errors
    /// Returns reached localization, glyph, presentation or native placement failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        presentation: &maestro_theme::BrandPresentation<'_>,
        modal: Option<egui::Id>,
    ) -> Result<egui::Response, ReadingUiError> {
        let mut copy = match self.copy.take() {
            Some(copy) => copy,
            None => CopyButton::new(Rc::clone(&self.host), i18n)?,
        };
        let result = self.draw(ui, &mut copy, (i18n, presentation, modal));
        self.copy = Some(copy);
        result
    }
    /// Draw with a retained control, restoring it even when preparation fails.
    fn draw(
        &mut self,
        ui: &mut egui::Ui,
        copy: &mut CopyButton,
        (i18n, presentation, modal): (
            &I18n,
            &maestro_theme::BrandPresentation<'_>,
            Option<egui::Id>,
        ),
    ) -> Result<egui::Response, ReadingUiError> {
        attachment_content::style(ui, presentation).map_err(ReadingUiError::Presentation)?;
        copy.compact = true;
        copy.set_feedback_duration(1500);
        copy.hover = attachment_content::GhostHover::Accent;
        copy.title = i18n.text("Copy output")?;
        copy.show_text = true;
        copy.text.clone_from(&self.content);
        let next = Rc::new(Prepared {
            content: self.content.clone(),
            variant: self.variant,
            label: i18n.text("console")?,
            copied: copy.copied(),
        });
        let size = copy.size(ui, i18n)?;
        let layout = self.publish(ui, next, (copy.id, size), (presentation, modal))?;
        let controls = code_block::controls(ui, &layout, [copy], (i18n, presentation))?;
        let gained = controls.iter().flatten().any(egui::Response::gained_focus);
        if let Some(mounted) = self.mounted.borrow().upgrade().filter(|_| gained) {
            mounted.reveal();
        }
        Ok(layout.response)
    }
    /// Publish a staged native update before scrolling its own retained body.
    fn publish(
        &mut self,
        ui: &mut egui::Ui,
        next: Rc<Prepared>,
        control: (egui::Id, egui::Vec2),
        (presentation, modal): (&maestro_theme::BrandPresentation<'_>, Option<egui::Id>),
    ) -> Result<attachment_content::reading::controlled::ControlledLayout, ReadingUiError> {
        let changed = self.prepared.as_ref().is_none_or(|old| **old != *next);
        let mounted = Rc::default();
        let candidate = changed.then(|| browser::content(&next, &mounted, control.0));
        let reading = candidate
            .as_ref()
            .or(self.reading.as_ref())
            .ok_or_else(|| {
                ReadingUiError::Host(
                    crate::browser::attachment_content::AttachmentHostError::Host(
                        "Missing console".into(),
                    ),
                )
            })?;
        let result = ControlledReading {
            intrinsic_width: false,
            height: ReadingHeight::Flow,
            host: self.host.as_ref(),
            id: self.id,
            content: reading,
            sizes: &[control],
            modal,
            presentation,
        }
        .draw(ui)?;
        if let Some(reading) = candidate {
            self.reading = Some(reading);
            self.prepared = Some(next);
            self.mounted = mounted;
            if let Some(mounted) = self.mounted.borrow().upgrade() {
                mounted.scroll_bottom();
            }
        }
        Ok(result)
    }
}
impl Drop for ConsoleBlock {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}

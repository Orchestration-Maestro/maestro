//! Retained single-line browser input.
use crate::I18nError;
use crate::browser::attachment_content::AttachmentHostError;
use crate::{
    I18n,
    browser::attachment_content::{
        AttachmentContent, AttachmentHost, AttachmentSurface, identity, style,
    },
};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, rc::Rc};
use web_sys::{Event, HtmlInputElement, KeyboardEvent};
/// Failure from the retained input operation.
#[derive(Debug)]
pub enum InputUiError {
    /// Localization failed.
    I18n(I18nError),
    /// Native operation failed.
    Host(AttachmentHostError),
    /// Selected presentation could not be prepared.
    Presentation(String),
}
impl std::fmt::Display for InputUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I18n(error) => std::fmt::Display::fmt(error, f),
            Self::Host(error) => std::fmt::Display::fmt(error, f),
            Self::Presentation(error) => std::fmt::Display::fmt(error, f),
        }
    }
}
impl std::error::Error for InputUiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::I18n(error) => Some(error),
            Self::Host(error) => Some(error),
            Self::Presentation(_) => None,
        }
    }
}
impl From<I18nError> for InputUiError {
    fn from(error: I18nError) -> Self {
        Self::I18n(error)
    }
}
impl From<AttachmentHostError> for InputUiError {
    fn from(error: AttachmentHostError) -> Self {
        Self::Host(error)
    }
}
/// Accepted native input modes.
#[derive(Clone, Copy, Default)]
pub enum InputType {
    /// Plain text.
    #[default]
    Text,
    /// Electronic mail.
    Email,
    /// Masked text.
    Password,
    /// Native number.
    Number,
    /// Web address.
    Url,
    /// Telephone.
    Tel,
    /// Search text.
    Search,
}
/// Input dimensions.
#[derive(Clone, Copy, Default)]
pub enum InputSize {
    /// Small.
    Sm,
    /// Medium.
    #[default]
    Md,
    /// Large.
    Lg,
}
/// Caller-retained native reference slot.
pub type InputRef = Rc<RefCell<Option<HtmlInputElement>>>;
/// Callback with the actual browser event.
pub type EventCallback = Rc<dyn Fn(Event)>;
/// Callback with the actual keyboard event.
pub type KeyCallback = Rc<dyn Fn(KeyboardEvent)>;
/// Owned input operands.
#[derive(Default)]
pub struct InputProps {
    /// Native mode.
    pub input_type: InputType,
    /// Dimensions.
    pub size: InputSize,
    /// Literal text.
    pub value: String,
    /// Empty-text hint.
    pub placeholder: String,
    /// Optional label.
    pub label: String,
    /// Optional error.
    pub error: String,
    /// Disabled property.
    pub disabled: bool,
    /// Required property.
    pub required: bool,
    /// Native form name.
    pub name: String,
    /// Native autocomplete setting.
    pub autocomplete: String,
    /// Lower bound.
    pub min: Option<f64>,
    /// Upper bound.
    pub max: Option<f64>,
    /// Native numeric step.
    pub step: Option<f64>,
    /// Optional native reference slot.
    pub input_ref: Option<InputRef>,
    /// Input callback.
    pub on_input: Option<EventCallback>,
    /// Change callback.
    pub on_change: Option<EventCallback>,
    /// Key-down callback.
    pub on_key_down: Option<KeyCallback>,
    /// Key-up callback.
    pub on_key_up: Option<KeyCallback>,
    /// Outer wrapper classes.
    pub class_name: String,
}
/// An independently retained single-line input.
pub struct Input {
    /// Live configuration for the next presentation.
    props: InputProps,
    /// Native element owner.
    host: Rc<dyn AttachmentHost>,
    /// Stable placement identity.
    id: egui::Id,
    /// Last successfully supplied value, not native edited text.
    previous_value: Option<String>,
}
impl Input {
    /// Retain configuration and its native host.
    #[must_use]
    pub fn new(props: InputProps, host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            props,
            host,
            id: identity(),
            previous_value: None,
        }
    }
    /// Replace the operands consumed by the next render.
    pub fn set_props(&mut self, props: InputProps) {
        self.props = props;
    }
    /// Allocate the field and present its live native operands.
    /// # Errors
    /// Propagates localization, brand and native presentation failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &BrandPresentation<'_>,
    ) -> Result<(), InputUiError> {
        style(ui, presentation).map_err(|error| InputUiError::Presentation(error.to_string()))?;
        ui.scope_builder(
            egui::UiBuilder::new().layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| self.render_contents(ui, i18n, presentation),
        )
        .inner
    }
    /// Stack one field's label, native control and error inside its own column.
    fn render_contents<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &BrandPresentation<'_>,
    ) -> Result<(), InputUiError> {
        ui.spacing_mut().item_spacing.y = 6.;
        let mut accessible_label = self.props.label.clone();
        let label = |ui: &mut egui::Ui| -> Result<(), InputUiError> {
            ui.label(&self.props.label);
            if self.props.required {
                let marker = i18n.text("*")?;
                accessible_label.push(' ');
                accessible_label.push_str(&marker);
                ui.colored_label(
                    crate::browser::attachment_content::color(presentation, "error")
                        .map_err(|error| InputUiError::Presentation(error.to_string()))?,
                    marker,
                );
            }
            Ok(())
        };
        if !self.props.label.is_empty() {
            ui.horizontal(label).inner?;
        }
        let height = match self.props.size {
            InputSize::Sm => 32.,
            InputSize::Md => 36.,
            InputSize::Lg => 40.,
        };
        let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), height));
        self.host.present(
            AttachmentSurface {
                id: self.id,
                rect,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: Vec::new(),
                content: AttachmentContent::Input {
                    props: &self.props,
                    accessible_label: &accessible_label,
                    value_changed: self.previous_value.as_ref() != Some(&self.props.value),
                    modal: (ui.layer_id().order == egui::Order::Foreground)
                        .then_some(ui.layer_id().id),
                    context: ui.ctx(),
                },
            },
            presentation,
        )?;
        self.previous_value = Some(self.props.value.clone());
        if !self.props.error.is_empty() {
            ui.colored_label(
                crate::browser::attachment_content::color(presentation, "error")
                    .map_err(|error| InputUiError::Presentation(error.to_string()))?,
                &self.props.error,
            );
        }
        Ok(())
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}

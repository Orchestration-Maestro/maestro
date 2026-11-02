//! Native button element with variant and size classes.
use wasm_bindgen::{JsCast as _, JsValue};
/// Visual emphasis of a button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action.
    #[default]
    Default,
    /// Destructive action.
    Destructive,
    /// Bordered action.
    Outline,
    /// Secondary action.
    Secondary,
    /// Borderless action.
    Ghost,
    /// Link-styled action.
    Link,
}
/// Size of a button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    /// Small.
    Sm,
    /// Medium.
    #[default]
    Md,
    /// Large.
    Lg,
    /// Square icon.
    Icon,
}
/// Form behavior of a button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonType {
    /// Plain action.
    #[default]
    Button,
    /// Submits its form.
    Submit,
    /// Resets its form.
    Reset,
}
/// Button operands; click handling belongs to the caller's lifecycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonProps<'a> {
    /// Visual emphasis.
    pub variant: ButtonVariant,
    /// Size.
    pub size: ButtonSize,
    /// Whether activation is refused.
    pub disabled: bool,
    /// Form behavior.
    pub r#type: ButtonType,
    /// Whether a pending loader replaces activation.
    pub loading: bool,
    /// Optional native title.
    pub title: Option<&'a str>,
    /// Caller classes appended after the variant and size classes.
    pub class_name: &'a str,
}
impl ButtonProps<'_> {
    /// Space-separated classes for the selected variant and size, then the caller's.
    pub(crate) fn classes(&self) -> String {
        let variant = match self.variant {
            ButtonVariant::Default => "default",
            ButtonVariant::Destructive => "destructive",
            ButtonVariant::Outline => "outline",
            ButtonVariant::Secondary => "secondary",
            ButtonVariant::Ghost => "ghost",
            ButtonVariant::Link => "link",
        };
        let size = match self.size {
            ButtonSize::Sm => "sm",
            ButtonSize::Md => "md",
            ButtonSize::Lg => "lg",
            ButtonSize::Icon => "icon",
        };
        let classes = format!("maestro-button maestro-button-{variant} maestro-button-{size}");
        match self.class_name {
            "" => classes,
            extra => format!("{classes} {extra}"),
        }
    }
}
/// Create the native button with its children.
/// # Errors
/// Returns the native creation failure.
pub fn button(
    document: &web_sys::Document,
    props: &ButtonProps<'_>,
    children: &[web_sys::Node],
) -> Result<web_sys::HtmlButtonElement, JsValue> {
    let button: web_sys::HtmlButtonElement = document.create_element("button")?.dyn_into()?;
    button.set_type(match props.r#type {
        ButtonType::Button => "button",
        ButtonType::Submit => "submit",
        ButtonType::Reset => "reset",
    });
    button.set_class_name(&props.classes());
    button.set_disabled(props.disabled || props.loading);
    if let Some(title) = props.title {
        button.set_title(title);
    }
    if props.loading {
        let node = crate::icons::icon_node("Loader")
            .map_err(|error| JsValue::from_str(&error.to_string()))?
            .ok_or_else(|| JsValue::from_str("Missing icon Loader"))?;
        let spinner = crate::icons::icon_dom(
            &node,
            crate::icons::IconSize::Sm,
            Some("maestro-button-spinner"),
        )?;
        button.append_child(&spinner)?;
    }
    for child in children {
        button.append_child(child)?;
    }
    Ok(button)
}

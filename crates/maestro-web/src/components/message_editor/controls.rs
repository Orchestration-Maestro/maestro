//! Construction of the editor's icon, model and reasoning controls.
use super::{THINKING_LEVELS, level_value};
use crate::{
    browser::attachment_content::browser::{append, element, host_error},
    components::button::{ButtonProps, ButtonSize, ButtonVariant, button},
    dom::{DomView, icon},
    tools::types::ToolUiError,
};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlButtonElement, HtmlSelectElement};

/// A ghost square button showing the catalog icon `name`.
pub(super) fn icon_button(
    document: &web_sys::Document,
    name: &str,
    class: &str,
) -> Result<HtmlButtonElement, ToolUiError> {
    let glyph = icon(document, name, "")?;
    button(
        document,
        &ButtonProps {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Icon,
            class_name: class,
            ..ButtonProps::default()
        },
        &[glyph.into()],
    )
    .map_err(|error| host_error(&error).into())
}
/// The Send control, its arrow turned to point up and right.
pub(super) fn send_control(document: &web_sys::Document) -> Result<HtmlButtonElement, ToolUiError> {
    let send = icon_button(document, "Send", "maestro-editor-send")?;
    if let Some(glyph) = send.first_element_child() {
        glyph
            .class_list()
            .add_1("maestro-editor-send-glyph")
            .map_err(|error| host_error(&error))?;
    }
    Ok(send)
}
/// The model control and the label holding the model identifier.
pub(super) fn model_control(
    document: &web_sys::Document,
) -> Result<(HtmlButtonElement, Element), ToolUiError> {
    let label = element(document, "span", "maestro-editor-model-id")?;
    let glyph = icon(document, "Sparkles", "")?;
    let model = button(
        document,
        &ButtonProps {
            variant: ButtonVariant::Ghost,
            size: ButtonSize::Sm,
            class_name: "maestro-editor-model",
            ..ButtonProps::default()
        },
        &[glyph.into(), label.clone().into()],
    )
    .map_err(|error| host_error(&error))?;
    Ok((model, label))
}
/// The reasoning level menu with its localized options.
pub(super) fn level_menu(view: &DomView) -> Result<HtmlSelectElement, ToolUiError> {
    let document = &view.document;
    let menu: HtmlSelectElement = element(
        document,
        "select",
        "maestro-select maestro-select-sm maestro-editor-level",
    )?
    .unchecked_into();
    for (level, label) in THINKING_LEVELS
        .iter()
        .zip(["Off", "Minimal", "Low", "Medium", "High"])
    {
        let option = element(document, "option", "")?;
        option
            .set_attribute("value", level_value(level))
            .map_err(|error| host_error(&error))?;
        option.set_text_content(Some(&view.i18n.text(label)?));
        append(&menu, &option)?;
    }
    menu.set_attribute("aria-label", &view.i18n.text("Thinking")?)
        .map_err(|error| host_error(&error))?;
    Ok(menu)
}

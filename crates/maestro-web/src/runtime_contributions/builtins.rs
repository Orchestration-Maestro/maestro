//! Built-in element initialization shared by standalone chats and contribution hosts.
use super::slot;
use crate::page::DomView;
use std::rc::Rc;
use wasm_bindgen::JsValue;

/// Register the built-in elements and their styles once for the page.
/// # Errors
/// Returns a browser registration or presentation failure.
pub(crate) fn register_builtin_elements(view: &Rc<DomView>) -> Result<(), JsValue> {
    let registry = view
        .document
        .default_view()
        .ok_or_else(|| JsValue::from_str("No window"))?
        .custom_elements();
    if registry.get(slot::TAG).is_undefined() {
        crate::page::install_styles(view)?;
        crate::components::copy_button::dom::define(view)?;
        crate::components::source::dom::define(view)?;
        crate::components::markdown_block::MarkdownBlock::define(view)
            .and_then(|()| crate::ThinkingBlock::define(view))
            .and_then(|()| crate::tools::documents::html_artifact::define(view))
            .and_then(|()| crate::tools::documents::svg_artifact::define(view))
            .and_then(|()| crate::tools::documents::pdf_artifact::define(view))
            .and_then(|()| crate::tools::documents::native::define(view))
            .and_then(|()| crate::tools::documents::console::define(view))
            .and_then(|()| crate::tools::documents::panel::define(view))
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        crate::components::preview_code_toggle::dom::define(view)?;
        crate::components::mode_toggle::define(view)?;
        slot::define(view)?;
    }
    Ok(())
}

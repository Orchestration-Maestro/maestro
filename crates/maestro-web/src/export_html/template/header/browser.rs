//! Native literal header composition and locale binding.
use super::{HELP, LABELS, Prepared};
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{append, element as create, host_error},
};
use crate::browser::invoke0;
use std::io;
use web_sys::{Document, Element};
mod details;
/// Create a literal element without interpreting markup.
fn element(
    document: &Document,
    tag: &str,
    class: &str,
    text: &str,
) -> Result<Element, AttachmentHostError> {
    let e = create(document, tag, class)?;
    e.set_text_content(Some(text));
    Ok(e)
}
/// Mount literal metadata and empty mixed-content slots.
pub(super) fn mount(
    document: &Document,
    prepared: &Prepared,
    ids: [egui::Id; 3],
) -> Result<ReadingRoot, AttachmentHostError> {
    let root = element(document, "section", "maestro-session-header", "")?;
    let style = element(document, "style", "", include_str!("../header.css"))?;
    append(&root, &style)?;
    append(&root, &element(document, "h1", "", &prepared.title)?)?;
    let help = element(document, "div", "help-bar", "")?;
    append(&help, &element(document, "span", "help-hint", HELP)?)?;
    let actions = element(document, "div", "help-actions", "")?;
    let mut slots = Vec::new();
    for id in ids {
        let slot = element(document, "span", "control-slot", "")?;
        append(&actions, &slot)?;
        slots.push((id, slot));
    }
    append(&help, &actions)?;
    append(&root, &help)?;
    let info = element(document, "div", "header-info", "")?;
    for (label, value) in LABELS.iter().zip(&prepared.rows) {
        let row = element(document, "div", "info-item", "")?;
        append(&row, &element(document, "span", "info-label", label)?)?;
        append(&row, &element(document, "span", "info-value", value)?)?;
        append(&info, &row)?;
    }
    append(&root, &info)?;
    let flow = element(document, "div", "maestro-session-viewer-header", "")?;
    flow.set_attribute("style", "display:flow-root")
        .map_err(|e| host_error(&e))?;
    append(&flow, &root)?;
    let owner = details::mount(document, &flow, prepared)?;
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        commit: None,
        owner,
        element: flow,
        slots,
    })
}
/// Invoke the owning browser's native zero-argument locale operation.
/// # Errors
/// Returns invalid data for an unsupported timestamp operand.
pub fn localize_header_date(timestamp: &serde_json::Value) -> io::Result<String> {
    let value = match timestamp {
        serde_json::Value::String(text) => wasm_bindgen::JsValue::from_str(text),
        serde_json::Value::Number(number) => {
            wasm_bindgen::JsValue::from_f64(number.as_f64().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "nonnumeric timestamp")
            })?)
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported timestamp",
            ));
        }
    };
    let date = js_sys::Date::new(&value);
    let result =
        invoke0(&date, "toLocaleString").map_err(|e| io::Error::other(format!("{e:?}")))?;
    result
        .as_string()
        .ok_or_else(|| io::Error::other("nonstring localized date"))
}

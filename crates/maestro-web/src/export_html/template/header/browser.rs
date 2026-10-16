//! Native literal header composition and locale binding.
use super::{HELP, LABELS, Prepared};
use crate::browser::attachment_content::{AttachmentUiError, ReadingRoot, browser::host_error};
use std::io;
use web_sys::{Document, Element};
/// Create a literal element without interpreting markup.
fn element(
    document: &Document,
    tag: &str,
    class: &str,
    text: &str,
) -> Result<Element, AttachmentUiError> {
    let e = document.create_element(tag).map_err(|e| host_error(&e))?;
    e.set_attribute("class", class)
        .map_err(|e| host_error(&e))?;
    e.set_text_content(Some(text));
    Ok(e)
}
/// Append owned nodes through the native effect boundary.
fn append(parent: &Element, child: &Element) -> Result<(), AttachmentUiError> {
    parent.append_child(child).map_err(|e| host_error(&e))?;
    Ok(())
}
/// Mount literal metadata and empty mixed-content slots.
pub(super) fn mount(
    document: &Document,
    prepared: &Prepared,
    ids: [egui::Id; 3],
) -> Result<ReadingRoot, AttachmentUiError> {
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
    let flow = element(document, "div", "", "")?;
    flow.set_attribute("style", "display:flow-root")
        .map_err(|e| host_error(&e))?;
    append(&flow, &root)?;
    Ok(ReadingRoot {
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
    let result = js_sys::Reflect::get(&date, &"toLocaleString".into())
        .and_then(wasm_bindgen::JsCast::dyn_into::<js_sys::Function>)
        .and_then(|f| f.call0(&date))
        .map_err(|e| io::Error::other(format!("{e:?}")))?;
    result
        .as_string()
        .ok_or_else(|| io::Error::other("nonstring localized date"))
}

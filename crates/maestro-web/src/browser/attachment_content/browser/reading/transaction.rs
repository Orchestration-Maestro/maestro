//! One restoration owner for bounded measurements and atomic publication.
use super::{AttachmentUiError, ReadingPhase, attr, host_error};
use js_sys::Reflect;
use std::collections::HashMap;
use wasm_bindgen::JsValue;
use web_sys::Element;

/// Native fractional scroll offsets, without integer Element bindings.
pub(super) struct Scroll {
    /// Native horizontal offset.
    left: JsValue,
    /// Native vertical offset.
    top: JsValue,
}
impl Scroll {
    /// Observe both native axes before mutation.
    pub(super) fn read(viewport: &Element) -> Result<Self, AttachmentUiError> {
        Ok(Self {
            left: Reflect::get(viewport, &"scrollLeft".into()).map_err(|e| host_error(&e))?,
            top: Reflect::get(viewport, &"scrollTop".into()).map_err(|e| host_error(&e))?,
        })
    }
    /// Attempt both axes even when the first write fails.
    pub(super) fn apply(&self, viewport: &Element) -> Result<(), AttachmentUiError> {
        let left =
            Reflect::set(viewport, &"scrollLeft".into(), &self.left).map_err(|e| host_error(&e));
        let top =
            Reflect::set(viewport, &"scrollTop".into(), &self.top).map_err(|e| host_error(&e));
        left.and(top).map(|_| ())
    }
}
/// Size slots by identity, preserving native slot ownership.
pub(super) fn size_slots(
    slots: &[(egui::Id, Element)],
    sizes: &[(egui::Id, egui::Vec2)],
) -> Result<(), AttachmentUiError> {
    let sizes: HashMap<_, _> = sizes.iter().copied().collect();
    for (id, slot) in slots {
        if let Some(size) = sizes.get(id) {
            attr(slot, &format!("width:{}px;height:{}px;", size.x, size.y))?;
        }
    }
    Ok(())
}
/// Restore exact optional styles, including the absence of an attribute.
fn restore(element: &Element, style: Option<&str>) -> Result<(), AttachmentUiError> {
    match style {
        Some(style) => element.set_attribute("style", style),
        None => element.remove_attribute("style"),
    }
    .map_err(|e| host_error(&e))
}
/// Restore frame, viewport and slot styles plus both viewport axes after failure or readback.
pub(super) fn transaction<T>(
    nodes: (&Element, &Element),
    slots: &[(egui::Id, Element)],
    phase: ReadingPhase,
    operation: impl FnOnce(&Scroll) -> Result<T, AttachmentUiError>,
) -> Result<T, AttachmentUiError> {
    let saved: Vec<_> = slots
        .iter()
        .map(|(_, e)| e)
        .chain([nodes.1, nodes.0])
        .map(|e| (e, e.get_attribute("style")))
        .collect();
    let scroll = Scroll::read(nodes.1)?;
    let result = operation(&scroll);
    if phase == ReadingPhase::Publish && result.is_ok() {
        return result;
    }
    let mut restored = Ok(());
    for (element, style) in &saved {
        let current = restore(element, style.as_deref());
        restored = restored.and(current);
    }
    let offsets = scroll.apply(nodes.1);
    result.and_then(|value| restored.and(offsets).map(|()| value))
}

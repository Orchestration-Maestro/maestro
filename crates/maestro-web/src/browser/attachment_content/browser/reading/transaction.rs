//! One restoration owner for bounded measurements and atomic publication.
use super::{AttachmentHostError, ReadingPhase, ReadingRoot, attr, host_error};
use js_sys::Reflect;
use std::collections::HashMap;
use wasm_bindgen::JsValue;
use web_sys::Element;

/// Native fractional scroll offsets, without integer Element bindings.
pub(super) struct Scroll {
    /// Native horizontal offset.
    left: f64,
    /// Native vertical offset.
    top: f64,
}
impl Scroll {
    /// Observe both native axes before mutation.
    pub(super) fn read(viewport: &Element) -> Result<Self, AttachmentHostError> {
        Ok(Self {
            left: offset(viewport, "scrollLeft")?,
            top: offset(viewport, "scrollTop")?,
        })
    }
    /// Restore the qualified prospective offsets after the successful native content commit.
    pub(super) fn commit(&self, viewport: &Element) {
        viewport.scroll_to_with_x_and_y(self.left, self.top);
    }
    /// Attempt both axes even when the first write fails.
    pub(super) fn apply(&self, viewport: &Element) -> Result<(), AttachmentHostError> {
        let left = Reflect::set(viewport, &"scrollLeft".into(), &JsValue::from(self.left))
            .map_err(|e| host_error(&e));
        let top = Reflect::set(viewport, &"scrollTop".into(), &JsValue::from(self.top))
            .map_err(|e| host_error(&e));
        left.and(top).map(|_| ())
    }
}
/// Read a fractional native offset before any publication mutation.
pub(super) fn offset(viewport: &Element, name: &str) -> Result<f64, AttachmentHostError> {
    Reflect::get(viewport, &name.into())
        .map_err(|e| host_error(&e))?
        .as_f64()
        .ok_or_else(|| AttachmentHostError::Host("Invalid native scroll offset".into()))
}
/// Size slots by identity, preserving native slot ownership.
pub(super) fn size_slots(
    slots: &HashMap<egui::Id, Element>,
    sizes: &[(egui::Id, egui::Vec2)],
) -> Result<(), AttachmentHostError> {
    for (id, size) in sizes {
        if let Some(slot) = slots.get(id) {
            attr(slot, &format!("width:{}px;height:{}px;", size.x, size.y))?;
        }
    }
    Ok(())
}
/// Restore exact optional styles, including the absence of an attribute.
fn restore(element: &Element, style: Option<&str>) -> Result<(), AttachmentHostError> {
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
    operation: impl FnOnce(&Scroll) -> Result<T, AttachmentHostError>,
) -> Result<T, AttachmentHostError> {
    let saved: Vec<_> = slots
        .iter()
        .map(|(_, e)| e)
        .chain([nodes.1, nodes.0])
        .map(|e| (e, e.get_attribute("style")))
        .collect();
    let scroll = Scroll::read(nodes.1)?;
    let detached = !nodes.0.is_connected();
    let result = (|| {
        if detached {
            attr(
                nodes.0,
                "position:fixed;visibility:hidden;pointer-events:none;",
            )?;
            nodes
                .0
                .owner_document()
                .and_then(|document| document.body())
                .ok_or_else(|| AttachmentHostError::Host("Document has no body".into()))?
                .append_child(nodes.0)
                .map_err(|error| host_error(&error))?;
        }
        operation(&scroll)
    })();
    if phase == ReadingPhase::Publish && result.is_ok() {
        return result;
    }
    if detached {
        nodes.0.remove();
    }
    let mut restored = Ok(());
    for (element, style) in &saved {
        let current = restore(element, style.as_deref());
        restored = restored.and(current);
    }
    let offsets = scroll.apply(nodes.1);
    result.and_then(|value| restored.and(offsets).map(|()| value))
}

/// Read the decoration's actual border rather than assuming CSS pixel rounding.
pub(super) fn decoration_border(
    root: &ReadingRoot,
) -> Result<egui::epaint::MarginF32, AttachmentHostError> {
    let Some(decoration) = &root.decoration else {
        return Ok(egui::epaint::MarginF32::ZERO);
    };
    let style = decoration
        .owner_document()
        .and_then(|d| d.default_view())
        .ok_or_else(|| AttachmentHostError::Host("Missing decoration window".into()))?
        .get_computed_style(decoration)
        .map_err(|e| host_error(&e))?
        .ok_or_else(|| AttachmentHostError::Host("Missing decoration style".into()))?;
    let side = |name| -> Result<f32, AttachmentHostError> {
        let value = style.get_property_value(name).map_err(|e| host_error(&e))?;
        value
            .trim_end_matches("px")
            .parse()
            .map_err(|e| AttachmentHostError::Host(format!("Invalid decoration border: {e}")))
    };
    Ok(egui::epaint::MarginF32 {
        left: side("border-left-width")?,
        right: side("border-right-width")?,
        top: side("border-top-width")?,
        bottom: side("border-bottom-width")?,
    })
}

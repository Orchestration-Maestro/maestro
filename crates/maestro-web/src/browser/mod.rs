//! Browser-owned attachment content effects.
pub mod attachment_content;

/// Composed native control decorations.
pub(crate) mod control_paint;

/// Local-server model discovery with the page's effects.
#[cfg(target_arch = "wasm32")]
pub(crate) mod discovery;

/// Cancel-on-drop browser timers.
pub mod timer;

/// Mounted CSS-property interpolation shared by canvas controls.
pub(crate) mod transition;

/// Native sandbox values.
#[cfg(target_arch = "wasm32")]
pub mod sandbox_value;

/// Retained browser event subscriptions.
#[cfg(target_arch = "wasm32")]
pub(crate) mod event_subscription;

/// Append the selected pack's escaped custom-property declarations to native inline CSS.
#[cfg(target_arch = "wasm32")]
pub(crate) fn append_brand_properties(
    style: &mut String,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    use std::fmt::Write as _;
    for (key, value) in presentation.css_properties() {
        let _ = write!(style, "{key}:{value};");
    }
}

/// Complete-message socket effects.
pub mod websocket;

/// Install one native stylesheet per supplied document without retaining feature owners.
#[cfg(target_arch = "wasm32")]
pub(crate) fn install_stylesheet(
    document: &web_sys::Document,
    id: &str,
    source: &str,
) -> Result<(), attachment_content::AttachmentHostError> {
    use attachment_content::{AttachmentHostError, browser::host_error};
    if document.get_element_by_id(id).is_some() {
        return Ok(());
    }
    let style = document
        .create_element("style")
        .map_err(|error| host_error(&error))?;
    style.set_id(id);
    style.set_text_content(Some(source));
    document
        .document_element()
        .ok_or_else(|| AttachmentHostError::Host("Document has no root".into()))?
        .append_child(&style)
        .map_err(|error| host_error(&error))?;
    Ok(())
}

/// Map linear progress through the supplied cubic-bezier timing curve.
pub(crate) fn transition_easing(progress: f32, curve: [f32; 4]) -> f32 {
    if progress <= 0. || progress >= 1. {
        return progress;
    }
    let (mut low, mut high) = (0., 1.);
    for _ in 0..24 {
        let t = f32::midpoint(low, high);
        let x = bezier(t, curve[0], curve[2]);
        if x < progress {
            low = t;
        } else {
            high = t;
        }
    }
    let t = f32::midpoint(low, high);
    bezier(t, curve[1], curve[3])
}

/// Evaluate one axis of a unit-endpoint cubic bezier.
fn bezier(t: f32, first: f32, second: f32) -> f32 {
    let inverse = 1. - t;
    3. * inverse * inverse * t * first + 3. * inverse * t * t * second + t * t * t
}

pub(crate) mod copy_feedback;

/// Math stylesheet and fonts.
#[cfg(target_arch = "wasm32")]
pub(crate) mod math_assets;

/// Mounted native disclosure bindings.
#[cfg(target_arch = "wasm32")]
pub(crate) mod disclosure;

#[cfg(target_arch = "wasm32")]
/// Invoke an owning native object's zero-argument operation without coercion.
pub(crate) fn invoke0(
    value: &wasm_bindgen::JsValue,
    name: &str,
) -> Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    js_sys::Reflect::get(value, &name.into())
        .and_then(wasm_bindgen::JsCast::dyn_into::<js_sys::Function>)
        .and_then(|f| f.call0(value))
}

/// JSON-to-native conversion.
#[cfg(target_arch = "wasm32")]
pub(crate) mod json;

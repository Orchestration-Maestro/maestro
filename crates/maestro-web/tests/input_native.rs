//! Compile-only qualification of native browser input and event capabilities.
#![cfg(target_arch = "wasm32")]

/// Compile the actual browser editing and event capabilities without claiming runtime dispatch.
pub fn native_input_capabilities(
    element: &web_sys::HtmlInputElement,
    event: &web_sys::Event,
    key: &web_sys::KeyboardEvent,
) {
    let _ = element.focus();
    let _ = element.set_selection_range(0, 1);
    let _ = element.selection_start();
    let _ = element.check_validity();
    event.prevent_default();
    event.stop_propagation();
    let _ = event.target();
    let _ = key.shift_key();
    let _ = key.ctrl_key();
    let _ = key.alt_key();
    let _ = key.meta_key();
}

/// Keep the compile-only capability witness reachable without exporting a browser operation.
const _: fn(&web_sys::HtmlInputElement, &web_sys::Event, &web_sys::KeyboardEvent) =
    native_input_capabilities;

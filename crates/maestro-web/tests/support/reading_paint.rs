//! Await painted observations while the caller retains the real native owner.
use wasm_bindgen::{JsCast as _, JsValue};

/// Require the manual browser driver's screenshot-derived observation.
/// # Panics
/// Panics when the driver supplies no callback, capture fails or the observation does not pass.
pub async fn observe(scene: &str) {
    let callback = js_sys::Reflect::get(&js_sys::global(), &"readingPaint".into())
        .unwrap()
        .dyn_into::<js_sys::Function>()
        .expect("the browser driver must supply a screenshot callback");
    let result = callback.call1(&JsValue::NULL, &scene.into()).unwrap();
    let result = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&result))
        .await
        .expect("screenshot capture and comparison must succeed");
    assert_eq!(result.as_bool(), Some(true), "paint observation must pass");
}

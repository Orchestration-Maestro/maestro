//! Native fixture identity and call-boundary regression.
#![cfg(target_arch = "wasm32")]
use crate::browser_fixture_support::FixtureModule;
use maestro_web::browser::attachment_content::{
    AttachmentHost as _, BrowserAttachmentHost, ClipboardMode,
};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
/// Exercise module identity, relative imports, native values, errors and real fixture transactions.
/// # Errors
/// Returns a failed native call.
/// # Panics
/// Panics when a preserved native value or error branch differs.
#[wasm_bindgen]
pub async fn browser_fixture_modules_preserve_native_state_and_failures() -> Result<JsValue, JsValue>
{
    let native = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/sandbox_loading_support/native.js"
    ))
    .await?;
    let repeated = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/sandbox_loading_support/native.js"
    ))
    .await?;
    assert!(js_sys::Object::is(&native.0, &repeated.0));
    let routes = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/sandbox_loading_support/routes.js"
    ))
    .await?;
    routes.call("begin", &js_sys::Array::new())?;
    routes.call("closed", &js_sys::Array::of1(&JsValue::from(0_u32)))?;
    routes.call("end", &js_sys::Array::new())?;
    let execution = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/sandbox_execution_support/native.js"
    ))
    .await?;
    execution.call("begin", &js_sys::Array::new())?;
    execution.call("timers", &js_sys::Array::of1(&JsValue::from(0_u32)))?;
    execution.call("end", &js_sys::Array::new())?;
    let programs = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/sandbox_loading_support/programs.js"
    ))
    .await?;
    assert!(
        programs
            .call("artifact", &js_sys::Array::new())?
            .is_string()
    );
    let clipboard = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/session_links_support/clipboard.js"
    ))
    .await?;
    clipboard.call(
        "begin",
        &js_sys::Array::of2(&"missing".into(), &"literal 雪".into()),
    )?;
    let host = host()?;
    let copying = host.copy_to_clipboard("literal 雪", ClipboardMode::Fallback);
    clipboard.call("immediate", &js_sys::Array::of1(&"missing".into()))?;
    clipboard.call("finish", &js_sys::Array::of1(&JsValue::from(copying.await)))?;
    download(&host).await?;
    values(&native).await?;
    Ok("six fixture modules; shared ESM identity; native typed array; promise; rejection".into())
}

/// Construct the real browser adapter from the containing canvas.
fn host() -> Result<BrowserAttachmentHost, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window missing"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("document missing"))?;
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or_else(|| JsValue::from_str("canvas missing"))?;
    BrowserAttachmentHost::new(canvas.dyn_into()?)
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

/// Observe the production download through the original fixture's cleanup assertion.
async fn download(host: &BrowserAttachmentHost) -> Result<(), JsValue> {
    let module = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/session_download_support/browser.js"
    ))
    .await?;
    module.call("begin", &js_sys::Array::of1(&"success".into()))?;
    let bytes = [0_u8, 128, 255];
    host.download_bytes(&bytes, "literal 雪.dat", "application/octet-stream")
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let args = js_sys::Array::of4(
        &js_sys::Uint8Array::from(&bytes[..]),
        &"application/octet-stream".into(),
        &"literal 雪.dat".into(),
        &JsValue::TRUE,
    );
    let pending = module
        .call("finish", &args)?
        .dyn_into::<js_sys::Promise>()?;
    wasm_bindgen_futures::JsFuture::from(pending).await?;
    Ok(())
}

/// Preserve native values and distinguish every failing seam branch.
async fn values(native: &FixtureModule) -> Result<(), JsValue> {
    let args = js_sys::Array::of2(&JsValue::TRUE, &"qualification".into());
    native.call("assert", &args)?;
    assert!(
        native
            .call(
                "assert",
                &js_sys::Array::of2(&JsValue::FALSE, &"reject".into())
            )
            .is_err()
    );
    let byte_data = js_sys::Uint8Array::from(&[0_u8, 128, 255][..]);
    let echo = FixtureModule::load(&wasm_bindgen::link_to!(
        module = "/tests/browser_fixture_support/native.js"
    ))
    .await?;
    for value in [
        JsValue::UNDEFINED,
        JsValue::NULL,
        JsValue::TRUE,
        JsValue::from(7.5),
        JsValue::from("literal 雪"),
        js_sys::Object::new().into(),
    ] {
        let actual = echo.call(
            "identity",
            &js_sys::Array::of3(&JsValue::UNDEFINED, &JsValue::NULL, &value),
        )?;
        assert!(js_sys::Object::is(&actual, &value));
    }
    assert!(!js_sys::Object::is(&native.0, &echo.0));
    let values = js_sys::Array::of3(&JsValue::UNDEFINED, &JsValue::NULL, &byte_data);
    let returned = echo.call("identity", &values)?;
    assert!(js_sys::Object::is(&returned, &values.get(2)));
    let retained = maestro_app::SharedHandle::new(returned);
    assert!(js_sys::Object::is(retained.as_ref(), &byte_data));
    let promise = echo.call("promise", &js_sys::Array::of1(&JsValue::TRUE))?;
    assert_eq!(
        wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?,
        JsValue::TRUE
    );
    assert!(echo.call("absent", &js_sys::Array::new()).is_err());
    assert!(FixtureModule::load("/absent.js").await.is_err());
    Ok(())
}

//! Native fixture module loading and calls shared by browser witnesses.
#![cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast as _, JsValue};

/// Retained native module namespace.
#[derive(Clone)]
pub(crate) struct FixtureModule(pub(crate) JsValue);

impl FixtureModule {
    /// Load through the browser's module registry, preserving its relative imports.
    pub(crate) async fn load(url: &str) -> Result<Self, JsValue> {
        let import = js_sys::Function::new_with_args("url", "return import(url)");
        Self::import(&import, url).await
    }

    /// Load `url` through a page's own module import, which its policy allows.
    pub(crate) async fn import(import: &js_sys::Function, url: &str) -> Result<Self, JsValue> {
        let pending = import.call1(&JsValue::UNDEFINED, &url.into())?;
        wasm_bindgen_futures::JsFuture::from(pending.dyn_into::<js_sys::Promise>()?)
            .await
            .map(Self)
    }

    /// Invoke the retained export with native arguments and results.
    pub(crate) fn call(&self, name: &str, args: &js_sys::Array) -> Result<JsValue, JsValue> {
        let function =
            js_sys::Reflect::get(&self.0, &name.into())?.dyn_into::<js_sys::Function>()?;
        function.apply(&JsValue::UNDEFINED, args)
    }
}

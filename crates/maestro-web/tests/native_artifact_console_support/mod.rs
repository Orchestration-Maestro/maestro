//! Recorded summaries and native operands shared by the console browser witnesses.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::settle;
use maestro_web::{I18n, dom::DomView};
use serde::Deserialize;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};

/// Consumed console corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cases {
    /// Both languages and all summary branches.
    pub(super) summaries: Vec<CountCase>,
    /// Ordinary native values and corrected dangerous values.
    pub(super) values: Vec<Value>,
    /// Exact formatter output at three growing input sizes.
    pub(super) costs: Vec<Cost>,
}
/// One summary observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CountCase {
    /// Supplied language.
    pub(super) language: String,
    /// Ordered source lines.
    pub(super) logs: Vec<Line>,
    /// Exact localized result.
    pub(super) summary: String,
}
/// One ordinary fixture line.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Line {
    /// Line kind.
    pub(super) r#type: String,
    /// Literal operand.
    pub(super) text: String,
}
/// One native-domain observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Value {
    /// Native constructor selector.
    pub(super) kind: String,
    /// Tagged constructor operands, never a product format.
    #[serde(default)]
    pub(super) operand: serde_json::Value,
    /// Expected rendered line.
    pub(super) paint: Option<String>,
    /// Expected whole-value clipboard line.
    pub(super) copy: Option<String>,
}
/// Read the strictly consumed corpus.
pub(super) fn cases() -> Cases {
    serde_json::from_str(include_str!("cases.json")).unwrap()
}
/// Localization scoped to this caller, not the page's global state.
pub(super) fn localized(view: &DomView, language: &str) -> DomView {
    DomView {
        i18n: Rc::new(
            I18n::new(Rc::new(crate::support::Host {
                stored: Some(language.into()).into(),
                ..crate::support::Host::default()
            }))
            .unwrap(),
        ),
        ..view.clone()
    }
}
/// Import the native operand factory through the actual page module loader.
pub(super) async fn factory(import_module: &js_sys::Function) -> JsValue {
    let url = wasm_bindgen::link_to!(module = "/tests/native_artifact_console_support/values.js");
    settle(import_module.call1(&JsValue::NULL, &url.into()).unwrap())
        .await
        .unwrap()
}
/// Invoke a synchronous native helper without a promise/serialization boundary.
pub(super) fn native(module: &JsValue, name: &str, args: &js_sys::Array) -> JsValue {
    js_sys::Reflect::get(module, &name.into())
        .unwrap()
        .unchecked_into::<js_sys::Function>()
        .apply(&JsValue::NULL, args)
        .unwrap()
}

/// One recorded formatter growth observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cost {
    /// Number of supplied rows.
    pub(super) count: u32,
    /// Exact ordered whole-value result.
    pub(super) formatted: String,
}

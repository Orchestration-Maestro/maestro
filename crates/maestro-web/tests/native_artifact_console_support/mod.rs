//! Recorded summaries and native operands shared by the console browser witnesses.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::browser_fixture_support::{construct, global, invoke, object, set};
use maestro_web::{I18n, page::DomView};
use serde::Deserialize;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};

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
/// The page value of a JSON operand.
fn parsed(value: &serde_json::Value) -> JsValue {
    js_sys::JSON::parse(&value.to_string()).unwrap()
}
/// Construct the native operand that the tagged `value` describes, which JSON alone cannot carry.
pub(super) fn operand(kind: &str, value: &serde_json::Value) -> JsValue {
    if kind == "negative-zero" {
        return JsValue::from_f64(-0.0);
    }
    if kind == "template" {
        let strings = parsed(&value["strings"]);
        set(&strings, "raw", &parsed(&value["raw"]));
        let template = object(&[
            ("_$litType$", &parsed(&value["litType"])),
            ("strings", &strings),
            ("values", &parsed(&value["values"])),
        ]);
        return invoke(&js_sys::global(), "structuredClone", &[&template]).unwrap();
    }
    let Some(native) = value.get("nativeType").and_then(serde_json::Value::as_str) else {
        return parsed(value);
    };
    let field = |name: &str| parsed(&value[name]);
    match native {
        "Undefined" => JsValue::UNDEFINED,
        "Number" => construct_value("Number", &field("value")),
        "BigInt" => construct_value("BigInt", &field("value")),
        "Date" => construct("Date", &[&field("value")]),
        "RegExp" => construct("RegExp", &[&field("value"), &field("flags")]),
        "Map" => construct("Map", &[&field("entries")]),
        "Set" => construct("Set", &[&field("items")]),
        "Uint8Array" => construct("Uint8Array", &[&field("items")]),
        "ArrayBuffer" => construct("ArrayBuffer", &[&field("bytes")]),
        "DataView" => construct("DataView", &[&construct("ArrayBuffer", &[&field("bytes")])]),
        "Array" if value["self"] == true => {
            let array = js_sys::Array::new();
            array.push(&array);
            array.into()
        }
        "Array" => {
            let copies = value["copies"].as_u64().unwrap();
            let shared = field("shared");
            (0..copies)
                .map(|_| shared.clone())
                .collect::<js_sys::Array>()
                .into()
        }
        other => panic!("unknown fixture native type {other}"),
    }
}
/// Call the page conversion function `name` on `value`, as `Number(value)` does.
fn construct_value(name: &str, value: &JsValue) -> JsValue {
    global(name)
        .unchecked_into::<js_sys::Function>()
        .call1(&JsValue::UNDEFINED, value)
        .unwrap()
}
/// The page's own text of a value; date text follows the browser's locale and timezone.
pub(super) fn native_string(value: &JsValue) -> String {
    construct_value("String", value).as_string().unwrap()
}

/// Supplied native values that count their own conversions and iterations.
pub(super) struct Probe {
    /// Scalars converting to their index, each conversion counted.
    pub(super) scalars: js_sys::Array,
    /// An iterable yielding `x` the requested number of times, each step counted.
    pub(super) iterable: JsValue,
    /// Iteration steps taken so far.
    visits: Rc<Cell<u32>>,
    /// Scalar conversions so far.
    reads: Rc<Cell<u32>>,
}
impl Probe {
    /// Counters live on supplied native values, not on the production console.
    pub(super) fn new(count: u32) -> Self {
        let (visits, reads) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let scalars = (0..count).map(|index| scalar(index, &reads)).collect();
        let iterator = Closure::<dyn Fn() -> JsValue>::new({
            let visits = Rc::clone(&visits);
            move || steps(count, &visits)
        });
        let iterable = js_sys::Object::new();
        js_sys::Reflect::set(
            &iterable,
            &js_sys::Symbol::iterator(),
            &iterator.into_js_value(),
        )
        .unwrap();
        Self {
            scalars,
            iterable: iterable.into(),
            visits,
            reads,
        }
    }
    /// Iteration steps and scalar conversions so far.
    pub(super) fn stats(&self) -> (u32, u32) {
        (self.visits.get(), self.reads.get())
    }
}
/// A scalar converting to `index`, counting each conversion in `reads`.
fn scalar(index: u32, reads: &Rc<Cell<u32>>) -> JsValue {
    let reads = Rc::clone(reads);
    let text = Closure::<dyn Fn() -> String>::new(move || {
        reads.set(reads.get() + 1);
        index.to_string()
    });
    object(&[("toString", &text.into_js_value())]).into()
}
/// A fresh iterator yielding `x` `count` times, counting each step in `visits`.
fn steps(count: u32, visits: &Rc<Cell<u32>>) -> JsValue {
    let (visits, step) = (Rc::clone(visits), Cell::new(0));
    let next = Closure::<dyn Fn() -> JsValue>::new(move || {
        let done = step.get() == count;
        if !done {
            step.set(step.get() + 1);
            visits.set(visits.get() + 1);
        }
        object(&[("value", &"x".into()), ("done", &done.into())]).into()
    });
    object(&[("next", &next.into_js_value())]).into()
}
/// A deliberately wrong whole-input rescan, retained as a discriminating control.
pub(super) fn rescan(count: u32) -> u32 {
    count * count
}
/// A deliberately wrong global identity filter suppresses completed shared occurrences.
pub(super) fn deduplicated(value: &JsValue) -> String {
    /// Visit `value`, skipping every iterable already seen anywhere.
    fn visit(value: &JsValue, seen: &mut Vec<JsValue>) -> String {
        if value.is_null() || value.is_undefined() {
            return String::new();
        }
        if !value.is_object() || js_sys::try_iter(value).unwrap().is_none() {
            return native_string(value);
        }
        if seen.iter().any(|known| js_sys::Object::is(known, value)) {
            return String::new();
        }
        seen.push(value.clone());
        let items: Vec<JsValue> = js_sys::try_iter(value)
            .unwrap()
            .unwrap()
            .map(Result::unwrap)
            .collect();
        items.iter().map(|item| visit(item, seen)).collect()
    }
    visit(value, &mut Vec::new())
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

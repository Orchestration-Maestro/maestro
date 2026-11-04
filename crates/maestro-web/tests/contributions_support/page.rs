//! Browser observations read by the contribution exports, which run without eval.
use super::settle;
use crate::browser_fixture_support::{construct, get, global, invoke, object};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, Node};

/// Mutation kinds a page observer watches below its target.
fn watched(old_values: bool) -> js_sys::Object {
    let on = JsValue::TRUE;
    let old = JsValue::from(old_values);
    object(&[
        ("subtree", &on),
        ("childList", &on),
        ("characterData", &on),
        ("attributes", &on),
        ("attributeOldValue", &old),
        ("characterDataOldValue", &old),
    ])
}

/// Settle at the next change below `target`.
pub(crate) async fn changed(target: &Node) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let callback = Closure::once_into_js(move |_: JsValue, observer: JsValue| {
            invoke(&observer, "disconnect", &[]).unwrap();
            resolve.call0(&JsValue::NULL).unwrap();
        });
        let observer = construct("MutationObserver", &[&callback]);
        invoke(&observer, "observe", &[target, &watched(false)]).unwrap();
    });
    settle(promise.into()).await.unwrap();
}

/// Changes recorded below one target, taken in batches; disconnected when dropped.
pub(crate) struct Recorder {
    /// The page observer.
    observer: JsValue,
    /// Changes delivered since the last batch.
    records: Rc<RefCell<Vec<JsValue>>>,
    /// The observer callback, kept alive while the observer runs.
    _callback: Closure<dyn Fn(js_sys::Array)>,
}

impl Recorder {
    /// Record every change below `target`, with the values it replaced.
    pub(crate) fn new(target: &Node) -> Self {
        let records: Rc<RefCell<Vec<JsValue>>> = Rc::default();
        let delivered = Rc::clone(&records);
        let callback = Closure::<dyn Fn(js_sys::Array)>::new(move |taken: js_sys::Array| {
            delivered.borrow_mut().extend(taken.iter());
        });
        let observer = construct("MutationObserver", &[callback.as_ref()]);
        invoke(&observer, "observe", &[target, &watched(true)]).unwrap();
        Self {
            observer,
            records,
            _callback: callback,
        }
    }

    /// The changes since the previous batch, including those not yet delivered.
    pub(crate) fn take(&self) -> Vec<JsValue> {
        let pending = js_sys::Array::from(&invoke(&self.observer, "takeRecords", &[]).unwrap());
        let mut records = std::mem::take(&mut *self.records.borrow_mut());
        records.extend(pending.iter());
        records
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        invoke(&self.observer, "disconnect", &[]).unwrap();
    }
}

/// Values a later change replaced, by node and attribute name (empty for character data).
#[derive(Default)]
struct Replaced(Vec<(Node, HashMap<String, Option<String>>)>);

impl Replaced {
    /// The value a change wrote: the next change's old value, else `current`.
    fn latest(&self, node: &Node, key: &str, current: Option<String>) -> Option<String> {
        self.0
            .iter()
            .find(|(seen, _)| seen.is_same_node(Some(node)))
            .and_then(|(_, values)| values.get(key).cloned())
            .unwrap_or(current)
    }

    /// Remember the value `node`'s `key` held before a change.
    fn remember(&mut self, node: &Node, key: &str, old: Option<String>) {
        let index = self
            .0
            .iter()
            .position(|(seen, _)| seen.is_same_node(Some(node)));
        let index = index.unwrap_or_else(|| {
            self.0.push((node.clone(), HashMap::new()));
            self.0.len() - 1
        });
        self.0[index].1.insert(key.to_owned(), old);
    }
}

/// `node` as an element, whichever document's realm created it.
fn element(node: &Node) -> Option<&Element> {
    (node.node_type() == Node::ELEMENT_NODE).then(|| node.unchecked_ref())
}

/// UTF-16 code units of a page string.
fn units(text: Option<&str>) -> usize {
    text.map_or(0, |text| text.encode_utf16().count())
}

/// The UTF-16 code units `records` publish outside elements matching `skip`, counting each
/// written attribute or character-data value and the text and attribute values of each
/// inserted node.
pub(crate) fn written(records: &[JsValue], skip: &str) -> usize {
    // Walking backwards, the value a change wrote is the old value of the next change.
    let mut replaced = Replaced::default();
    let mut total = 0;
    for record in records.iter().rev() {
        let node: Node = get(record, "target").unchecked_into();
        let element = match element(&node) {
            Some(element) => Some(element.clone()),
            None => node.parent_element(),
        };
        if !skip.is_empty()
            && element.is_some_and(|element| element.closest(skip).unwrap().is_some())
        {
            continue;
        }
        let old = get(record, "oldValue").as_string();
        match get(record, "type").as_string().as_deref() {
            Some("attributes") => {
                let name = get(record, "attributeName").as_string().unwrap();
                let current = node.unchecked_ref::<Element>().get_attribute(&name);
                total += units(replaced.latest(&node, &name, current).as_deref());
                replaced.remember(&node, &name, old);
            }
            Some("characterData") => {
                total += units(replaced.latest(&node, "", node.text_content()).as_deref());
                replaced.remember(&node, "", old);
            }
            _ => {
                let added = js_sys::Array::from(&get(record, "addedNodes"));
                for added in added.iter() {
                    total += inserted(&added.unchecked_into(), &replaced);
                }
            }
        }
    }
    total
}

/// Text and attribute values below an inserted node; inserted text counts as first written.
fn inserted(node: &Node, replaced: &Replaced) -> usize {
    let Some(element) = element(node) else {
        let text = node.text_content().unwrap_or_default();
        return units(replaced.latest(node, "", Some(text)).as_deref());
    };
    let descendants = element.query_selector_all("*").unwrap();
    let elements = std::iter::once(element.clone()).chain(
        (0..descendants.length()).map(|index| descendants.item(index).unwrap().unchecked_into()),
    );
    let attributes: usize = elements
        .map(|element: Element| {
            element
                .get_attribute_names()
                .iter()
                .map(|name| units(element.get_attribute(&name.as_string().unwrap()).as_deref()))
                .sum::<usize>()
        })
        .sum();
    units(element.text_content().as_deref()) + attributes
}

/// The time of the next animation frame, in milliseconds.
async fn frame() -> f64 {
    let window = web_sys::window().unwrap();
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        window.request_animation_frame(&resolve).unwrap();
    });
    settle(promise.into()).await.unwrap().as_f64().unwrap()
}

/// Intervals between the next `count` animation frames, in milliseconds.
pub(crate) async fn frames(count: u32) -> Vec<f64> {
    let mut previous = frame().await;
    let mut intervals = Vec::new();
    for _ in 0..count {
        let time = frame().await;
        intervals.push(time - previous);
        previous = time;
    }
    intervals
}

/// The engine's used heap size, when the browser reports it.
pub(crate) fn heap() -> Option<f64> {
    let memory = get(&global("performance"), "memory");
    if memory.is_undefined() {
        return None;
    }
    get(&memory, "usedJSHeapSize").as_f64()
}

/// The bytes of a fetched or compressed body.
async fn body(response: &JsValue) -> js_sys::ArrayBuffer {
    let bytes = settle(invoke(response, "arrayBuffer", &[]).unwrap())
        .await
        .unwrap();
    bytes.unchecked_into()
}

/// Raw and gzip-compressed byte counts of each asset, by URL.
pub(crate) async fn sizes(urls: &[String]) -> serde_json::Map<String, serde_json::Value> {
    let window: JsValue = web_sys::window().unwrap().into();
    let mut measured = serde_json::Map::new();
    for url in urls {
        let response = settle(invoke(&window, "fetch", &[&url.into()]).unwrap())
            .await
            .unwrap();
        let bytes = body(&response).await;
        let blob = construct("Blob", &[&js_sys::Array::of1(&bytes)]);
        let stream = invoke(&blob, "stream", &[]).unwrap();
        let gzip = construct("CompressionStream", &[&"gzip".into()]);
        let compressed = invoke(&stream, "pipeThrough", &[&gzip]).unwrap();
        let raw = bytes.byte_length();
        let gzip = body(&construct("Response", &[&compressed]))
            .await
            .byte_length();
        measured.insert(url.clone(), serde_json::json!({"raw": raw, "gzip": gzip}));
    }
    measured
}

/// The SHA-256 digest of a text's UTF-8 bytes, as the page computes it.
pub(crate) async fn digest(text: &str) -> Vec<u8> {
    let subtle = get(&global("crypto"), "subtle");
    let bytes = js_sys::Uint8Array::from(text.as_bytes());
    let digest = invoke(&subtle, "digest", &[&"SHA-256".into(), &bytes]).unwrap();
    js_sys::Uint8Array::new(&settle(digest).await.unwrap()).to_vec()
}

/// Lowercase hexadecimal SHA-256 of a text's UTF-8 bytes, as the page computes it.
pub(crate) async fn sha256(text: &str) -> String {
    use std::fmt::Write as _;
    digest(text)
        .await
        .iter()
        .fold(String::new(), |mut hex, byte| {
            // Writing to a String cannot fail.
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// The page selection.
fn page_selection() -> JsValue {
    invoke(&web_sys::window().unwrap(), "getSelection", &[]).unwrap()
}

/// Select every child of `node`; returns the selected text.
pub(crate) fn select_children(node: &Node) -> String {
    let selection = page_selection();
    invoke(&selection, "selectAllChildren", &[node]).unwrap();
    invoke(&selection, "toString", &[])
        .unwrap()
        .as_string()
        .unwrap()
}

/// The selected text and whether `root` contains the selection's anchor.
pub(crate) fn selection(root: &Element) -> (String, bool) {
    let selection = page_selection();
    let anchor = get(&selection, "anchorNode").dyn_into::<Node>().ok();
    let text = invoke(&selection, "toString", &[])
        .unwrap()
        .as_string()
        .unwrap();
    (text, root.contains(anchor.as_ref()))
}

/// Whether the browser's own find locates `text` from the start of the page.
pub(crate) fn find(text: &str) -> bool {
    invoke(&page_selection(), "removeAllRanges", &[]).unwrap();
    invoke(&web_sys::window().unwrap(), "find", &[&text.into()])
        .unwrap()
        .as_bool()
        .unwrap()
}

/// The running animations of `element`.
fn animations(element: &Element) -> js_sys::Array {
    invoke(element, "getAnimations", &[])
        .unwrap()
        .unchecked_into()
}

/// Pause the running transition of `property` on `element` at `time` milliseconds; returns
/// its duration, or `None` when none runs.
pub(crate) fn seek(element: &Element, property: &str, time: f64) -> Option<f64> {
    let transition = animations(element).iter().find(|animation| {
        get(animation, "transitionProperty").as_string().as_deref() == Some(property)
    })?;
    invoke(&transition, "pause", &[]).unwrap();
    crate::browser_fixture_support::set(&transition, "currentTime", &time.into());
    let timing = invoke(&get(&transition, "effect"), "getTiming", &[]).unwrap();
    get(&timing, "duration").as_f64()
}

/// Finish every running animation of `element`.
pub(crate) fn finish(element: &Element) {
    for animation in animations(element).iter() {
        invoke(&animation, "finish", &[]).unwrap();
    }
}

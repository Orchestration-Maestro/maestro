//! Word previews mounted as browser nodes and read back.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "../src/docx_preview/tests/corrections.rs"]
mod corrections;
use maestro_web::docx_preview::{
    HtmlContent, HtmlElement, Options, parse_async, render_async, render_document,
};
use serde::Deserialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Recorded pages whose trees the mounted nodes must rebuild.
const PAGES: [(&str, &[u8]); 4] = [
    (
        "inline-hostile-values",
        include_bytes!("../src/docx_preview/fixtures/docx/inline-hostile-values.docx"),
    ),
    (
        "table-vertical-merge",
        include_bytes!("../src/docx_preview/fixtures/docx/table-vertical-merge.docx"),
    ),
    (
        "header-footer-default",
        include_bytes!("../src/docx_preview/fixtures/docx/header-footer-default.docx"),
    ),
    (
        "footnotes",
        include_bytes!("../src/docx_preview/fixtures/docx/footnotes.docx"),
    ),
];

/// One recorded rendering mounted with object URLs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectCase {
    /// Fixture name, also the package file stem.
    name: String,
    /// Options the recording rendered with.
    options: RecordedOptions,
    /// Recorded result kind.
    outcome: String,
    /// Recorded body container.
    #[serde(default)]
    expected: Vec<Value>,
    /// Recorded style container, present only when the recording mounted styles apart.
    #[serde(default)]
    styles: Option<Vec<Value>>,
    /// The object URLs the recording named, by the token its tree uses.
    #[serde(default)]
    blobs: Vec<Blob>,
}

/// Options the recording rendered with, by their recorded spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RecordedOptions {
    /// Page-width override.
    ignore_width: Option<bool>,
}

impl RecordedOptions {
    /// Apply the recorded values to the default options.
    fn options(&self) -> Options {
        let mut options = Options::default();
        options.page_size.ignore_width =
            self.ignore_width.unwrap_or(options.page_size.ignore_width);
        options
    }
}

/// One recorded object URL and the bytes it served.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    /// Token the recorded tree spells the URL as.
    token: String,
    /// Bytes the URL served, base64 encoded.
    base64: String,
}

/// One observed container: its tree with `blob:` URLs replaced by their first-appearance
/// token, and the URL behind each token.
#[derive(Deserialize)]
struct Observed {
    /// The container's elements.
    tree: Vec<Value>,
    /// Each token with the URL it stands for.
    blobs: Vec<(String, String)>,
}

/// Read the tree of one mounted container: declarations are read back from the parsed style,
/// `blob:` URLs become tokens in order of appearance and namespaces are spelled short.
const OBSERVE: &str = r#"
const SHORT = {'http://www.w3.org/1999/xhtml': 'html', 'http://www.w3.org/2000/svg': 'svg', 'http://www.w3.org/1998/Math/MathML': 'math'};
const hexColor = (value) => value.replace(/rgb\((\d+), (\d+), (\d+)\)/g, (w, r, g, b) =>
  '#' + [r, g, b].map((c) => Number(c).toString(16).padStart(2, '0')).join('').toUpperCase());
const expand = (property, value) => {
  const parts = value.split(/\s+/).filter(Boolean);
  const [top, right = top, bottom = top, left = right] = parts;
  if (parts.length < 1 || parts.length > 4) return [[property, value]];
  return [[property + '-left', left], [property + '-right', right], [property + '-top', top], [property + '-bottom', bottom]];
};
const normalizeStyle = (text) => {
  let declarations = text.split(';').map((d) => d.trim()).filter(Boolean).map((d) => {
    const at = d.indexOf(':');
    const property = d.slice(0, at).trim();
    const value = hexColor(d.slice(at + 1).trim());
    return ['padding', 'margin'].includes(property) ? expand(property, value) : [[property, value]];
  }).flat();
  const value = (name) => declarations.find(([key]) => key === name)?.[1];
  if (value('border-width') === 'medium' && value('border-style') === 'none' && value('border-color') === 'currentcolor') {
    declarations = declarations.filter(([key]) => !['border-width', 'border-style', 'border-color', 'border-image'].includes(key));
    declarations.push(['border', 'none']);
  }
  const ordered = new Map();
  for (const [key, v] of declarations) {
    ordered.delete(key);
    ordered.set(key, v.startsWith('calc(') ? 'calc(...)' : v);
  }
  return [...ordered.entries()].sort((l, r) => (l[0] < r[0] ? -1 : l[0] > r[0] ? 1 : 0)).map(([k, v]) => `${k}:${v}`).join(';');
};
const blobs = new Map();
const blobOf = (text) => text.replace(/blob:[^)"'\s]+/g, (url) => {
  if (!blobs.has(url)) blobs.set(url, `blob:${blobs.size}`);
  return blobs.get(url);
});
const scratch = document.createElement('div');
const reserialize = (text) => {
  scratch.setAttribute('style', text);
  const out = normalizeStyle(scratch.style.cssText);
  scratch.removeAttribute('style');
  return out;
};
const normalizeTree = (nodes) => nodes.map((node) => {
  if (node.text !== undefined || node.css !== undefined) return node;
  const record = {...node};
  if (node.attributes) record.attributes = node.attributes.map(([name, value]) =>
    name === 'style' && node.ns !== 'svg' && !value.includes('calc(') ? [name, reserialize(value)] : [name, value]);
  if (node.children) record.children = normalizeTree(node.children);
  return record;
});
const observe = (node) => {
  if (node.nodeType === 3) return {text: node.textContent};
  const ns = SHORT[node.namespaceURI] ?? node.namespaceURI;
  if (node.localName === 'style' && ns === 'html') return {tag: 'style', css: blobOf(node.textContent)};
  const attributes = [...node.attributes].map((a) => {
    const raw = blobOf(a.value);
    return a.name === 'style' && ns !== 'svg' ? [a.name, normalizeStyle(node.style.cssText)] : [a.name, raw];
  }).sort((l, r) => (l[0] < r[0] ? -1 : l[0] > r[0] ? 1 : 0));
  const record = {tag: node.localName};
  if (ns !== 'html') record.ns = ns;
  record.attributes = attributes;
  record.children = [...node.childNodes].map(observe);
  return record;
};
if (root === null) return (nodes) => JSON.stringify(normalizeTree(nodes));
return JSON.stringify({tree: [...root.childNodes].map(observe), blobs: [...blobs.entries()].map(([url, token]) => [token, url])});
"#;

/// The page recordings, hostile values included, rebuild through the node route.
#[wasm_bindgen]
pub fn mounted_pages_rebuild_the_recorded_trees() {
    let recorded: Vec<Value> = serde_json::from_str(include_str!(
        "../src/docx_preview/fixtures/docx/render-cases.json"
    ))
    .unwrap();
    let document = window().document().unwrap();
    let mut options = Options::default();
    options.wrapper.in_wrapper = false;
    for (name, bytes) in PAGES {
        let body = container(&document);
        let styles = container(&document);
        let preview = parse_async(bytes, options.clone()).unwrap();
        let rendered = render_document(&preview, &body, Some(&styles), &options).unwrap();
        let case = recorded.iter().find(|case| case["name"] == name).unwrap();
        let mut expected: Vec<Value> = serde_json::from_value(case["expected"].clone()).unwrap();
        corrections::correct(name, &mut expected);
        compare(name, &observe(&body).tree, &expected);
        drop(rendered);
    }
}

/// Every preview recording rebuilds through the mount, with namespaces, the bytes of its
/// object URLs and the sizes the browser measures for shapes.
#[wasm_bindgen]
pub async fn mounted_previews_rebuild_the_recorded_containers() {
    let cases: Vec<ObjectCase> = serde_json::from_str(include_str!(
        "../src/docx_preview/fixtures/docx/preview-object-urls.json"
    ))
    .unwrap();
    let document = window().document().unwrap();
    for case in cases {
        assert_eq!(case.outcome, "success", "{}", case.name);
        let body = container(&document);
        let styles = case.styles.as_ref().map(|_| container(&document));
        let bytes = package(&case.name);
        let (_, rendered) =
            render_async(bytes, &body, styles.as_ref(), case.options.options()).unwrap();
        settle().await;
        assert_bytes(&observe(&body), &case).await;
        let mut expected = case.expected.clone();
        let mut expected_styles = case.styles.clone().unwrap_or_default();
        corrections::correct_preview(&case.name, &mut expected, &mut expected_styles);
        compare(&case.name, &observe(&body).tree, &expected);
        if let Some(styles) = &styles {
            assert_bytes(&observe(styles), &case).await;
            compare(&case.name, &observe(styles).tree, &expected_styles);
        }
        drop(rendered);
    }
}

/// Dropping a rendering revokes the object URLs its nodes name.
#[wasm_bindgen]
pub async fn dropping_the_rendered_document_revokes_its_object_urls() {
    let document = window().document().unwrap();
    let body = container(&document);
    let bytes = package("image-inline");
    let (_, rendered) = render_async(bytes, &body, None, Options::default()).unwrap();
    let source = body
        .query_selector("img")
        .unwrap()
        .and_then(|image| image.get_attribute("src"))
        .expect("image source");
    assert!(source.starts_with("blob:"), "{source}");
    assert!(
        !fetch_bytes(&source).await.is_empty(),
        "mounted URL serves bytes"
    );
    drop(rendered);
    assert!(
        fetch_bytes(&source).await.is_empty(),
        "a dropped rendering revokes {source}"
    );
}

/// Pages returned without a mount keep the object URLs their images name.
#[wasm_bindgen]
pub async fn section_pages_keep_the_object_urls_they_name() {
    let preview = parse_async(package("image-inline"), Options::default()).unwrap();
    let sections = preview.render_sections();
    let source = image_source(&sections.pages).expect("image source");
    assert!(source.starts_with("blob:"), "{source}");
    assert!(
        !fetch_bytes(&source).await.is_empty(),
        "returned sections keep {source}"
    );
}

/// The source of the first image among rendered elements.
fn image_source(elements: &[HtmlElement]) -> Option<String> {
    fn find(element: &HtmlElement) -> Option<String> {
        if element.tag() == "img" {
            return element
                .attributes()
                .find(|(name, _)| *name == "src")
                .map(|(_, value)| value.to_owned());
        }
        element.children().iter().find_map(|child| match child {
            HtmlContent::Element(child) => find(child),
            HtmlContent::Text(_) => None,
        })
    }
    elements.iter().find_map(find)
}

/// After an alternate chunk's frame loads, the content it shows has run no script.
#[wasm_bindgen]
pub async fn alternate_chunk_scripts_do_not_run() {
    let document = window().document().unwrap();
    let body = container(&document);
    let bytes = package("alt-chunk-script");
    let (_, rendered) = render_async(bytes, &body, None, Options::default()).unwrap();
    let frame = body.query_selector("iframe").unwrap().expect("chunk frame");
    assert_eq!(frame.get_attribute("sandbox").as_deref(), Some(""));
    wait_for_load(&frame).await;
    let flag = js_sys::Reflect::get(&window(), &JsValue::from_str("__chunkScript")).unwrap();
    assert!(flag.is_undefined(), "chunk script ran: {flag:?}");
    drop(rendered);
}

/// Compare one container with its recording, naming the first element that differs.
fn compare(name: &str, observed: &[Value], expected: &[Value]) {
    let expected = &normalize_recorded(expected);
    for (index, (observed, expected)) in observed.iter().zip(expected).enumerate() {
        if observed != expected {
            web_sys::console::error_1(&JsValue::from_str(&format!(
                "{name}/{index} observed {observed}\n{name}/{index} expected {expected}"
            )));
        }
    }
    assert_eq!(observed.len(), expected.len(), "{name} elements");
    assert_eq!(observed, expected, "{name}");
}

/// Log one diagnostic line to the browser console.
fn log(message: &str) {
    web_sys::console::log_1(&JsValue::from_str(message));
}

/// The page's window.
fn window() -> web_sys::Window {
    web_sys::window().unwrap()
}

/// Create an empty container appended to the page.
fn container(document: &web_sys::Document) -> web_sys::Element {
    let element = document.create_element("div").unwrap();
    document
        .body()
        .unwrap()
        .append_child(&element)
        .expect("container attached");
    element
}

/// Read one mounted container.
fn observe(element: &web_sys::Element) -> Observed {
    let read = js_sys::Function::new_with_args("root", OBSERVE);
    let observed = read.call1(&JsValue::NULL, element).unwrap();
    serde_json::from_str(&observed.as_string().unwrap()).unwrap()
}

/// Read a recorded tree through the browser's own style serialization, so declarations the
/// browser writes in its own spelling compare equal on both sides.
fn normalize_recorded(tree: &[Value]) -> Vec<Value> {
    let read = js_sys::Function::new_with_args("root", OBSERVE);
    let normalize = read.call1(&JsValue::NULL, &JsValue::NULL).unwrap();
    let normalize: js_sys::Function = normalize.dyn_into().unwrap();
    let tree = serde_json::to_string(tree).unwrap();
    let value = js_sys::JSON::parse(&tree).unwrap();
    let normalized = normalize.call1(&JsValue::NULL, &value).unwrap();
    serde_json::from_str(&normalized.as_string().unwrap()).unwrap()
}

/// Check that every object URL a container names serves the recorded bytes.
async fn assert_bytes(observed: &Observed, case: &ObjectCase) {
    for (token, url) in &observed.blobs {
        let recorded = case
            .blobs
            .iter()
            .find(|recorded| recorded.token == *token)
            .unwrap_or_else(|| panic!("{} records no {token}", case.name));
        let note_image = case.name == "image-in-header-and-note";
        let expected = if note_image && recorded.base64 == corrections::DOCUMENT_IMAGE_BYTES {
            corrections::NOTES_IMAGE_BYTES.to_owned()
        } else {
            recorded.base64.clone()
        };
        let bytes = fetch_bytes(url).await;
        let recorded = Blob {
            token: recorded.token.clone(),
            base64: expected,
        };
        if bytes != recorded.base64 {
            log(&format!(
                "{} {token} observed {bytes} expected {}",
                case.name, recorded.base64
            ));
        }
        assert_eq!(bytes, recorded.base64, "{} {token}", case.name);
    }
}

/// Read one recorded package.
fn package(name: &str) -> &'static [u8] {
    match name {
        "numbering-picture-bullet" => {
            include_bytes!("../src/docx_preview/fixtures/docx/numbering-picture-bullet.docx")
        }
        "embedded-fonts" => include_bytes!("../src/docx_preview/fixtures/docx/embedded-fonts.docx"),
        "font-name-escape" => {
            include_bytes!("../src/docx_preview/fixtures/docx/font-name-escape.docx")
        }
        "image-inline" => include_bytes!("../src/docx_preview/fixtures/docx/image-inline.docx"),
        "image-anchors" => include_bytes!("../src/docx_preview/fixtures/docx/image-anchors.docx"),
        "image-crop-rotate" => {
            include_bytes!("../src/docx_preview/fixtures/docx/image-crop-rotate.docx")
        }
        "image-in-header-and-note" => {
            include_bytes!("../src/docx_preview/fixtures/docx/image-in-header-and-note.docx")
        }
        "vml-shapes" => include_bytes!("../src/docx_preview/fixtures/docx/vml-shapes.docx"),
        "full-composition" => {
            include_bytes!("../src/docx_preview/fixtures/docx/full-composition.docx")
        }
        "full-composition-separate" => {
            include_bytes!("../src/docx_preview/fixtures/docx/full-composition-separate.docx")
        }
        "alt-chunk-script" => {
            include_bytes!("../src/docx_preview/fixtures/docx/alt-chunk-script.docx")
        }
        other => panic!("unrecorded package {other}"),
    }
}

/// Wait two animation frames: one for the shape sizing and one for its effect to land.
async fn settle() {
    let frames = js_sys::Function::new_no_args(
        "return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));",
    );
    let promise: js_sys::Promise = frames.call0(&JsValue::NULL).unwrap().dyn_into().unwrap();
    wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}

/// Wait for a frame's document to finish loading.
async fn wait_for_load(frame: &web_sys::Element) {
    let loaded = js_sys::Function::new_with_args(
        "frame",
        "return new Promise((resolve) => frame.addEventListener('load', () => resolve(true), {once: true}));",
    );
    let promise: js_sys::Promise = loaded
        .call1(&JsValue::NULL, frame)
        .unwrap()
        .dyn_into()
        .unwrap();
    wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}

/// Fetch one object URL as base64 text, or an empty text when it no longer resolves.
pub(crate) async fn fetch_bytes(url: &str) -> String {
    use base64::Engine as _;
    use wasm_bindgen_futures::JsFuture;
    let Ok(response) = JsFuture::from(window().fetch_with_str(url)).await else {
        return String::new();
    };
    let response: web_sys::Response = response.dyn_into().unwrap();
    if !response.ok() {
        return String::new();
    }
    let Ok(buffer) = response.array_buffer() else {
        return String::new();
    };
    let Ok(bytes) = JsFuture::from(buffer).await else {
        return String::new();
    };
    base64::engine::general_purpose::STANDARD.encode(js_sys::Uint8Array::new(&bytes).to_vec())
}

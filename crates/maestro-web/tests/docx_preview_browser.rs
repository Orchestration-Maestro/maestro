//! Word page markup mounted and read back by a browser.
#![cfg(all(test, target_arch = "wasm32"))]
#[path = "../src/docx_preview/tests/corrections.rs"]
mod corrections;
use maestro_web::docx_preview::{HtmlElement, Options, WordPreview};
use serde_json::Value;
use wasm_bindgen::prelude::*;

/// Recorded packages whose page trees the mounted markup must rebuild.
const CASES: [(&str, &[u8]); 4] = [
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

/// Observe mounted nodes the way the recording observed its pages: declarations
/// are read back from the parsed style, numbers and colors in the browser's spelling,
/// shorthand margins expanded, `calc(...)` operands collapsed and attributes sorted. A table
/// row's implied `tbody`, which only markup parsing adds, is read through.
const OBSERVE: &str = r"
const hexColor = (value) => value.replace(/rgb\((\d+), (\d+), (\d+)\)/g, (whole, red, green, blue) =>
  '#' + [red, green, blue].map((channel) => Number(channel).toString(16).padStart(2, '0')).join('').toUpperCase());
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
  for (const [key, value] of declarations) {
    ordered.delete(key);
    ordered.set(key, value.startsWith('calc(') ? 'calc(...)' : value);
  }
  return [...ordered.entries()].sort((l, r) => (l[0] < r[0] ? -1 : l[0] > r[0] ? 1 : 0)).map(([k, v]) => `${k}:${v}`).join(';');
};
const observe = (node) => node.nodeType === 3 ? {text: node.textContent} : {
  tag: node.localName,
  attributes: [...node.attributes]
    .map((a) => [a.name, a.name === 'style' ? normalizeStyle(node.style.cssText) : a.value])
    .filter(([name, value]) => name !== 'style' || value !== '')
    .sort((l, r) => (l[0] < r[0] ? -1 : l[0] > r[0] ? 1 : 0)),
  children: [...node.childNodes].flatMap((c) => c.localName === 'tbody' ? [...c.childNodes] : [c]).map(observe),
};
return JSON.stringify([...root.childNodes].map(observe));
";

/// Rendered pages mounted as markup rebuild the recorded trees, as Maestro corrects them,
/// with every value unescaped once.
#[wasm_bindgen]
pub fn emitted_markup_parses_back_to_the_recorded_tree() {
    let recorded: Vec<Value> = serde_json::from_str(include_str!(
        "../src/docx_preview/fixtures/docx/render-cases.json"
    ))
    .unwrap();
    let observe = js_sys::Function::new_with_args("root", OBSERVE);
    let document = web_sys::window().unwrap().document().unwrap();
    for (name, bytes) in CASES {
        let html: String = WordPreview::load(bytes, Options::default())
            .unwrap()
            .render_sections()
            .iter()
            .map(HtmlElement::to_html)
            .collect();
        let root = document.create_element("div").unwrap();
        root.set_inner_html(&html);
        let observed = observe.call1(&JsValue::NULL, &root).unwrap();
        let observed: Value = serde_json::from_str(&observed.as_string().unwrap()).unwrap();
        let case = recorded.iter().find(|case| case["name"] == name).unwrap();
        let mut expected: Vec<Value> = serde_json::from_value(case["expected"].clone()).unwrap();
        corrections::correct(name, &mut expected);
        assert_eq!(observed, Value::from(expected), "{name}");
    }
}

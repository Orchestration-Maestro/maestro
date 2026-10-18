//! Shared export corpus observations.
mod corpus;
pub use corpus::{Row, rows};

pub use tests::assert_markup;
#[cfg(test)]
mod tests {
    /// Compare markup after only the approved syntax-engine substitution and emitter indentation.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn assert_markup(actual: &str, expected: &str, id: &str) {
        let spans = regress::Regex::new(r"</?span[^>]*>").unwrap();
        let project = |html: &str| {
            spans
                .replace_all(html, "")
                .replace("\n              ", "")
                .replace("&#10;", "\n")
        };
        assert_eq!(project(actual), project(expected), "{id}");
    }

    /// Compare native elements, literal attributes and ordered text through the shared DOM projection.
    #[cfg(target_arch = "wasm32")]
    pub fn assert_markup(actual: &str, expected: &str, id: &str) {
        use wasm_bindgen::JsValue;
        js_sys::eval(include_str!("../markdown_support/dom.js")).unwrap();
        let global = js_sys::global();
        js_sys::Reflect::set(&global, &"exportActual".into(), &JsValue::from_str(actual)).unwrap();
        js_sys::Reflect::set(
            &global,
            &"exportExpected".into(),
            &JsValue::from_str(&expected.replace("\n              ", "")),
        )
        .unwrap();
        let difference = js_sys::eval("(()=>{const a=document.createElement('div'),e=document.createElement('div');a.innerHTML=exportActual;e.innerHTML=exportExpected;for(const r of [a,e]){for(const s of r.querySelectorAll('code span'))s.replaceWith(...s.childNodes);r.normalize();}if([...a.querySelectorAll('*')].some(e=>[...e.attributes].some(a=>/^on/i.test(a.name))))throw Error('event-handler attribute survived');return maestro.diff(a,e.innerHTML)})()").unwrap().as_string().unwrap();
        assert_eq!(difference, "", "{id}");
    }
}

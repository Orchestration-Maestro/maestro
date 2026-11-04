//! One native failure injected at a selected presentation phase.
use crate::browser_fixture_support::{Patch, error, get, global, invoke};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// Page operations that throw once, at the first operation of the selected phase.
pub struct PresentationFault {
    /// Whether the selected phase was reached.
    fired: Rc<Cell<bool>>,
    /// Replaced page operations, restored when stopped.
    patches: Vec<Patch>,
}

/// The prototype of the page class `name`.
fn prototype(name: &str) -> JsValue {
    get(&global(name), "prototype")
}

/// Replace `method` on `class`'s prototype, throwing once when `reached` selects a call.
fn fail_once(
    class: &str,
    method: &str,
    fired: &Rc<Cell<bool>>,
    reached: impl Fn(&JsValue, &js_sys::Array) -> bool + 'static,
) -> Patch {
    let fired = Rc::clone(fired);
    Patch::method(&prototype(class), method, move |original, this, args| {
        if !fired.get() && reached(&this, &args) {
            fired.set(true);
            return Err(error("native phase marker"));
        }
        original.apply(&this, &args)
    })
}

/// Whether `value` is an element whose tag name is `tag`.
fn tagged(value: &JsValue, tag: &str) -> bool {
    value
        .dyn_ref::<Element>()
        .is_some_and(|element| element.tag_name() == tag)
}

impl PresentationFault {
    /// Throw at the first `phase` operation: `create`, `replacement`, `append`, `placement`,
    /// `content` or `visibility`.
    #[must_use]
    pub fn install(phase: &str) -> Self {
        let fired = Rc::new(Cell::new(false));
        let is = |selected: &'static str| phase == selected;
        let (create, replacement, append) = (is("create"), is("replacement"), is("append"));
        let (placement, content, visibility) = (is("placement"), is("content"), is("visibility"));
        let patches = vec![
            fail_once("Document", "createElement", &fired, move |_, args| {
                create && args.get(0) == "pre"
            }),
            fail_once("Node", "replaceChild", &fired, move |_, args| {
                replacement && tagged(&args.get(0), "PRE")
            }),
            fail_once("Node", "appendChild", &fired, move |this, args| {
                append
                    && get(this, "nodeName") == "BODY"
                    && invoke(&args.get(0), "querySelector", &[&"pre".into()])
                        .is_ok_and(|found| !found.is_null())
            }),
            fail_once(
                "Element",
                "getBoundingClientRect",
                &fired,
                move |this, _| placement && get(this, "id") == "canvas",
            ),
            fail_once("Element", "setAttribute", &fired, move |this, args| {
                let style = args.get(0) == "style";
                let pinned = args
                    .get(1)
                    .as_string()
                    .is_some_and(|value| value.contains("position:fixed"));
                style
                    && ((content && tagged(this, "PRE"))
                        || (visibility && tagged(this, "DIV") && pinned))
            }),
        ];
        Self { fired, patches }
    }

    /// Restore every patched browser method, returning whether the phase was reached.
    #[must_use]
    pub fn stop(self) -> bool {
        drop(self.patches);
        self.fired.get()
    }
}

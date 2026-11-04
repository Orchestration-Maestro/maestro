//! Native faults below the reading placement and disclosure registration.
use crate::browser_fixture_support::{Patch, error, get, global};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::Element;

/// The prototype of the page class `name`.
fn prototype(name: &str) -> JsValue {
    get(&global(name), "prototype")
}

/// What one placement fault observed after its hooks were removed.
pub(crate) struct Evidence {
    /// Indexes of the slots, viewport and frame whose original style was written back.
    pub(crate) restored: Vec<usize>,
    /// Viewport offsets written, in order.
    pub(crate) offsets: Vec<&'static str>,
}

/// The placement state a fault selects its target from.
struct Placement {
    /// The fault kind: a target and an optional `-before`/`-after` moment, or a readback.
    kind: String,
    /// Whether the first restoring write also fails.
    cleanup: bool,
    /// The measured slots, then the viewport wrapper and the frame.
    elements: Vec<Element>,
    /// The original style of each element.
    styles: Vec<Option<String>>,
    /// Whether the operation fault fired.
    fired: Cell<bool>,
    /// Whether the cleanup fault fired.
    cleanup_fired: Cell<bool>,
    /// Indexes whose original style was written back.
    restored: RefCell<Vec<usize>>,
    /// Viewport offsets written.
    offsets: RefCell<Vec<&'static str>>,
}

impl Placement {
    /// The frame element.
    fn frame(&self) -> &Element {
        &self.elements[self.elements.len() - 1]
    }

    /// Whether a style write of `text` to the element at `index` is the selected operation.
    fn selected(&self, index: Option<usize>, text: Option<&str>) -> bool {
        let target = self.kind.strip_prefix("publish-").unwrap_or(&self.kind);
        let target = target
            .strip_suffix("-before")
            .or_else(|| target.strip_suffix("-after"))
            .unwrap_or(target);
        let last = self.elements.len() - 1;
        let slot = target
            .strip_prefix("slot")
            .and_then(|number| number.parse::<usize>().ok());
        (slot.is_some() && index == slot)
            || (target == "frame" && index == Some(last))
            || (target == "viewport" && index == Some(last - 1))
            || (self.kind == "publish"
                && index == Some(last)
                && text.is_some_and(|text| text.contains("visibility:visible")))
    }

    /// Throw the operation marker once.
    fn fire(&self) -> Result<(), JsValue> {
        self.fired.set(true);
        Err(error("operation marker"))
    }

    /// Observe one attribute write or removal around `operation`, failing where selected.
    fn attribute(
        &self,
        element: &JsValue,
        name: &JsValue,
        text: Option<&str>,
        operation: impl FnOnce() -> Result<JsValue, JsValue>,
    ) -> Result<JsValue, JsValue> {
        let index = self
            .elements
            .iter()
            .position(|known| JsValue::from(known.clone()) == *element);
        let style = *name == "style";
        if let Some(index) = index.filter(|&index| style && text == self.styles[index].as_deref()) {
            self.restored.borrow_mut().push(index);
            let first = self.cleanup && !self.cleanup_fired.get() && index == 0;
            if first && !self.kind.contains("axis") {
                self.cleanup_fired.set(true);
                return Err(error("cleanup marker"));
            }
        }
        let matches = style && self.selected(index, text);
        if !self.fired.get() && matches && self.kind.ends_with("-before") {
            self.fire()?;
        }
        let result = operation()?;
        if !self.fired.get() && matches {
            self.fire()?;
        }
        Ok(result)
    }
}

/// Placement operations that fail once at the selected target, restored when stopped or dropped.
pub(crate) struct PlacementFault {
    /// The shared observation state.
    placement: Rc<Placement>,
    /// Replaced page operations, restored in reverse order.
    patches: Vec<Patch>,
}

impl PlacementFault {
    /// Fail the first `kind` operation below `root`; `cleanup` also fails the first restoration.
    pub(crate) fn install(root: &Element, kind: &str, cleanup: bool) -> Self {
        let wrapper = root.parent_element().unwrap();
        let frame = wrapper.parent_element().unwrap();
        let slots = root.query_selector_all(".test-slot").unwrap();
        let mut elements: Vec<Element> = (0..slots.length())
            .map(|index| slots.item(index).unwrap().unchecked_into())
            .collect();
        elements.extend([wrapper.clone(), frame]);
        let styles = elements.iter().map(|e| e.get_attribute("style")).collect();
        let placement = Rc::new(Placement {
            kind: kind.to_owned(),
            cleanup,
            elements,
            styles,
            fired: Cell::new(false),
            cleanup_fired: Cell::new(false),
            restored: RefCell::default(),
            offsets: RefCell::default(),
        });
        let mut patches = attribute_patches(&placement);
        patches.extend(readback_patches(&placement));
        patches.extend(["scrollLeft", "scrollTop"].map(|key| offset(&wrapper, key, &placement)));
        Self { placement, patches }
    }

    /// Restore every page operation and return what the fault observed.
    pub(crate) fn stop(self) -> Evidence {
        let placement = Rc::clone(&self.placement);
        drop(self);
        Evidence {
            restored: placement.restored.take(),
            offsets: placement.offsets.take(),
        }
    }
}

impl Drop for PlacementFault {
    fn drop(&mut self) {
        while let Some(patch) = self.patches.pop() {
            drop(patch);
        }
    }
}

/// Observe style writes and removals on every element.
fn attribute_patches(placement: &Rc<Placement>) -> Vec<Patch> {
    let element = prototype("Element");
    let written = Rc::clone(placement);
    let set = Patch::method(&element, "setAttribute", move |original, this, args| {
        let text = args.get(1).as_string();
        written.attribute(&this, &args.get(0), text.as_deref(), || {
            original.apply(&this, &args)
        })
    });
    let removed = Rc::clone(placement);
    let remove = Patch::method(&element, "removeAttribute", move |original, this, args| {
        removed.attribute(&this, &args.get(0), None, || original.apply(&this, &args))
    });
    vec![set, remove]
}

/// Fail the frame's first measurement in a readback kind, through a getter in `readback-getter`.
fn readback_patches(placement: &Rc<Placement>) -> Vec<Patch> {
    let element = prototype("Element");
    let original = get(&element, "getBoundingClientRect");
    let read = Rc::clone(placement);
    let mut patches = vec![Patch::method(
        &element,
        "getBoundingClientRect",
        move |original, this, args| {
            let result = original.apply(&this, &args)?;
            let frame: &JsValue = read.frame();
            if !read.fired.get() && read.kind.starts_with("readback") && this == *frame {
                read.fire()?;
            }
            Ok(result)
        },
    )];
    if placement.kind == "readback-getter" {
        let read = Rc::clone(placement);
        patches.push(Patch::getter(
            &element,
            "getBoundingClientRect",
            move |this| {
                let frame: &JsValue = read.frame();
                if !read.fired.get() && this == *frame {
                    read.fire()?;
                }
                Ok(original.clone())
            },
        ));
    }
    patches
}

/// Record viewport offset writes, failing the first horizontal one in an axis cleanup kind.
fn offset(wrapper: &Element, key: &'static str, placement: &Rc<Placement>) -> Patch {
    let descriptor = js_sys::Reflect::get_own_property_descriptor(
        &prototype("Element").unchecked_into::<js_sys::Object>(),
        &key.into(),
    )
    .unwrap();
    let (read, write) = (get(&descriptor, "get"), get(&descriptor, "set"));
    let placement = Rc::clone(placement);
    Patch::accessor(
        wrapper,
        key,
        move |this| read.unchecked_ref::<js_sys::Function>().call0(&this),
        move |this, value| {
            placement.offsets.borrow_mut().push(key);
            let axis = placement.cleanup && placement.kind.contains("axis") && key == "scrollLeft";
            if placement.fired.get() && axis && !placement.cleanup_fired.get() {
                placement.cleanup_fired.set(true);
                return Err(error("cleanup marker"));
            }
            write
                .unchecked_ref::<js_sys::Function>()
                .call1(&this, &value)
                .map(|_| ())
        },
    )
}

/// Registered or removed event targets with their callbacks, in order.
type Pairs = Vec<(JsValue, JsValue)>;

/// Disclosure click registrations observed while installed.
pub(crate) struct Registrations {
    /// Registered targets and callbacks.
    added: Rc<RefCell<Pairs>>,
    /// Removed targets and callbacks.
    removed: Rc<RefCell<Pairs>>,
    /// The replaced registration and removal.
    patches: [Patch; 2],
}

/// Whether `target` is a disclosure control.
fn disclosure(target: &JsValue) -> bool {
    target.dyn_ref::<Element>().is_some_and(|element| {
        element
            .matches(".expandable,.tool-item:has(.tool-params-hint)")
            .unwrap()
    })
}

impl Registrations {
    /// Fail the second disclosure registration and, when `failed_removal`, every removal.
    pub(crate) fn install(failed_removal: bool) -> Self {
        let target = prototype("EventTarget");
        let added: Rc<RefCell<Pairs>> = Rc::default();
        let removed: Rc<RefCell<Pairs>> = Rc::default();
        let (registered, count) = (Rc::clone(&added), Cell::new(0));
        let add = Patch::method(&target, "addEventListener", move |original, this, args| {
            let observed = args.get(0) == "click" && disclosure(&this);
            if observed {
                count.set(count.get() + 1);
            }
            if observed && count.get() == 2 {
                return Err(error("registration marker"));
            }
            if observed {
                registered.borrow_mut().push((this.clone(), args.get(1)));
            }
            original.apply(&this, &args)
        });
        let released = Rc::clone(&removed);
        let remove = Patch::method(
            &target,
            "removeEventListener",
            move |original, this, args| {
                let observed = args.get(0) == "click" && disclosure(&this);
                if observed {
                    released.borrow_mut().push((this.clone(), args.get(1)));
                }
                if observed && failed_removal {
                    return Err(error("removal marker"));
                }
                original.apply(&this, &args)
            },
        );
        Self {
            added,
            removed,
            patches: [add, remove],
        }
    }

    /// Restore registration and return the registered and removed pairs.
    pub(crate) fn stop(self) -> (Pairs, Pairs) {
        drop(self.patches);
        (self.added.take(), self.removed.take())
    }
}

/// A one-shot failure of a header candidate replacing the published `root`.
pub(crate) struct CandidateFault {
    /// Whether the fault fired.
    fired: Rc<Cell<bool>>,
    /// The replaced attribute write and measurement.
    patches: [Patch; 2],
}

impl CandidateFault {
    /// Fail the candidate's `kind` step: `mount`, `read` or `publish`.
    pub(crate) fn install(root: &Element, kind: &str) -> Self {
        let element = prototype("Element");
        let fired = Rc::new(Cell::new(false));
        let root = JsValue::from(root.clone());
        let candidate = move |target: &JsValue| {
            let header = ".maestro-session-viewer-header";
            let target: &Element = target.unchecked_ref();
            JsValue::from(target.clone()) != root
                && (target.matches(header).unwrap()
                    || target.query_selector(header).unwrap().is_some())
        };
        let candidate = Rc::new(candidate);
        let (kind, written, writing) = (kind.to_owned(), Rc::clone(&fired), Rc::clone(&candidate));
        let read_kind = kind.clone();
        let set = Patch::method(&element, "setAttribute", move |original, this, args| {
            let (name, text) = (args.get(0), args.get(1).as_string().unwrap_or_default());
            let mount = kind == "mount" && name == "class" && text == "system-prompt expandable";
            let publish = kind == "publish"
                && name == "style"
                && writing(&this)
                && !text.contains("visibility:hidden")
                && text.contains("height:");
            if !written.get() && (mount || publish) {
                written.set(true);
                return Err(error("candidate marker"));
            }
            original.apply(&this, &args)
        });
        let read = Rc::clone(&fired);
        let bounds = Patch::method(
            &element,
            "getBoundingClientRect",
            move |original, this, args| {
                if !read.get() && read_kind == "read" && candidate(&this) {
                    read.set(true);
                    return Err(error("read marker"));
                }
                original.apply(&this, &args)
            },
        );
        Self {
            fired,
            patches: [set, bounds],
        }
    }

    /// Restore the page and require the fault to have fired.
    pub(crate) fn stop(self) {
        drop(self.patches);
        assert!(self.fired.get(), "candidate fault not reached");
    }
}

/// The text of the first `selector` match below `root`, or null.
fn text(root: &Element, selector: &str) -> serde_json::Value {
    root.query_selector(selector)
        .unwrap()
        .and_then(|found| found.text_content())
        .map_or(serde_json::Value::Null, serde_json::Value::String)
}

/// Every `selector` match below `root`, in document order.
fn all(root: &Element, selector: &str) -> Vec<Element> {
    let found = root.query_selector_all(selector).unwrap();
    (0..found.length())
        .map(|index| found.item(index).unwrap().unchecked_into())
        .collect()
}

/// The system prompt and tool disclosures below `root`, as the recorded corpus projects them.
pub(crate) fn projection(root: &Element) -> serde_json::Value {
    let prompt = root
        .query_selector(".system-prompt")
        .unwrap()
        .map(|prompt| {
            serde_json::json!({
                "expandable": prompt.class_list().contains("expandable"),
                "heading": text(&prompt, ".system-prompt-header"),
                "preview": text(&prompt, ".system-prompt-preview"),
                "hint": text(&prompt, ".system-prompt-expand-hint"),
                "full": text(&prompt, ".system-prompt-full"),
            })
        });
    let tools: Vec<serde_json::Value> = all(root, ".tool-item").iter().map(tool).collect();
    serde_json::json!({"prompt": prompt, "tools": tools})
}

/// One tool's name, description, parameter hint and parameters.
fn tool(tool: &Element) -> serde_json::Value {
    let hint = tool.query_selector(".tool-params-hint").unwrap();
    let marker = hint.as_ref().map(|hint| {
        let style = web_sys::window()
            .unwrap()
            .get_computed_style_with_pseudo_elt(hint, "::after")
            .unwrap()
            .unwrap();
        let content = style.get_property_value("content").unwrap();
        serde_json::from_str::<serde_json::Value>(&content).unwrap()
    });
    let parameters: Vec<serde_json::Value> = all(tool, ".tool-param")
        .iter()
        .map(|parameter| {
            serde_json::json!({
                "name": text(parameter, ".tool-param-name"),
                "type": text(parameter, ".tool-param-type"),
                "required": parameter.query_selector(".tool-param-required").unwrap().is_some(),
                "description": text(parameter, ".tool-param-desc"),
            })
        })
        .collect();
    serde_json::json!({
        "name": text(tool, ".tool-item-name"),
        "description": text(tool, ".tool-item-desc"),
        "expandable": hint.is_some(),
        "hint": marker,
        "parameters": parameters,
    })
}

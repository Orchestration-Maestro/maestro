//! Native page content mounted once and updated in place.
use crate::{I18n, tools::types::ToolUiError};
use std::{
    any::{Any, TypeId},
    rc::Rc,
};
use web_sys::{Document, Node};

/// Page-wide operands shared by mounted content and built-in elements.
#[derive(Clone)]
pub struct DomView {
    /// Document receiving mounted roots.
    pub document: Document,
    /// Current localization.
    pub i18n: Rc<I18n>,
    /// Shared syntax engine.
    pub highlighter: Rc<dyn maestro_theme::SyntaxHighlighter>,
    /// Selected brand pack.
    pub pack: Rc<maestro_theme::BrandPack>,
    /// Selected brand mode.
    pub mode: maestro_theme::BrandMode,
}
/// A mounted root and the native resources that live exactly as long as it.
pub struct DomRoot {
    /// Root node the caller places.
    pub node: Node,
    /// Resources released when the root is replaced or dropped, such as an isolated frame.
    pub owner: Option<Box<dyn Any>>,
}
impl DomRoot {
    /// A root that owns no further resources.
    #[must_use]
    pub fn new(node: impl Into<Node>) -> Self {
        Self {
            node: node.into(),
            owner: None,
        }
    }
}
/// Content that mounts its native root once and updates that root while its type and key stay selected.
pub trait DomContent: Any {
    /// Create the root.
    /// # Errors
    /// Returns a native or presentation failure.
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError>;
    /// Bring a root mounted by content of this type and key up to date.
    /// # Errors
    /// Returns a native or presentation failure.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError>;
    /// Separates contents of one type whose roots cannot update each other, such as elements of different tags.
    fn key(&self) -> Option<&str> {
        None
    }
}
/// One placed root and the content type and key that own it.
#[derive(Default)]
pub struct MountedContent {
    /// Owning content type, its key and its root.
    active: Option<(TypeId, Option<String>, DomRoot)>,
}
impl MountedContent {
    /// The current root, if any.
    #[must_use]
    pub fn root(&self) -> Option<&Node> {
        self.active.as_ref().map(|(.., root)| &root.node)
    }
    /// Update the root in place, or replace it under `parent` when the content type or key changed.
    ///
    /// A replaced root's owned resources are released once the new root is placed.
    /// # Errors
    /// Returns the content's failure; a failed replacement keeps the previous root.
    #[cfg(target_arch = "wasm32")]
    pub fn show(
        &mut self,
        content: &dyn DomContent,
        parent: &Node,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        self.show_before(content, parent, None, view)
    }
    /// Like [`Self::show`], but a first root is placed before `before` instead of last.
    /// # Errors
    /// Returns the content's failure; a failed replacement keeps the previous root.
    #[cfg(target_arch = "wasm32")]
    pub fn show_before(
        &mut self,
        content: &dyn DomContent,
        parent: &Node,
        before: Option<&Node>,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        let kind = (content as &dyn Any).type_id();
        if let Some((active, key, root)) = &self.active
            && *active == kind
            && key.as_deref() == content.key()
        {
            return content.update(root, view);
        }
        let root = content.mount(view)?;
        match &self.active {
            Some((.., old)) => parent.replace_child(&root.node, &old.node),
            None => parent.insert_before(&root.node, before),
        }
        .map_err(|error| ToolUiError::native(&error))?;
        self.active = Some((kind, content.key().map(str::to_owned), root));
        Ok(())
    }
    /// Remove the root element from its parent and release its resources.
    #[cfg(target_arch = "wasm32")]
    pub fn clear(&mut self) {
        use wasm_bindgen::JsCast as _;
        if let Some((.., root)) = self.active.take() {
            root.node.unchecked_ref::<web_sys::Element>().remove();
        }
    }
}
/// An element property reflected onto one of its attributes.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
pub(crate) enum Property {
    /// A string property holding the attribute's text; a non-string removes the attribute.
    Text(&'static str, &'static str),
    /// A boolean property holding the attribute's presence.
    Flag(&'static str, &'static str),
    /// A boolean property holding the attribute's absence, for properties that default to true.
    Unset(&'static str, &'static str),
}
/// A property getter callable.
#[cfg(target_arch = "wasm32")]
pub(crate) type Getter = wasm_bindgen::closure::Closure<dyn Fn() -> wasm_bindgen::JsValue>;
/// A property setter callable.
#[cfg(target_arch = "wasm32")]
pub(crate) type Setter = wasm_bindgen::closure::Closure<dyn Fn(wasm_bindgen::JsValue)>;
/// A weak reference to one built-in element, which a callback must not keep alive.
#[cfg(target_arch = "wasm32")]
pub(crate) type Link = js_sys::WeakRef<web_sys::HtmlElement>;
/// Property accessors installed on one element, kept as long as its state.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Reflected {
    /// Getter and setter callables, released with the element's state.
    _accessors: Vec<(Getter, Setter)>,
}
/// The element behind `link` while it exists.
#[cfg(target_arch = "wasm32")]
pub(crate) fn live(link: &Link) -> Option<web_sys::HtmlElement> {
    link.deref()
}
/// The element behind `link` while it exists and is still connected.
#[cfg(target_arch = "wasm32")]
pub(crate) fn connected(link: &Link) -> Option<web_sys::HtmlElement> {
    live(link).filter(|element| element.is_connected())
}
/// Install `get` and `set` as the configurable property `name` of `element`, reporting success.
#[cfg(target_arch = "wasm32")]
pub(crate) fn install(
    element: &web_sys::HtmlElement,
    name: &str,
    get: &Getter,
    set: &Setter,
) -> bool {
    use wasm_bindgen::JsValue;
    let descriptor = js_sys::Object::new();
    for (key, value) in [("get", get.as_ref()), ("set", set.as_ref())] {
        let _ = js_sys::Reflect::set(&descriptor, &key.into(), value);
    }
    let _ = js_sys::Reflect::set(&descriptor, &"configurable".into(), &JsValue::TRUE);
    js_sys::Reflect::define_property(element, &name.into(), &descriptor).is_ok()
}
/// Install accessors for `properties` on `element`, so setting one changes its observed attribute.
#[cfg(target_arch = "wasm32")]
fn reflect(element: &web_sys::HtmlElement, properties: &[Property]) -> Reflected {
    use wasm_bindgen::JsValue;
    let mut kept = Vec::new();
    for &property in properties {
        let (name, attribute, flag) = match property {
            Property::Text(name, attribute) => (name, attribute, None),
            Property::Flag(name, attribute) => (name, attribute, Some(true)),
            Property::Unset(name, attribute) => (name, attribute, Some(false)),
        };
        let weak = js_sys::WeakRef::new(element);
        let read = weak.clone();
        let get = Getter::new(move || match (live(&read), flag) {
            (Some(element), Some(present)) => (element.has_attribute(attribute) == present).into(),
            (Some(element), None) => element.get_attribute(attribute).unwrap_or_default().into(),
            (None, _) => JsValue::UNDEFINED,
        });
        let set = Setter::new(move |value: JsValue| {
            let Some(element) = live(&weak) else { return };
            let text = match flag {
                Some(present) => (value.is_truthy() == present).then(String::new),
                None => value.as_string(),
            };
            // Attribute changes on a live element only fail for invalid names, which these are not.
            let _ = match text {
                Some(text) => element.set_attribute(attribute, &text),
                None => element.remove_attribute(attribute),
            };
        });
        if install(element, name, &get, &set) {
            kept.push((get, set));
        }
    }
    Reflected { _accessors: kept }
}
/// Install the built-in elements' stylesheet once per document, with the selected pack's properties.
/// # Errors
/// Returns a presentation, highlighting or native failure.
#[cfg(target_arch = "wasm32")]
pub(crate) fn install_styles(view: &DomView) -> Result<(), wasm_bindgen::JsValue> {
    let failure =
        |error: &dyn std::fmt::Display| wasm_bindgen::JsValue::from_str(&error.to_string());
    let presentation = view
        .pack
        .presentation(view.mode)
        .map_err(|error| failure(&error))?;
    let mut css = String::from(":root{");
    crate::browser::append_brand_properties(&mut css, &presentation);
    css.push('}');
    css.push_str(crate::icons::ICON_STYLES);
    css.push_str(include_str!("components/code_block/code.css"));
    css.push_str(&maestro_theme::highlight_css().map_err(|error| failure(&error))?);
    css.push_str(include_str!("components/markdown_block/markdown.css"));
    css.push_str(include_str!("components/messages.css"));
    css.push_str(include_str!("tools/artifacts/artifact_element.css"));
    css.push_str(include_str!("tools/artifacts/generic_artifact.css"));
    css.push_str(include_str!("tools/artifacts/docx_artifact.css"));
    css.push_str(include_str!("tools/artifacts/console.css"));
    css.push_str(include_str!("components/diff.css"));
    css.push_str(include_str!("dom.css"));
    crate::browser::install_stylesheet(&view.document, "maestro-dom-styles", &css)
        .map_err(|error| failure(&error))
}
/// Define an autonomous element whose constructor callback lives as long as the page.
///
/// Each instance gets accessors for `properties`, which `construct` receives to keep.
/// # Errors
/// Returns the browser's registration failure, including a repeated name.
#[cfg(target_arch = "wasm32")]
pub(crate) fn define_element<E: custom_element::CustomElement + 'static>(
    name: &str,
    observed: Vec<String>,
    properties: &'static [Property],
    construct: impl Fn(web_sys::HtmlElement, Reflected) -> E + 'static,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
    /// Constructor callbacks retained for the page lifetime.
    type Constructor = Closure<dyn FnMut(JsValue, js_sys::Array) -> JsValue>;
    thread_local! {
        static CONSTRUCTORS: std::cell::RefCell<Vec<Constructor>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window"))?;
    let (closure, constructor) = custom_element::create_custom_element(
        move |instance, _| {
            let element: web_sys::HtmlElement = instance.unchecked_into();
            let reflected = reflect(&element, properties);
            construct(element, reflected)
        },
        observed,
    );
    window.custom_elements().define(name, &constructor)?;
    CONSTRUCTORS.with(|all| all.borrow_mut().push(closure));
    Ok(())
}

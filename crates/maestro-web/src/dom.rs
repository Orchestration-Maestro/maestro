//! Native page content mounted once and updated in place.
use crate::{I18n, tools::types::ToolUiError};
use std::{
    any::{Any, TypeId},
    rc::Rc,
};
use web_sys::{Document, Node};

/// Page-wide operands shared by mounted content and built-in elements.
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
/// Content that mounts its native root once and updates that root while its type stays selected.
pub trait DomContent: Any {
    /// Create the root.
    /// # Errors
    /// Returns a native or presentation failure.
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError>;
    /// Bring a root this content type mounted up to date.
    /// # Errors
    /// Returns a native or presentation failure.
    fn update(&self, root: &Node, view: &DomView) -> Result<(), ToolUiError>;
}
/// One placed root and the content type that owns it.
#[derive(Default)]
pub struct MountedContent {
    /// Owning content type and its root.
    active: Option<(TypeId, Node)>,
}
impl MountedContent {
    /// The current root, if any.
    #[must_use]
    pub fn root(&self) -> Option<&Node> {
        self.active.as_ref().map(|(_, root)| root)
    }
    /// Update the root in place, or replace it under `parent` when the content type changed.
    /// # Errors
    /// Returns the content's failure; a failed replacement keeps the previous root.
    #[cfg(target_arch = "wasm32")]
    pub fn show(
        &mut self,
        content: &dyn DomContent,
        parent: &Node,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        let kind = (content as &dyn Any).type_id();
        if let Some((active, root)) = &self.active
            && *active == kind
        {
            return content.update(root, view);
        }
        let root = content.mount(view)?;
        match &self.active {
            Some((_, old)) => parent.replace_child(&root, old),
            None => parent.append_child(&root),
        }
        .map_err(|e| {
            ToolUiError::Host(crate::browser::attachment_content::browser::host_error(&e))
        })?;
        self.active = Some((kind, root));
        Ok(())
    }
}
/// Define an autonomous element whose constructor callback lives as long as the page.
/// # Errors
/// Returns the browser's registration failure, including a repeated name.
#[cfg(target_arch = "wasm32")]
pub(crate) fn define_element<E: custom_element::CustomElement + 'static>(
    name: &str,
    observed: Vec<String>,
    construct: impl Fn(web_sys::HtmlElement) -> E + 'static,
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
        move |instance, _| construct(instance.unchecked_into()),
        observed,
    );
    window.custom_elements().define(name, &constructor)?;
    CONSTRUCTORS.with(|all| all.borrow_mut().push(closure));
    Ok(())
}

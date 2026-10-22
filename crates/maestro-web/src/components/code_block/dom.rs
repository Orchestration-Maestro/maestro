//! Native highlighted code element with a language label and copy control.
use super::Prepared;
use crate::{
    browser::attachment_content::{
        AttachmentHostError,
        browser::{element, host_error},
    },
    dom::DomView,
};
use js_sys::WeakRef;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::HtmlElement;

/// Element tag of the code listing.
pub(crate) const TAG: &str = "code-block";
/// One code listing; its `code` and `language` attributes are observed.
struct CodeElement {
    /// Element owning this state.
    root: WeakRef,
    /// Page operands.
    view: Rc<DomView>,
}
impl custom_element::CustomElement for CodeElement {
    fn connected_callback(&mut self) {
        // Lifecycle callbacks have no error channel; a failed render keeps the previous listing.
        let _ = self.render();
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if self
            .root
            .deref()
            .is_some_and(|root| root.unchecked_into::<HtmlElement>().is_connected())
        {
            let _ = self.render();
        }
    }
}
impl CodeElement {
    /// Rebuild the label, copy control and highlighted listing from the attributes.
    fn render(&self) -> Result<(), AttachmentHostError> {
        let Some(root) = self
            .root
            .deref()
            .and_then(|r| r.dyn_into::<HtmlElement>().ok())
        else {
            return Ok(());
        };
        let presentation =
            |error: &dyn std::fmt::Display| AttachmentHostError::Presentation(error.to_string());
        let text = root.get_attribute("code").unwrap_or_default();
        let language = root.get_attribute("language").unwrap_or_default();
        let prepared = Prepared::new(&*self.view.highlighter, text, language)
            .map_err(|error| presentation(&error))?;
        let document = &self.view.document;
        let frame = element(document, "div", "maestro-code-block")?;
        let bar = element(document, "div", "maestro-code-block-bar")?;
        let label = element(document, "span", "maestro-code-block-language")?;
        label.set_text_content(Some(prepared.label()));
        let title = self
            .view
            .i18n
            .text("Copy code")
            .map_err(|error| presentation(&error))?;
        let copy = document
            .create_element(crate::components::copy_button::dom::TAG)
            .map_err(|error| host_error(&error))?;
        for (name, value) in [
            ("text", prepared.text.as_str()),
            ("title", &title),
            ("show-text", ""),
        ] {
            copy.set_attribute(name, value)
                .map_err(|error| host_error(&error))?;
        }
        let pre = element(document, "pre", "maestro-code-block-listing")?;
        let code = document
            .create_element("code")
            .map_err(|error| host_error(&error))?;
        code.set_inner_html(&prepared.html);
        let attach = |parent: &web_sys::Element, children: &[&web_sys::Element]| {
            children
                .iter()
                .try_for_each(|child| parent.append_child(child).map(drop))
                .map_err(|error| host_error(&error))
        };
        attach(&bar, &[&label, &copy])?;
        attach(&pre, &[&code])?;
        attach(&frame, &[&bar, &pre])?;
        root.replace_children_with_node_1(&frame);
        Ok(())
    }
}
/// Define the code listing once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    crate::dom::define_element(TAG, vec!["code".into(), "language".into()], move |root| {
        CodeElement {
            root: WeakRef::new(&root),
            view: Rc::clone(&view),
        }
    })
}

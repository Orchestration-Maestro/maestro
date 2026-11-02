//! Native highlighted code element with a language label and copy control.
use super::{Prepared, browser, decode};
use crate::{
    browser::attachment_content::{
        AttachmentHostError,
        browser::{append, host_error},
    },
    components::copy_button::dom::{self as copy_button, Localization},
    dom::{DomView, Link, Property, Reflected},
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Element, HtmlElement};

/// Element tag of the code listing.
pub(crate) const TAG: &str = "code-block";
/// Properties reflected onto the observed attributes.
const PROPERTIES: &[Property] = &[
    Property::Text("code", "code"),
    Property::Text("language", "language"),
];
/// One code listing; its `code` (base64 or literal) and `language` attributes are observed.
struct CodeElement {
    /// Element owning this state.
    root: Link,
    /// Page operands.
    view: Rc<DomView>,
    /// The built listing, changed in place on later renders.
    shown: Option<Listing>,
    /// Accessors of the `code` and `language` properties.
    _properties: Reflected,
}
impl custom_element::CustomElement for CodeElement {
    fn connected_callback(&mut self) {
        // Lifecycle callbacks have no error channel; a failed render keeps the previous listing.
        let _ = self.render();
    }
    fn attribute_changed_callback(&mut self, _: &str, _: JsValue, _: JsValue) {
        if crate::dom::connected(&self.root).is_some() {
            let _ = self.render();
        }
    }
}
impl CodeElement {
    /// Show the attributes' listing, changing only what differs once it is built.
    fn render(&mut self) -> Result<(), AttachmentHostError> {
        let Some(root) = self
            .root
            .deref()
            .and_then(|r| r.dyn_into::<HtmlElement>().ok())
        else {
            return Ok(());
        };
        let presentation =
            |error: &dyn std::fmt::Display| AttachmentHostError::Presentation(error.to_string());
        let text = decode(&root.get_attribute("code").unwrap_or_default());
        let language = root.get_attribute("language").unwrap_or_default();
        let prepared = Prepared::new(&*self.view.highlighter, text, language)
            .map_err(|error| presentation(&error))?;
        let title = self
            .view
            .i18n
            .text("Copy code")
            .map_err(|error| presentation(&error))?;
        if let Some(listing) = &self.shown {
            return listing.update(&self.view, &prepared, &title);
        }
        let (node, listing) = Listing::build(&self.view, &prepared, &title)?;
        root.replace_children_with_node_1(&node);
        self.shown = Some(listing);
        Ok(())
    }
}
/// A built listing and its copy control.
pub(crate) struct Listing {
    /// Updatable label and body.
    mounted: browser::Mounted,
    /// Copy control receiving the listing text and title.
    copy: Element,
    /// Localization of the copy control's labels.
    i18n: Localization,
}
impl Listing {
    /// Build `prepared` with a copy control titled `title`; the caller places the returned node.
    pub(crate) fn build(
        view: &DomView,
        prepared: &Prepared,
        title: &str,
    ) -> Result<(Element, Self), AttachmentHostError> {
        let built = browser::build(&view.document, prepared)?;
        let i18n = Rc::new(RefCell::new(Rc::clone(&view.i18n)));
        let copy = copy_button::create(&view.document, &i18n, copy_button::FEEDBACK_MS, None, None)
            .map_err(|error| host_error(&error))?;
        for (name, value) in [
            ("text", prepared.text.as_str()),
            ("title", title),
            ("show-text", ""),
        ] {
            copy.set_attribute(name, value)
                .map_err(|error| host_error(&error))?;
        }
        append(&built.slot, &copy)?;
        let listing = Self {
            mounted: built.mounted,
            copy,
            i18n,
        };
        Ok((built.root, listing))
    }
    /// Change only the label, body, text and title that differ, retaining selection and feedback.
    ///
    /// The copy control's labels follow `view`'s localization from now on.
    pub(crate) fn update(
        &self,
        view: &DomView,
        prepared: &Prepared,
        title: &str,
    ) -> Result<(), AttachmentHostError> {
        self.mounted.update(prepared);
        self.i18n.replace(Rc::clone(&view.i18n));
        for (name, value) in [("text", prepared.text.as_str()), ("title", title)] {
            if self.copy.get_attribute(name).as_deref() != Some(value) {
                self.copy
                    .set_attribute(name, value)
                    .map_err(|error| host_error(&error))?;
            }
        }
        Ok(())
    }
}
/// Define the code listing once for the page.
/// # Errors
/// Returns the browser's registration failure.
pub(crate) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    let observed = vec!["code".into(), "language".into()];
    crate::dom::define_element(TAG, observed, PROPERTIES, move |root, properties| {
        CodeElement {
            root: Link::new(&root),
            view: Rc::clone(&view),
            shown: None,
            _properties: properties,
        }
    })
}

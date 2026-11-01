//! Fallible literal listings mounted through the shared code and copy owners.
use super::Listing;
use crate::{
    components::code_block::{Prepared, dom::Listing as MountedListing},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use wasm_bindgen::JsCast as _;
use web_sys::Element;

impl Listing {
    /// Highlight literal source with the current caller's engine before publication.
    fn prepare(&self, view: &DomView) -> Result<Prepared, ToolUiError> {
        Prepared::new(&*view.highlighter, self.code.clone(), self.language.into())
            .map_err(ToolUiError::Reading)
    }
}
impl DomContent for Listing {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let prepared = self.prepare(view)?;
        let title = view.i18n.text("Copy code")?;
        let (node, listing) = MountedListing::build(view, &prepared, &title)?;
        node.set_attribute("language", self.language)
            .map_err(|error| ToolUiError::native(&error))?;
        Ok(DomRoot {
            node: node.into(),
            owner: Some(Box::new(listing)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let listing = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<MountedListing>())
            .ok_or_else(|| ToolUiError::Presentation("Missing listing state".into()))?;
        let prepared = self.prepare(view)?;
        let title = view.i18n.text("Copy code")?;
        listing.update(view, &prepared, &title)?;
        root.node
            .unchecked_ref::<Element>()
            .set_attribute("language", self.language)
            .map_err(|error| ToolUiError::native(&error))
    }
}

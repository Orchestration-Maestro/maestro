//! Page-content presentation of the default input and output listings.
use super::{DefaultContent, Listing, prepare};
use crate::{
    browser::attachment_content::browser::{element, host_error},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::cell::RefCell;
use wasm_bindgen::JsCast as _;
use web_sys::Element;

/// A labelled section retaining its listing's native resources.
struct Section {
    /// Section placed after the header.
    node: Element,
    /// Highlighted body and copy-control owner.
    listing: DomRoot,
}
/// Independently selected input and output sections.
type Sections = RefCell<[Option<Section>; 2]>;

impl DomContent for DefaultContent {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-tool-default")?.into(),
            owner: Some(Box::new(Sections::default())),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Keep the containing root and each still-selected listing's native resources.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let sections = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Sections>())
            .ok_or_else(|| ToolUiError::Presentation("Missing default tool state".into()))?;
        let node: &Element = root.node.unchecked_ref();
        let no_output = view.i18n.text("(no output)")?;
        let prepared = prepare(
            self.params.as_deref(),
            self.output.as_deref(),
            self.state,
            &no_output,
        )?;
        let header = crate::tools::renderer_registry::header::dom::render_header_element(
            view,
            self.state,
            "Code",
            prepared.caption,
        )?;
        for (index, (name, label, listing)) in [
            ("input", "Input", &prepared.input),
            ("output", "Output", &prepared.output),
        ]
        .into_iter()
        .enumerate()
        {
            show_section(
                node,
                &mut sections.borrow_mut()[index],
                (name, label),
                listing.as_ref(),
                view,
            )?;
        }
        match node
            .query_selector(":scope > .maestro-tool-header")
            .map_err(|error| host_error(&error))?
        {
            Some(old) => node.replace_child(&header, &old),
            None => node.insert_before(&header, node.first_child().as_ref()),
        }
        .map_err(|error| host_error(&error))?;
        Ok(())
    }
}
/// Update or replace one selected section, mounting fallible children before publication.
fn show_section(
    root: &Element,
    section: &mut Option<Section>,
    (name, label): (&str, &str),
    listing: Option<&Listing>,
    view: &DomView,
) -> Result<(), ToolUiError> {
    match (section.as_ref(), listing) {
        (Some(section), Some(listing)) => listing.update(&section.listing, view),
        (Some(old), None) => {
            old.node.remove();
            *section = None;
            Ok(())
        }
        (None, Some(listing)) => {
            let candidate = create_section(view, (name, label), listing)?;
            let anchor = if name == "input" {
                root.query_selector(":scope > [data-section=output]")
                    .map_err(|error| host_error(&error))?
            } else {
                None
            };
            root.insert_before(&candidate.node, anchor.as_ref().map(AsRef::as_ref))
                .map_err(|error| host_error(&error))?;
            *section = Some(candidate);
            Ok(())
        }
        (None, None) => Ok(()),
    }
}
/// Build a labelled listing without attaching it to the published content.
fn create_section(
    view: &DomView,
    (name, label): (&str, &str),
    listing: &Listing,
) -> Result<Section, ToolUiError> {
    let document = &view.document;
    let node = element(document, "div", "")?;
    node.set_attribute("data-section", name)
        .map_err(|error| host_error(&error))?;
    let caption = element(document, "div", "maestro-tool-section-label")?;
    caption.set_text_content(Some(&view.i18n.text(label)?));
    let listing = listing.mount(view)?;
    node.append_with_node_2(&caption, &listing.node)
        .map_err(|error| host_error(&error))?;
    Ok(Section { node, listing })
}

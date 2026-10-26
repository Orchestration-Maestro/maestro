//! Selectable literal error text with one styling owner.
use super::Text;
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{host_error, sync_text},
};
use std::{cell::RefCell, rc::Rc};
/// Mount a centered panel with literal heading and diagnostic.
pub(in crate::dialogs::attachment_overlay) fn mount(
    document: &web_sys::Document,
    text: &Rc<RefCell<Text>>,
    padded: bool,
) -> Result<ReadingRoot, AttachmentHostError> {
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        commit: None,
        owner: None,
        element: root(document, text, padded)?,
        slots: vec![],
    })
}
/// Build a literal error root without a retained-placement wrapper.
pub(in crate::dialogs::attachment_overlay) fn root(
    document: &web_sys::Document,
    text: &Rc<RefCell<Text>>,
    padded: bool,
) -> Result<web_sys::Element, AttachmentHostError> {
    let (root, nodes) = nodes(document, padded)?;
    text.borrow_mut().nodes = Some(nodes);
    {
        let text = text.borrow();
        sync_text(text.nodes.as_ref(), &text.values);
    }
    Ok(root)
}

/// Build literal error nodes independently of retained placement.
pub(crate) fn nodes(
    document: &web_sys::Document,
    padded: bool,
) -> Result<(web_sys::Element, [web_sys::Element; 2]), AttachmentHostError> {
    let create = |tag| document.create_element(tag).map_err(|e| host_error(&e));
    let root = create("section")?;
    root.set_attribute(
        "class",
        if padded {
            "maestro-file-error maestro-file-error-artifact"
        } else {
            "maestro-file-error"
        },
    )
    .map_err(|e| host_error(&e))?;
    let style = create("style")?;
    style.set_text_content(Some(include_str!("../error.css")));
    root.append_child(&style).map_err(|e| host_error(&e))?;
    let panel = create("div")?;
    let nodes = [create("h3")?, create("p")?];
    for node in &nodes {
        panel.append_child(node).map_err(|e| host_error(&e))?;
    }
    root.append_child(&panel).map_err(|e| host_error(&e))?;
    Ok((root, nodes))
}

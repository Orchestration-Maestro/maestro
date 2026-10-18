//! Selectable literal error text with one styling owner.
use super::Text;
use crate::browser::attachment_content::{
    AttachmentUiError, ReadingRoot,
    browser::{host_error, set_text_if_changed},
};
use std::{cell::RefCell, rc::Rc};
/// Update changed text only, preserving unchanged native selection.
pub(in crate::dialogs::attachment_overlay) fn sync(text: &Text) {
    if let Some(nodes) = &text.nodes {
        for (node, value) in nodes.iter().zip(&text.values) {
            set_text_if_changed(node, value);
        }
    }
}
/// Mount a centered panel with literal heading and diagnostic.
pub(in crate::dialogs::attachment_overlay) fn mount(
    document: &web_sys::Document,
    text: &Rc<RefCell<Text>>,
    padded: bool,
) -> Result<ReadingRoot, AttachmentUiError> {
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
    text.borrow_mut().nodes = Some(nodes);
    sync(&text.borrow());
    Ok(ReadingRoot {
        commit: None,
        owner: None,
        element: root,
        slots: vec![],
    })
}

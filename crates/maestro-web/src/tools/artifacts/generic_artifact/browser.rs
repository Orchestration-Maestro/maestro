//! Literal native artifact text with stable nodes.
use super::Text;
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{host_error, sync_text},
};
use std::{cell::RefCell, rc::Rc};
use web_sys::Element;
/// Build the literal body nodes, returning its root and its owned text nodes.
pub(super) fn body(
    document: &web_sys::Document,
) -> Result<(Element, [Element; 3]), AttachmentHostError> {
    let create = |tag| document.create_element(tag).map_err(|e| host_error(&e));
    let root = create("section")?;
    root.set_attribute("class", "maestro-generic-artifact")
        .map_err(|e| host_error(&e))?;
    let body = create("div")?;
    let icon = icon(document)?;
    body.append_child(&icon).map_err(|e| host_error(&e))?;
    let nodes = [create("div")?, create("span")?, create("span")?];
    body.append_child(&nodes[0]).map_err(|e| host_error(&e))?;
    let guidance = create("p")?;
    guidance
        .append_child(&nodes[1])
        .map_err(|e| host_error(&e))?;
    let separator = create("span")?;
    separator.set_text_content(Some(" "));
    guidance
        .append_child(&separator)
        .map_err(|e| host_error(&e))?;
    guidance
        .append_child(&nodes[2])
        .map_err(|e| host_error(&e))?;
    body.append_child(&guidance).map_err(|e| host_error(&e))?;
    root.append_child(&body).map_err(|e| host_error(&e))?;
    Ok((root, nodes))
}
/// Mount one retained literal body in the actual canvas document.
pub(super) fn mount(
    document: &web_sys::Document,
    text: &Rc<RefCell<Text>>,
) -> Result<ReadingRoot, AttachmentHostError> {
    let (root, nodes) = body(document)?;
    let style = document
        .create_element("style")
        .map_err(|e| host_error(&e))?;
    style.set_text_content(Some(include_str!("../generic_artifact.css")));
    root.insert_before(&style, root.first_child().as_ref())
        .map_err(|e| host_error(&e))?;
    let mut text = text.borrow_mut();
    text.nodes = Some(nodes);
    sync_text(text.nodes.as_ref(), &text.values);
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        commit: None,
        owner: None,
        element: root,
        slots: vec![],
    })
}

/// Create the literal document mark without changing the mounted body.
fn icon(document: &web_sys::Document) -> Result<web_sys::Element, AttachmentHostError> {
    let icon = document
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
        .map_err(|e| host_error(&e))?;
    for (name, value) in [
        ("viewBox", "0 0 24 24"),
        ("fill", "none"),
        ("stroke", "currentColor"),
        ("aria-hidden", "true"),
    ] {
        icon.set_attribute(name, value)
            .map_err(|e| host_error(&e))?;
    }
    let path = document
        .create_element_ns(Some("http://www.w3.org/2000/svg"), "path")
        .map_err(|e| host_error(&e))?;
    for (name, value) in [
        ("stroke-linecap", "round"),
        ("stroke-linejoin", "round"),
        ("stroke-width", "1.5"),
        (
            "d",
            "M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z",
        ),
    ] {
        path.set_attribute(name, value)
            .map_err(|e| host_error(&e))?;
    }
    icon.append_child(&path).map_err(|e| host_error(&e))?;
    Ok(icon)
}

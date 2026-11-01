//! Stable native opaque PDF page images.
use super::Pages;
use crate::browser::attachment_content::{AttachmentHostError, ReadingRoot, browser::host_error};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{cell::RefCell, rc::Rc};
/// Append only newly completed pages, without remounting existing images.
pub(super) fn sync(pages: &Pages) -> Result<(), AttachmentHostError> {
    let Some(root) = &pages.root else {
        return Ok(());
    };
    let document = root
        .owner_document()
        .ok_or_else(|| AttachmentHostError::Host("Missing page document".into()))?;
    let count = root.child_element_count().saturating_sub(1);
    for (index, page) in pages.values.iter().enumerate().skip(count as usize) {
        let container = document.create_element("div").map_err(|e| host_error(&e))?;
        let image = document.create_element("img").map_err(|e| host_error(&e))?;
        for (key, value) in [
            (
                "src",
                format!("data:image/png;base64,{}", STANDARD.encode(&page.png)),
            ),
            ("width", page.width.to_string()),
            ("height", page.height.to_string()),
            ("alt", String::new()),
        ] {
            image
                .set_attribute(key, &value)
                .map_err(|e| host_error(&e))?;
        }
        container.append_child(&image).map_err(|e| host_error(&e))?;
        if index + 1 < pages.total {
            let separator = document.create_element("hr").map_err(|e| host_error(&e))?;
            container
                .append_child(&separator)
                .map_err(|e| host_error(&e))?;
        }
        root.append_child(&container).map_err(|e| host_error(&e))?;
    }
    Ok(())
}
/// Mount the feature root in the supplied canvas document.
pub(super) fn mount(
    document: &web_sys::Document,
    pages: &Rc<RefCell<Pages>>,
    padded: bool,
) -> Result<ReadingRoot, AttachmentHostError> {
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        commit: None,
        owner: None,
        element: root(document, pages, padded)?,
        slots: vec![],
    })
}
/// Build the shared page root independently of its placement owner.
pub(super) fn root(
    document: &web_sys::Document,
    pages: &Rc<RefCell<Pages>>,
    padded: bool,
) -> Result<web_sys::Element, AttachmentHostError> {
    let root = document
        .create_element("section")
        .map_err(|e| host_error(&e))?;
    root.set_attribute(
        "class",
        if padded {
            "maestro-pdf maestro-pdf-artifact"
        } else {
            "maestro-pdf"
        },
    )
    .map_err(|e| host_error(&e))?;
    let style = document
        .create_element("style")
        .map_err(|e| host_error(&e))?;
    style.set_text_content(Some(include_str!("../pdf.css")));
    root.append_child(&style).map_err(|e| host_error(&e))?;
    pages.borrow_mut().root = Some(root.clone());
    sync(&pages.borrow())?;
    Ok(root)
}

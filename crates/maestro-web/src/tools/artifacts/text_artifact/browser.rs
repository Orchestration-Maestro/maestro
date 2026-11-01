//! Stable literal source nodes sharing the syntax palette.
use super::Body;
use crate::browser::attachment_content::{
    AttachmentHostError, ReadingRoot,
    browser::{append, element, set_text_if_changed},
};
use std::{cell::RefCell, rc::Rc};
/// Synchronize only after an authored edit, retaining the root and viewport.
pub(super) fn sync(body: &Body, html_changed: bool) {
    if let Some(code) = &body.code {
        code.set_class_name(
            &body
                .language
                .map_or_else(String::new, |label| format!("hljs language-{label}")),
        );
        if let Some(html) = &body.html {
            if html_changed {
                code.set_inner_html(html);
            }
        } else if html_changed {
            code.set_text_content(Some(&body.text));
        } else {
            set_text_if_changed(code, &body.text);
        }
    }
}
/// Create a literal listing's `pre` and `code` nodes, styled with the shared syntax palette.
pub(crate) fn source_nodes(
    document: &web_sys::Document,
) -> Result<(web_sys::Element, web_sys::Element), AttachmentHostError> {
    let root = element(document, "pre", "maestro-text-artifact")?;
    let style = element(document, "style", "")?;
    let syntax = maestro_theme::highlight_css()
        .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
    style.set_text_content(Some(&format!(
        "{}{syntax}",
        include_str!("../text_artifact.css")
    )));
    append(&root, &style)?;
    let code = element(document, "code", "")?;
    append(&root, &code)?;
    Ok((root, code))
}
/// Mount a literal listing in the canvas document.
pub(super) fn mount(
    document: &web_sys::Document,
    body: &Rc<RefCell<Body>>,
) -> Result<ReadingRoot, AttachmentHostError> {
    let (root, code) = source_nodes(document)?;
    if body.borrow().wrap {
        root.set_class_name("maestro-text-artifact maestro-markdown-source");
    }
    body.borrow_mut().code = Some(code);
    sync(&body.borrow(), true);
    Ok(ReadingRoot {
        frame_style: "",
        decoration: None,
        element: root,
        slots: vec![],
        owner: None,
        commit: None,
    })
}

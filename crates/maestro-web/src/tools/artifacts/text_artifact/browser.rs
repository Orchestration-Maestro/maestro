//! Stable literal source nodes sharing the syntax palette.
use super::Body;
use crate::browser::attachment_content::{
    AttachmentUiError, ReadingRoot,
    browser::{append, element},
};
use std::{cell::RefCell, rc::Rc};
/// Synchronize only after an authored edit, retaining the root and viewport.
pub(super) fn sync(body: &Body) {
    if let Some(code) = &body.code {
        code.set_class_name(
            &body
                .language
                .map_or_else(String::new, |label| format!("hljs language-{label}")),
        );
        if let Some(html) = &body.html {
            code.set_inner_html(html);
        } else {
            code.set_text_content(Some(&body.text));
        }
    }
}
/// Mount a literal listing in the canvas document.
pub(super) fn mount(
    document: &web_sys::Document,
    body: &Rc<RefCell<Body>>,
) -> Result<ReadingRoot, AttachmentUiError> {
    let root = element(document, "pre", "maestro-text-artifact")?;
    let style = element(document, "style", "")?;
    let syntax = maestro_theme::highlight_css()
        .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?;
    style.set_text_content(Some(&format!(
        "{}{syntax}",
        include_str!("../text_artifact.css")
    )));
    append(&root, &style)?;
    let code = element(document, "code", "")?;
    append(&root, &code)?;
    body.borrow_mut().code = Some(code);
    sync(&body.borrow());
    Ok(ReadingRoot {
        element: root,
        slots: vec![],
        owner: None,
        commit: None,
    })
}

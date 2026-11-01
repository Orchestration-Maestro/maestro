//! Host-free source listings retaining their native nodes across equal updates.
use super::{Body, TextArtifact};
use crate::{
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::cell::RefCell;

/// Original operands and listing nodes of one placement.
struct Mounted {
    /// Last filename and original source published.
    key: (String, String),
    /// Listing state shared with the retained source builder.
    body: Body,
}
impl TextArtifact {
    /// Build independent copy and original UTF-8 download controls.
    /// # Errors
    /// Returns native or localization failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::source(
            view,
            &self.filename,
            self.mime(),
            &self.content,
            self.titles(),
        )
    }
}
impl DomContent for TextArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let (node, code) = super::source_nodes(&view.document)?;
        if matches!(self.profile, super::SourceProfile::Markdown) {
            node.set_class_name("maestro-text-artifact maestro-markdown-source");
        }
        let frame = view
            .document
            .create_element("section")
            .map_err(|error| ToolUiError::native(&error))?;
        frame.set_class_name("maestro-text-artifact-body");
        frame
            .append_child(&node)
            .map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: frame.into(),
            owner: Some(Box::new(RefCell::new(Mounted {
                key: (String::new(), String::new()),
                body: Body {
                    code: Some(code),
                    ..Body::default()
                },
            }))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let state = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Source body is not mounted".into()))?;
        if state.borrow().key.0 == self.filename && state.borrow().key.1 == self.content {
            return Ok(());
        }
        let language = self.language();
        let html = self.highlighted(&*view.highlighter, language);
        let mut mounted = state.borrow_mut();
        let body = &mut mounted.body;
        body.language = language;
        let changed = body.html != html;
        body.html = html;
        body.text.clone_from(&self.content);
        super::browser::sync(body, changed);
        mounted.key = (self.filename.clone(), self.content.clone());
        Ok(())
    }
}

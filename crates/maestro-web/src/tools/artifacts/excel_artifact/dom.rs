//! Native workbook projection retaining its selected sheet and scrolling until replacement.
use super::ExcelArtifact;
use crate::{
    browser::attachment_content::browser::{append, element, sync_text},
    dialogs::attachment_overlay::{error, excel::Excel},
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast as _;

/// Published workbook or diagnostic and the assignment that produced it.
#[derive(Default)]
struct Mounted {
    /// Original source last processed.
    source: String,
    /// Explicit assignment identity.
    assignment: Option<Rc<()>>,
    /// Workbook root retaining native tab subscriptions.
    body: Option<DomRoot>,
    /// Localized heading of a failed body.
    error: Option<web_sys::Element>,
}
impl ExcelArtifact {
    /// Build a header saving the complete original workbook bytes.
    /// # Errors
    /// Returns decoding, localization or native failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::download(
            view,
            &self.filename,
            self.mime(),
            &crate::utils::base64::original(&self.content)
                .map_err(|e| ToolUiError::Presentation(e.to_string()))?,
        )
    }
    /// Decode and mount through the same workbook owner used by attachment previews.
    fn workbook(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let bytes = crate::utils::base64::original(&self.content)
            .map_err(|e| ToolUiError::Presentation(e.to_string()))?;
        let mut workbook = Excel::default();
        workbook.open(&bytes).map_err(ToolUiError::Presentation)?;
        workbook.mount_root(view, true)
    }
}
impl DomContent for ExcelArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot {
            node: element(
                &view.document,
                "section",
                "maestro-excel-artifact maestro-excel-artifact-body",
            )?
            .into(),
            owner: Some(Box::new(RefCell::new(Mounted::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Workbook body is not mounted".into()))?;
        let mut state = mounted.borrow_mut();
        let requested = state
            .assignment
            .as_ref()
            .is_none_or(|old| !Rc::ptr_eq(old, &self.assignment));
        if state.source == self.content && !(state.error.is_some() && requested) {
            if let Some(heading) = &state.error {
                heading.set_text_content(Some(&view.i18n.text("Error loading spreadsheet")?));
            }
            return Ok(());
        }
        state.source.clone_from(&self.content);
        state.assignment = Some(Rc::clone(&self.assignment));
        if self.content.is_empty() && state.error.is_none() {
            return Ok(());
        }
        publish(self, root.node.unchecked_ref(), view, &mut state)
    }
}
/// Replace only a changed workbook, preserving successful equal-source native roots.
fn publish(
    artifact: &ExcelArtifact,
    node: &web_sys::Element,
    view: &DomView,
    state: &mut Mounted,
) -> Result<(), ToolUiError> {
    state.body = None;
    state.error = None;
    node.replace_children_with_node_0();
    if artifact.content.is_empty() {
        return Ok(());
    }
    match artifact.workbook(view) {
        Ok(body) => {
            node.append_child(&body.node)
                .map_err(|e| ToolUiError::native(&e))?;
            state.body = Some(body);
        }
        Err(failure) => {
            let (panel, nodes) = error::nodes(&view.document, true)?;
            sync_text(
                Some(&nodes),
                &[
                    view.i18n.text("Error loading spreadsheet")?,
                    error::detail(
                        &failure.to_string(),
                        &*view.i18n,
                        "Failed to load spreadsheet",
                    )?,
                ],
            );
            append(node, &panel)?;
            state.error = Some(nodes[0].clone());
        }
    }
    Ok(())
}

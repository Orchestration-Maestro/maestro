//! Preview and source regions controlled by the existing shared mode control.
use crate::{
    browser::{
        attachment_content::browser::{append, element},
        event_subscription::EventSubscription,
    },
    components::preview_code_toggle::{PreviewCodeToggle, ViewMode},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::closure::Closure;
use web_sys::Element;

/// Non-owning link to the most recently published body.
pub(crate) type LastBody = Rc<RefCell<Weak<Regions>>>;
/// Two retained placements, neither replaced by selecting the other.
pub(crate) struct Regions {
    /// Scrollable containing root.
    pub(crate) node: Element,
    /// Selection of the controller that last updated this body.
    selection: RefCell<PreviewCodeToggle>,
    /// Live independent mode controls, retained by their header roots rather than the body.
    headers: RefCell<Vec<Weak<DomRoot>>>,
    /// Preview placement.
    preview: DomRoot,
    /// Source placement.
    source: DomRoot,
    /// Preview visibility region.
    preview_region: Element,
    /// Source visibility region.
    source_region: Element,
}
impl Regions {
    /// Mount each region once using its existing content owner.
    pub(crate) fn mount(
        view: &DomView,
        preview: &dyn DomContent,
        source: &dyn DomContent,
        toggle: &PreviewCodeToggle,
    ) -> Result<Rc<Self>, ToolUiError> {
        let node = element(&view.document, "section", "maestro-artifact-views")?;
        let preview_region = element(&view.document, "div", "maestro-artifact-preview")?;
        let source_region = element(&view.document, "div", "maestro-artifact-source")?;
        let preview = preview.mount(view)?;
        let source = source.mount(view)?;
        preview_region
            .append_child(&preview.node)
            .map_err(|e| ToolUiError::native(&e))?;
        source_region
            .append_child(&source.node)
            .map_err(|e| ToolUiError::native(&e))?;
        append(&node, &preview_region)?;
        append(&node, &source_region)?;
        Ok(Rc::new(Self {
            node,
            selection: RefCell::new(toggle.shared()),
            headers: RefCell::default(),
            preview,
            source,
            preview_region,
            source_region,
        }))
    }
    /// Refresh the independent content owners without remounting them.
    pub(crate) fn update(
        &self,
        view: &DomView,
        preview: &dyn DomContent,
        source: &dyn DomContent,
    ) -> Result<(), ToolUiError> {
        preview.update(&self.preview, view)?;
        source.update(&self.source, view)
    }
    /// Retarget subsequent header actions to the controller that now owns this body.
    pub(crate) fn bind(
        &self,
        toggle: &PreviewCodeToggle,
        view: &DomView,
    ) -> Result<(), ToolUiError> {
        let previous = self.selection.replace(toggle.shared());
        drop(previous);
        self.headers
            .borrow_mut()
            .retain(|header| header.strong_count() > 0);
        let headers: Vec<_> = self
            .headers
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for header in headers {
            toggle.update(&header, view)?;
        }
        Ok(())
    }
    /// Show the chosen region without disturbing native selection or scrolling.
    pub(crate) fn show(&self, mode: ViewMode) -> Result<(), ToolUiError> {
        self.selection.borrow().set_mode(mode);
        for (node, hidden) in [
            (&self.preview_region, mode == ViewMode::Code),
            (&self.source_region, mode == ViewMode::Preview),
        ] {
            node.toggle_attribute_with_force("hidden", hidden)
                .map_err(|e| ToolUiError::native(&e))?;
        }
        Ok(())
    }
}
/// Prepend a shared preview/source control to an existing source header.
pub(crate) fn header(
    view: &DomView,
    mut header: DomRoot,
    toggle: &PreviewCodeToggle,
    body: &LastBody,
) -> Result<DomRoot, ToolUiError> {
    use wasm_bindgen::JsCast as _;
    header
        .node
        .unchecked_ref::<web_sys::Element>()
        .class_list()
        .add_1("maestro-artifact-preview-header")
        .map_err(|error| ToolUiError::native(&error))?;
    let control = Rc::new(toggle.mount(view)?);
    if let Some(mounted) = body.borrow().upgrade() {
        mounted.headers.borrow_mut().push(Rc::downgrade(&control));
    }
    let body = Rc::clone(body);
    let changed =
        Closure::<dyn Fn(web_sys::CustomEvent)>::new(move |event: web_sys::CustomEvent| {
            let mode = match event.detail().as_string().as_deref() {
                Some("preview") => ViewMode::Preview,
                Some("code") => ViewMode::Code,
                _ => return,
            };
            let mounted = body.borrow().upgrade();
            if let Some(mounted) = mounted {
                // A native event has no error channel; a failed change keeps the previous region.
                let _ = mounted.show(mode);
            }
        });
    let event = EventSubscription::new(control.node.clone().into(), "mode-change", changed)
        .map_err(|e| ToolUiError::native(&e))?;
    header
        .node
        .insert_before(&control.node, header.node.first_child().as_ref())
        .map_err(|e| ToolUiError::native(&e))?;
    header.owner = Some(Box::new((header.owner.take(), control, event)));
    Ok(header)
}

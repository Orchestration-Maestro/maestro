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

/// Non-owning body and header registrations retained across native placements.
pub(crate) type LastBody = Rc<RefCell<BodyLink>>;
/// A controller's latest body and its independently mounted headers.
#[derive(Default)]
pub(crate) struct BodyLink {
    /// Most recently mounted or updated regions.
    body: Weak<Regions>,
    /// Independent controls, which may be mounted before the body.
    headers: Vec<Weak<DomRoot>>,
}
impl BodyLink {
    /// Read the currently published regions without retaining a native placement.
    fn upgrade(&self) -> Option<Rc<Regions>> {
        self.body.upgrade()
    }
    /// Read live headers only while this link still targets the supplied body.
    fn live_headers(&mut self, body: &Regions) -> Vec<Rc<DomRoot>> {
        if !std::ptr::eq(self.body.as_ptr(), body) {
            return Vec::new();
        }
        self.headers.retain(|header| header.strong_count() > 0);
        self.headers.iter().filter_map(Weak::upgrade).collect()
    }
}
/// Publish a placement without losing headers constructed before it or an earlier placement.
pub(crate) fn publish(last: &LastBody, body: &Rc<Regions>) {
    last.borrow_mut().body = Rc::downgrade(body);
    let link = Rc::downgrade(last);
    let mut links = body.links.borrow_mut();
    links.retain(|link| link.strong_count() > 0);
    if !links.iter().any(|existing| existing.ptr_eq(&link)) {
        links.push(link);
    }
}
/// Two retained placements, neither replaced by selecting the other.
pub(crate) struct Regions {
    /// Scrollable containing root.
    pub(crate) node: Element,
    /// Selection of the controller that last updated this body.
    selection: RefCell<PreviewCodeToggle>,
    /// Weak controller links published for this body.
    links: RefCell<Vec<Weak<RefCell<BodyLink>>>>,
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
            links: RefCell::default(),
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
        let links: Vec<_> = self
            .links
            .borrow()
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for link in links {
            let headers = link.borrow_mut().live_headers(self);
            for header in headers {
                toggle.update(&header, view)?;
            }
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
    body.borrow_mut().headers.push(Rc::downgrade(&control));
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

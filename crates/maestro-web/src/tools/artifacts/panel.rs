//! Native artifact panel backed by the existing application command and replay owner.
use super::{ArtifactsPanel, Callback, native::NativeArtifact};
use crate::{
    dom::{self, DomContent, DomRoot, DomView, Link},
    tools::types::ToolUiError,
};
use maestro_app::{SandboxUrlProvider, browser::tools::artifacts::LocalArtifactsPanel};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

mod adapter;
mod element;
mod render;
pub(crate) use element::define;
thread_local! {
    /// Page-local placement sequence; no heap address is exposed as an element key.
    static NEXT_PANEL: Cell<u64> = const { Cell::new(0) };
}
/// Allocate an instance key for panels whose application callers cannot update each other.
pub(super) fn new_key() -> String {
    NEXT_PANEL.with(|next| {
        let key = next.get();
        next.set(key.wrapping_add(1));
        format!("artifact-panel-{key}")
    })
}
/// Caller options retained independently of native connection.
#[derive(Default)]
pub(super) struct Options {
    /// Host notification when a pill opens an existing artifact.
    open: RefCell<Option<Rc<dyn Fn()>>>,
    /// Host notification after application-owned changes.
    changed: RefCell<Option<Rc<dyn Fn()>>>,
    /// Address forwarded to HTML artifact sandboxes.
    url: RefCell<Option<SandboxUrlProvider>>,
}
/// One literal file's controller and its retained source element.
struct Entry {
    /// Actual format owner.
    controller: Rc<RefCell<NativeArtifact>>,
    /// Retained native source element once mounted.
    root: Option<Rc<DomRoot>>,
    /// Content revision used to retain an unchanged independent header.
    revision: Rc<()>,
    /// Last application-to-view failure for this file only.
    failure: Option<String>,
}
/// Native nodes and application adapter shared by the handle and source element.
pub(super) struct State {
    /// Owned page operands refreshed by typed updates.
    view: RefCell<DomView>,
    /// The single application command/replay interface.
    caller: Rc<LocalArtifactsPanel>,
    /// Literal filename order follows the application owner.
    entries: RefCell<indexmap::IndexMap<String, Rc<RefCell<Entry>>>>,
    /// Weak native placement.
    root: RefCell<Option<Link>>,
    /// Collapsed selection shared with the panel handle.
    collapsed: Rc<Cell<bool>>,
    /// Close notification shared with the handle.
    close: Callback,
    /// Replaceable host callbacks and sandbox address.
    options: Rc<Options>,
    /// Connected chrome subscriptions.
    events: RefCell<Vec<render::Click>>,
    /// Names of the currently built tab strip.
    tabs: RefCell<Vec<String>>,
    /// Current independent artifact header and its revision.
    header: RefCell<Option<render::Header>>,
    /// Requested active tab reveal after painting.
    scroll: RefCell<Option<String>>,
}
impl State {
    /// Build the command adapter with a weak backedge to presentation state.
    fn new(
        view: &DomView,
        collapsed: Rc<Cell<bool>>,
        close: Callback,
        options: Rc<Options>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak| Self {
            view: RefCell::new(view.clone()),
            caller: Rc::new(LocalArtifactsPanel::new_local(Rc::new(
                adapter::Presentation(Weak::clone(weak)),
            ))),
            entries: RefCell::default(),
            root: RefCell::default(),
            collapsed,
            close,
            options,
            events: RefCell::default(),
            tabs: RefCell::default(),
            header: RefCell::default(),
            scroll: RefCell::default(),
        })
    }
    /// Snapshot retained handles before running native or caller effects.
    fn entries(&self) -> Vec<(String, Rc<RefCell<Entry>>)> {
        self.entries
            .borrow()
            .iter()
            .map(|(name, entry)| (name.clone(), Rc::clone(entry)))
            .collect()
    }
    /// Whether a callback still belongs to the current native placement.
    fn owns(&self, root: &Link) -> bool {
        self.root
            .borrow()
            .as_ref()
            .and_then(dom::live)
            .zip(dom::live(root))
            .is_some_and(|(current, root)| current.is_same_node(Some(root.as_ref())))
    }
    /// Paint an application notification, retaining any reached error as literal visible text.
    pub(super) fn refresh(self: &Rc<Self>) {
        let failure = self
            .paint()
            .err()
            .map(|error| error.to_string())
            .or_else(|| {
                self.caller
                    .active_filename()
                    .and_then(|name| self.entries.borrow().get(&name).cloned())
                    .and_then(|entry| entry.borrow().failure.clone())
            });
        if let Some(root) = self.root.borrow().as_ref().and_then(dom::live)
            && let Ok(Some(status)) = root.query_selector(":scope > .maestro-artifact-panel-error")
        {
            status.set_text_content(failure.as_deref());
        }
    }
    /// Release connection-scoped chrome while retaining each file's detached view state.
    fn disconnect(&self) {
        let events = self.events.take();
        let header = self.header.take();
        self.tabs.borrow_mut().clear();
        for (_, entry) in self.entries() {
            let root = entry.borrow().root.clone();
            if let Some(root) = root {
                use wasm_bindgen::JsCast as _;
                root.node.unchecked_ref::<web_sys::Element>().remove();
            }
        }
        if let Some(root) = self.root.borrow().as_ref().and_then(dom::live) {
            root.replace_children_with_node_0();
        }
        drop((events, header));
    }
}
impl ArtifactsPanel {
    /// Find the installed native format owner for a literal filename.
    /// Independent headers share this owner's live body; application commands retain it across edits.
    /// Release controller borrows before executing commands or calling other panel operations.
    #[must_use]
    pub fn get_native_artifact_element(
        &self,
        filename: &str,
    ) -> Option<Rc<RefCell<NativeArtifact>>> {
        let state = self.native.borrow().clone()?;
        let entry = state.entries.borrow().get(filename).cloned()?;
        Some(Rc::clone(&entry.borrow().controller))
    }
    /// Obtain the existing application command interface for this native panel.
    #[must_use]
    pub fn application(&self, view: &DomView) -> Rc<LocalArtifactsPanel> {
        Rc::clone(&self.page_state(view).caller)
    }
    /// Retain caller operations without copying their artifact policy into presentation.
    fn page_state(&self, view: &DomView) -> Rc<State> {
        if let Some(state) = self.native.borrow().as_ref() {
            return Rc::clone(state);
        }
        let state = State::new(
            view,
            Rc::clone(&self.collapsed),
            Rc::clone(&self.on_close),
            Rc::clone(&self.native_options),
        );
        *self.native.borrow_mut() = Some(Rc::clone(&state));
        state
    }
    /// Replace the native panel's open notification.
    pub fn set_on_open(&self, callback: Option<Rc<dyn Fn()>>) {
        let previous = self.native_options.open.replace(callback);
        drop(previous);
    }
    /// Replace the native panel's application-change notification.
    pub fn set_on_artifacts_change(&self, callback: Option<Rc<dyn Fn()>>) {
        let previous = self.native_options.changed.replace(callback);
        drop(previous);
    }
    /// Replace the address supplied to HTML artifact frames on their next load.
    pub fn set_sandbox_url_provider(&self, provider: Option<SandboxUrlProvider>) {
        let previous = self.native_options.url.replace(provider);
        drop(previous);
    }
}
/// Root ownership disconnects native effects even when the element stays in the page.
struct Mounted(Rc<State>, Link);
impl Drop for Mounted {
    fn drop(&mut self) {
        if self.0.owns(&self.1) {
            self.0.disconnect();
            self.0.root.take();
        }
    }
}
impl DomContent for ArtifactsPanel {
    fn key(&self) -> Option<&str> {
        Some(&self.native_key)
    }
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let state = self.page_state(view);
        let node = dom::create_with(&view.document, "artifacts-panel", Rc::downgrade(&state))
            .map_err(|error| ToolUiError::native(&error))?;
        *state.root.borrow_mut() = Some(Link::new({
            use wasm_bindgen::JsCast as _;
            node.unchecked_ref()
        }));
        node.set_class_name("maestro-artifacts-panel");
        node.toggle_attribute_with_force("collapsed", self.collapsed.get())
            .map_err(|error| ToolUiError::native(&error))?;
        let owner = {
            use wasm_bindgen::JsCast as _;
            Mounted(state, Link::new(node.unchecked_ref()))
        };
        Ok(DomRoot {
            node: node.into(),
            owner: Some(Box::new(owner)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("Artifact panel is not mounted".into()))?;
        if self
            .native
            .borrow()
            .as_ref()
            .is_none_or(|state| !Rc::ptr_eq(state, &mounted.0))
        {
            return Err(ToolUiError::Presentation(
                "Artifact panel belongs to another controller".into(),
            ));
        }
        *mounted.0.view.borrow_mut() = view.clone();
        mounted.0.paint()
    }
}

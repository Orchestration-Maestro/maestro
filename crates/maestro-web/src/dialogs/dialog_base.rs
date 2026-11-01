//! An opened dialog that keeps itself mounted until closed and restores the focus it took over.
use super::dialog::{DialogProps, dialog};
use crate::{
    browser::attachment_content::browser::host_error,
    dom::{DomRoot, DomView},
    tools::types::ToolUiError,
};
use std::{any::Any, cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, Node};

/// The standard panel width of an opened dialog.
pub const MODAL_WIDTH: &str = "min(600px, 90vw)";
/// The standard panel height of an opened dialog.
pub const MODAL_HEIGHT: &str = "min(600px, 80vh)";
/// A dialog mounted on the page body.
///
/// Opening keeps the dialog alive until it closes, whether or not this handle is kept.
/// Clones share one dialog.
#[derive(Clone)]
pub struct DialogBase {
    /// Shared dialog state.
    state: Rc<State>,
}
/// Operands and the open dialog's resources.
struct State {
    /// Page operands.
    view: Rc<DomView>,
    /// Literal CSS panel width.
    width: String,
    /// Literal CSS panel height.
    height: String,
    /// The open dialog, absent while closed.
    opened: RefCell<Option<Opened>>,
    /// Notified after each close, once the dialog is removed and focus restored.
    on_close: RefCell<Option<Rc<dyn Fn()>>>,
}
/// Resources of an open dialog.
struct Opened {
    /// Mounted backdrop and its close routes, whose callbacks keep the state alive.
    root: DomRoot,
    /// The element focused before opening, including a focusable non-HTML element.
    previous_focus: Option<Element>,
    /// State of the component showing this dialog, kept alive while it is open.
    _component: Option<Rc<dyn Any>>,
}
impl DialogBase {
    /// A closed dialog with the given panel dimensions.
    #[must_use]
    pub fn new(view: Rc<DomView>, width: &str, height: &str) -> Self {
        Self {
            state: Rc::new(State {
                view,
                width: width.into(),
                height: height.into(),
                opened: RefCell::default(),
                on_close: RefCell::default(),
            }),
        }
    }
    /// Replace the callback notified after each close.
    pub fn set_on_close(&self, callback: Option<Rc<dyn Fn()>>) {
        let previous = self.state.on_close.replace(callback);
        drop(previous);
    }
    /// Remember the focused element and mount the dialog with `content` on the page body.
    ///
    /// `component` is the state of the component showing this dialog; it stays alive while the
    /// dialog is open, so its callbacks may hold it weakly. Opening an open dialog changes nothing.
    /// # Errors
    /// Returns a localization or native failure; the dialog then stays closed.
    pub fn open(
        &self,
        content: Vec<Node>,
        component: Option<Rc<dyn Any>>,
    ) -> Result<(), ToolUiError> {
        if self.is_open() {
            return Ok(());
        }
        let state = &self.state;
        let document = &state.view.document;
        let previous_focus = document.active_element();
        let owner = Rc::clone(state);
        let props = DialogProps {
            on_close: Some(Rc::new(move || close(&owner))),
            width: state.width.clone(),
            height: state.height.clone(),
            backdrop_class_name: "maestro-dialog-blur".into(),
            ..DialogProps::new(content)
        };
        let root = dialog(&state.view, &props)?
            .ok_or_else(|| ToolUiError::Presentation("Open dialog mounted nothing".into()))?;
        document
            .body()
            .ok_or_else(|| ToolUiError::Presentation("Document has no body".into()))?
            .append_child(&root.node)
            .map_err(|error| host_error(&error))?;
        state.opened.replace(Some(Opened {
            root,
            previous_focus,
            _component: component,
        }));
        Ok(())
    }
    /// Remove the dialog, restore the focus it took over, then notify the close callback.
    ///
    /// Closing a closed dialog changes nothing.
    pub fn close(&self) {
        close(&self.state);
    }
    /// Whether the dialog is mounted.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.opened.borrow().is_some()
    }
    /// The open dialog's panel.
    #[must_use]
    pub fn panel(&self) -> Option<Element> {
        self.state
            .opened
            .borrow()
            .as_ref()
            .and_then(|opened| opened.root.node.first_child())
            .and_then(|panel| panel.dyn_into().ok())
    }
}
/// Close `state`'s dialog once, releasing the routes that keep it alive.
fn close(state: &Rc<State>) {
    let Some(opened) = state.opened.take() else {
        return;
    };
    if let Some(parent) = opened.root.node.parent_node() {
        let _ = parent.remove_child(&opened.root.node);
    }
    if let Some(previous) = opened
        .previous_focus
        .as_ref()
        .filter(|focused| focused.is_connected())
    {
        let _ = crate::browser::invoke0(previous, "focus");
    }
    let callback = state.on_close.borrow().clone();
    if let Some(callback) = callback {
        callback();
    }
    // The component may hold this dialog only weakly; release it after its notification.
    drop(opened);
}

//! Native connection callbacks for a retained session view.
use crate::{browser::attachment_content::browser::host_error, dom, tools::types::ToolUiError};
use std::{cell::RefCell, rc::Rc};
use web_sys::{Document, Element};

/// Connection notification installed after the session view has been built.
pub(super) type Connection = Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>;

/// Native element holding only a weakly bound connection notification.
struct SessionElement {
    /// The creating view's notification, absent for an independently created element.
    connection: Option<Connection>,
}
impl SessionElement {
    /// Notify the retained view without holding the callback slot borrowed.
    fn changed(&self, connected: bool) {
        let notify = self
            .connection
            .as_ref()
            .and_then(|slot| slot.borrow().clone());
        if let Some(notify) = notify {
            notify(connected);
        }
    }
}
impl custom_element::CustomElement for SessionElement {
    fn connected_callback(&mut self) {
        self.changed(true);
    }
    fn disconnected_callback(&mut self) {
        self.changed(false);
    }
}

/// Create the session root with native connection callbacks.
pub(super) fn root(document: &Document, connection: &Connection) -> Result<Element, ToolUiError> {
    let window = document
        .default_view()
        .ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
    if window
        .custom_elements()
        .get("maestro-agent-interface")
        .is_undefined()
    {
        dom::define_element("maestro-agent-interface", Vec::new(), &[], |_, _| {
            SessionElement {
                connection: dom::take_creating::<Connection>(),
            }
        })
        .map_err(|error| host_error(&error))?;
    }
    let root = dom::create_with(document, "maestro-agent-interface", Rc::clone(connection))
        .map_err(|error| ToolUiError::native(&error))?;
    root.set_attribute("class", "maestro-agent")
        .map_err(|error| ToolUiError::native(&error))?;
    Ok(root)
}

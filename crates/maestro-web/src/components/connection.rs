//! Native connection callbacks for retained component roots.
use crate::{browser::attachment_content::browser::host_error, dom, tools::types::ToolUiError};
use std::{cell::RefCell, rc::Rc};
use web_sys::{Document, Element};

/// Connection notification installed after the component has been built.
pub(crate) type Connection = Rc<RefCell<Option<Rc<dyn Fn(bool)>>>>;

/// Native element holding only a weakly bound connection notification.
struct ConnectedElement {
    /// The creating component's notification, absent for an independently created element.
    connection: Option<Connection>,
}
impl ConnectedElement {
    /// Notify the retained component without holding the callback slot borrowed.
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
impl custom_element::CustomElement for ConnectedElement {
    fn connected_callback(&mut self) {
        self.changed(true);
    }
    fn disconnected_callback(&mut self) {
        self.changed(false);
    }
}

/// Create a `tag` root carrying `class` whose native connection changes reach `connection`.
pub(crate) fn root(
    document: &Document,
    tag: &str,
    class: &str,
    connection: &Connection,
) -> Result<Element, ToolUiError> {
    let window = document
        .default_view()
        .ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
    if window.custom_elements().get(tag).is_undefined() {
        dom::define_element(tag, Vec::new(), &[], |_, _| ConnectedElement {
            connection: dom::take_creating::<Connection>(),
        })
        .map_err(|error| host_error(&error))?;
    }
    let root = dom::create_with(document, tag, Rc::clone(connection))
        .map_err(|error| host_error(&error))?;
    root.set_attribute("class", class)
        .map_err(|error| host_error(&error))?;
    Ok(root)
}

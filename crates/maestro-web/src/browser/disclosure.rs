//! Mounted disclosure listeners shared by session headers and tool output.
use super::{
    attachment_content::{AttachmentHostError, browser::host_error},
    event_subscription::EventSubscription,
    invoke0,
};
use std::{any::Any, rc::Rc};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::Element;

/// One native class slot; unrelated classes stay owned by their existing writers.
pub(crate) struct Disclosure {
    /// Clickable native container.
    pub(crate) element: Element,
    /// Class toggled from its live native value.
    pub(crate) class: &'static str,
}
/// Fallible native click callable retained until listener removal.
type Click = Closure<dyn FnMut() -> Result<(), JsValue>>;
/// Root-owned state with weak callback captures.
struct Mounted {
    /// Strong counterpart to each weak callback.
    _state: Rc<Vec<Disclosure>>,
    /// Exact-target listeners detached before callable release.
    _listeners: Vec<EventSubscription<Click>>,
}
/// Read the owning window selection before toggling the live class slot.
fn toggle(disclosure: &Disclosure) -> Result<(), JsValue> {
    let window = disclosure
        .element
        .owner_document()
        .and_then(|d| d.default_view())
        .ok_or_else(|| JsValue::from_str("missing disclosure window"))?;
    let selection = invoke0(&window, "getSelection")?;
    if invoke0(&selection, "toString")?
        .as_string()
        .is_some_and(|s| !s.is_empty())
    {
        return Ok(());
    }
    disclosure.element.class_list().toggle(disclosure.class)?;
    Ok(())
}
/// Bind weak callbacks whose exact-target subscriptions belong to the mounted root.
pub(crate) fn bind(state: Vec<Disclosure>) -> Result<Option<Box<dyn Any>>, AttachmentHostError> {
    if state.is_empty() {
        return Ok(None);
    }
    let state = Rc::new(state);
    let mut listeners = Vec::new();
    for (ordinal, disclosure) in state.iter().enumerate() {
        let weak = Rc::downgrade(&state);
        let callback = Closure::new(move || {
            if let Some(state) = weak.upgrade() {
                toggle(&state[ordinal])?;
            }
            Ok(())
        });
        listeners.push(
            EventSubscription::new(disclosure.element.clone().into(), "click", callback)
                .map_err(|e| host_error(&e))?,
        );
    }
    Ok(Some(Box::new(Mounted {
        _state: state,
        _listeners: listeners,
    })))
}

//! The caller mounting every contribution attached to one slot.
use super::{ContributionFailure, adapters::FailureContent};
use crate::{
    browser::event_subscription::EventSubscription,
    dom::{DomView, MountedContent},
};
use js_sys::WeakRef;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, HtmlElement};

/// Element tag of a module-placeable slot.
pub(super) const TAG: &str = "maestro-slot";
/// Document event announcing changed registrations.
const CHANGED: &str = "maestro-contributions";
/// Mounted contributions of one named slot, in registration order.
pub struct SlotMount {
    /// Slot name.
    name: String,
    /// Container receiving each contribution's wrapper.
    parent: Element,
    /// One wrapper and mount per package and identifier.
    mounted: Vec<((String, String), Element, MountedContent)>,
}
impl SlotMount {
    /// Mount nothing yet into `parent`.
    #[must_use]
    pub fn new(name: &str, parent: Element) -> Self {
        Self {
            name: name.to_owned(),
            parent,
            mounted: Vec::new(),
        }
    }
    /// Show every attached contribution for `input`, replacing failed ones with visible failures.
    ///
    /// Wrappers and roots of contributions that stay attached are kept in place.
    /// # Errors
    /// Returns a native placement failure.
    pub fn show(
        &mut self,
        input: &Value,
        view: &DomView,
    ) -> Result<Vec<ContributionFailure>, JsValue> {
        let contract = super::slot_input(&self.name);
        let mut failures = Vec::new();
        let mut next = Vec::new();
        for (contribution, factory) in super::slot_contributions(&self.name) {
            let key = (
                contribution.package.clone(),
                contribution.descriptor.id.clone(),
            );
            let (wrapper, mut mounted) = match self.mounted.iter().position(|(k, ..)| *k == key) {
                Some(index) => {
                    let (_, wrapper, mounted) = self.mounted.remove(index);
                    (wrapper, mounted)
                }
                None => (
                    view.document.create_element("div")?,
                    MountedContent::default(),
                ),
            };
            let previous = mounted.root().cloned();
            let shown = match super::admit(&contribution.descriptor, contract.as_deref()) {
                Ok(()) => mounted.show(factory.content(input).as_ref(), &wrapper, view),
                Err(failure) => {
                    let shown = mounted.show(&FailureContent(failure.clone()), &wrapper, view);
                    failures.push(failure);
                    shown
                }
            };
            shown.map_err(|error| JsValue::from_str(&error.to_string()))?;
            // A replaced root may be an isolated frame that still holds its registration.
            if let Some(previous) = previous.filter(|previous| mounted.root() != Some(previous)) {
                super::adapters::release_frame(&previous);
            }
            if wrapper.parent_node().is_none() {
                self.parent.append_child(&wrapper)?;
            }
            next.push((key, wrapper, mounted));
        }
        release(std::mem::replace(&mut self.mounted, next));
        Ok(failures)
    }
    /// Remove every mounted contribution.
    pub fn clear(&mut self) {
        release(std::mem::take(&mut self.mounted));
    }
}
/// Remove wrappers, releasing any isolated frame they hold.
fn release(mounted: Vec<((String, String), Element, MountedContent)>) {
    for (_, wrapper, mounted) in mounted {
        if let Some(root) = mounted.root() {
            super::adapters::release_frame(root);
        }
        wrapper.remove();
    }
}
/// Ask connected slots to show the current registrations.
pub(super) fn announce() {
    if let Some(document) = web_sys::window().and_then(|window| window.document())
        && let Ok(event) = web_sys::Event::new(CHANGED)
    {
        // Dispatch reports only whether a listener cancelled the event.
        let _ = document.dispatch_event(&event);
    }
}
/// Mount state of one slot element.
#[derive(Default)]
struct SlotState {
    /// Mounted contributions while connected and not being shown.
    mount: Option<SlotMount>,
    /// Whether the element is connected.
    connected: bool,
    /// Whether a refresh is showing contributions now.
    running: bool,
    /// Whether registrations changed while a refresh was running.
    stale: bool,
}
/// One `maestro-slot` element.
struct SlotElement {
    /// Element owning this state.
    root: WeakRef,
    /// Page operands.
    view: Rc<DomView>,
    /// Mount state shared with the registration-change callback.
    state: Rc<RefCell<SlotState>>,
    /// Registration-change subscription while connected.
    changed: Option<EventSubscription<Closure<dyn Fn()>>>,
}
impl custom_element::CustomElement for SlotElement {
    fn connected_callback(&mut self) {
        let Some(root) = self
            .root
            .deref()
            .and_then(|r| r.dyn_into::<HtmlElement>().ok())
        else {
            return;
        };
        let name = root.get_attribute("name").unwrap_or_default();
        {
            let mut state = self.state.borrow_mut();
            state.connected = true;
            if state.mount.is_none() && !state.running {
                state.mount = Some(SlotMount::new(&name, root.into()));
            }
        }
        // Subscribe first: contributions mounted by this refresh may register more.
        let (weak, state, view) = (
            self.root.clone(),
            Rc::clone(&self.state),
            Rc::clone(&self.view),
        );
        let callback = Closure::new(move || refresh(&weak, &state, &view));
        self.changed =
            EventSubscription::new(self.view.document.clone().into(), CHANGED, callback).ok();
        refresh(&self.root, &self.state, &self.view);
    }
    fn disconnected_callback(&mut self) {
        self.changed = None;
        let mount = {
            let mut state = self.state.borrow_mut();
            state.connected = false;
            state.mount.take()
        };
        if let Some(mut mount) = mount {
            mount.clear();
        }
    }
}
/// Show the slot's contributions for its current input property.
///
/// Mounting runs contribution code, which may change registrations or move this
/// slot; no borrow is held across it, and a change during it shows again.
fn refresh(root: &WeakRef, state: &Rc<RefCell<SlotState>>, view: &DomView) {
    {
        let mut current = state.borrow_mut();
        if current.running {
            current.stale = true;
            return;
        }
        current.running = true;
    }
    loop {
        let taken = state.borrow_mut().mount.take();
        let Some(mut mount) = taken else { break };
        // Native placement failures leave the previous wrappers; lifecycle callbacks have no error channel.
        let _ = mount.show(&input(root), view);
        let mut current = state.borrow_mut();
        if !current.connected {
            drop(current);
            mount.clear();
            break;
        }
        current.mount = Some(mount);
        if !std::mem::take(&mut current.stale) {
            break;
        }
    }
    state.borrow_mut().running = false;
}
/// The slot's `maestroInput` property as plain data.
fn input(root: &WeakRef) -> Value {
    root.deref()
        .and_then(|root| js_sys::Reflect::get(&root, &"maestroInput".into()).ok())
        .and_then(|value| js_sys::JSON::stringify(&value).ok())
        .and_then(|text| text.as_string())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}
/// Define the slot element once for the page.
pub(super) fn define(view: &Rc<DomView>) -> Result<(), JsValue> {
    let view = Rc::clone(view);
    crate::dom::define_element(TAG, Vec::new(), move |root| SlotElement {
        root: WeakRef::new(&root),
        view: Rc::clone(&view),
        state: Rc::default(),
        changed: None,
    })
}

//! The caller mounting every contribution attached to one slot.
use super::{ContributionFailure, adapters::FailureContent};
use crate::{
    browser::event_subscription::EventSubscription,
    page::{DomView, MountedContent},
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
    mounted: Vec<Mounted>,
}
/// One contribution's identity, wrapper and mounted content.
type Mounted = ((String, String), Rc<()>, Element, MountedContent);
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
    /// Show every attached contribution for `input` and return the failures shown in place.
    ///
    /// Wrappers stay in place; roots are kept while their registration stays attached. A
    /// contribution that is not admitted, conflicts or fails to mount shows its failure
    /// in its own wrapper, and the rest of the slot still mounts.
    pub fn show(&mut self, input: &Value, view: &DomView) -> Vec<ContributionFailure> {
        let contract = super::slot_input(&self.name);
        let mut failures = Vec::new();
        let mut next = Vec::new();
        for entry in super::slot_entries(&self.name) {
            let contribution = entry.contribution;
            let key = (
                contribution.package.clone(),
                contribution.descriptor.id.clone(),
            );
            let (wrapper, mut mounted) = match self.take(&key, &entry.owner, view) {
                Ok(taken) => taken,
                Err(error) => {
                    failures.push(mount_failure(&key.1, format!("{error:?}")));
                    continue;
                }
            };
            let admitted = match &contract {
                Ok(contract) => super::admit(&contribution.descriptor, contract.as_deref()),
                Err(conflict) => Err(conflict.clone()),
            };
            let shown = admitted.and_then(|()| {
                let content = entry.view.content(input);
                mounted
                    .show(content.as_ref(), &wrapper, view)
                    .map_err(|error| mount_failure(&key.1, error.to_string()))
            });
            if let Err(failure) = shown {
                // A failure that cannot be placed leaves its wrapper empty; it is still reported.
                let _ = mounted.show(&FailureContent(failure.clone()), &wrapper, view);
                failures.push(failure);
            }
            if wrapper.parent_node().is_none()
                && let Err(error) = self.parent.append_child(&wrapper)
            {
                failures.push(mount_failure(&key.1, format!("{error:?}")));
            }
            next.push((key, entry.owner, wrapper, mounted));
        }
        remove(std::mem::replace(&mut self.mounted, next));
        failures
    }
    /// Keep the mount only while its registration still owns it.
    fn take(
        &mut self,
        key: &(String, String),
        owner: &Rc<()>,
        view: &DomView,
    ) -> Result<(Element, MountedContent), JsValue> {
        if let Some(index) = self.mounted.iter().position(|(k, ..)| k == key) {
            let (_, previous, wrapper, mounted) = self.mounted.remove(index);
            if Rc::ptr_eq(&previous, owner) {
                return Ok((wrapper, mounted));
            }
            wrapper.set_text_content(None);
            // Release the old frame's registered source before mounting its replacement.
            drop(mounted);
            return Ok((wrapper, MountedContent::default()));
        }
        Ok((
            view.document.create_element("div")?,
            MountedContent::default(),
        ))
    }
    /// Remove every mounted contribution.
    pub fn clear(&mut self) {
        remove(std::mem::take(&mut self.mounted));
    }
}
/// The failure for contribution `id` whose content or wrapper could not be placed.
fn mount_failure(id: &str, cause: String) -> ContributionFailure {
    ContributionFailure::Mount {
        id: id.to_owned(),
        cause,
    }
}
/// Remove wrappers; dropping their mounted content releases what each root owns.
fn remove(mounted: Vec<Mounted>) {
    for (_, _, wrapper, _) in mounted {
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
        // Failures are shown in place; lifecycle callbacks have no error channel.
        mount.show(&input(root), view);
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
    crate::page::define_element(TAG, Vec::new(), &[], move |root, _| SlotElement {
        root: WeakRef::new(&root),
        view: Rc::clone(&view),
        state: Rc::default(),
        changed: None,
    })
}

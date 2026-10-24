//! Native editing capabilities within the retained content owner.
use super::{AttachmentHostError, browser::host_error};
use crate::browser::event_subscription::EventSubscription;
use crate::components::fields::{InputProps, InputRef, InputSize, InputType};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlInputElement, KeyboardEvent};
/// Native input subscription with a typed browser event callback.
type InputSubscription = EventSubscription<Closure<dyn FnMut(Event)>>;
/// Adapt typed input callbacks to the shared browser subscription owner.
fn listen(
    element: &HtmlInputElement,
    name: &'static str,
    callback: Rc<dyn Fn(Event)>,
) -> Result<InputSubscription, AttachmentHostError> {
    let handler = Closure::wrap(Box::new(move |e| callback(e)) as Box<dyn FnMut(Event)>);
    EventSubscription::new(element.clone().into(), name, handler).map_err(|e| host_error(&e))
}
/// Per-element callbacks and the reference assignment this element owns.
pub(super) struct DomInput {
    /// Actual element whose registration is retained.
    element: HtmlInputElement,
    /// Current event registrations.
    listeners: Vec<InputSubscription>,
    /// Reference assignment owned by this field.
    slot: Option<InputRef>,
    /// Whether this native instance has received its first value.
    initialized: bool,
}
impl DomInput {
    /// Reuse the native child created by the content owner.
    pub(super) fn new(element: &Element) -> Result<Self, AttachmentHostError> {
        install_style(element)?;
        let element = element
            .clone()
            .dyn_into::<HtmlInputElement>()
            .map_err(|e| host_error(e.as_ref()))?;
        Ok(Self {
            element,
            listeners: Vec::new(),
            slot: None,
            initialized: false,
        })
    }
    /// Forward live operands and retain actual native callbacks.
    pub(super) fn update(
        &mut self,
        content: &super::AttachmentContent<'_>,
        escape: Rc<std::cell::Cell<bool>>,
    ) -> Result<(), AttachmentHostError> {
        let super::AttachmentContent::Input {
            props,
            accessible_label,
            value_changed,
            context,
            ..
        } = content
        else {
            return Ok(());
        };
        properties(
            &self.element,
            props,
            *value_changed || !self.initialized,
            accessible_label,
        )?;
        self.initialized = true;
        self.listeners.clear();
        for (name, callback) in [("input", &props.on_input), ("change", &props.on_change)] {
            if let Some(callback) = callback {
                self.listeners
                    .push(listen(&self.element, name, Rc::clone(callback))?);
            }
        }
        let key_down = props.on_key_down.clone();
        let drawing = (*context).clone();
        self.listeners.push(listen(
            &self.element,
            "keydown",
            Rc::new(move |e| {
                let Ok(event) = e.dyn_into::<KeyboardEvent>() else {
                    return;
                };
                if let Some(callback) = &key_down {
                    callback(event.clone());
                }
                admit_escape(&event, &escape, &drawing);
            }),
        )?);
        if let Some(callback) = &props.on_key_up {
            let callback = Rc::clone(callback);
            self.listeners.push(listen(
                &self.element,
                "keyup",
                Rc::new(move |e| {
                    let _ = e.dyn_into::<KeyboardEvent>().map(|event| callback(event));
                }),
            )?);
        }
        self.bind_ref(props.input_ref.as_ref())
    }
    /// Replace only this field's owned native reference assignment.
    fn bind_ref(&mut self, reference: Option<&InputRef>) -> Result<(), AttachmentHostError> {
        if matches!((&self.slot, reference), (Some(a), Some(b)) if Rc::ptr_eq(a, b)) {
            return Ok(());
        }
        self.clear_ref();
        if let Some(slot) = reference {
            *slot
                .try_borrow_mut()
                .map_err(|_| AttachmentHostError::BorrowConflict)? = Some(self.element.clone());
            self.slot = Some(Rc::clone(slot));
        }
        Ok(())
    }
    /// Never clear a reference subsequently assigned by another owner.
    fn clear_ref(&mut self) {
        if let Some(slot) = self.slot.take()
            && let Ok(mut slot) = slot.try_borrow_mut()
            && slot.as_ref() == Some(&self.element)
        {
            *slot = None;
        }
    }
}
impl Drop for DomInput {
    fn drop(&mut self) {
        self.listeners.clear();
        self.clear_ref();
    }
}
/// Delegate validation, input modes and numeric serialization to their owners.
fn properties(
    element: &HtmlInputElement,
    props: &InputProps,
    value_changed: bool,
    accessible_label: &str,
) -> Result<(), AttachmentHostError> {
    let kind = match props.input_type {
        InputType::Text => "text",
        InputType::Email => "email",
        InputType::Password => "password",
        InputType::Number => "number",
        InputType::Url => "url",
        InputType::Tel => "tel",
        InputType::Search => "search",
    };
    element.set_type(kind);
    if value_changed {
        element.set_value(&props.value);
    }
    element.set_placeholder(&props.placeholder);
    element.set_disabled(props.disabled);
    element.set_required(props.required);
    element.set_name(&props.name);
    element.set_autocomplete(&props.autocomplete);
    for (name, value) in [("min", props.min), ("max", props.max), ("step", props.step)] {
        let value = value.map_or_else(String::new, |value| {
            ryu_js::Buffer::new().format(value).to_owned()
        });
        element
            .set_attribute(name, &value)
            .map_err(|e| host_error(&e))?;
    }
    let label = (!accessible_label.is_empty()).then_some(accessible_label);
    let error = (!props.error.is_empty()).then_some(props.error.as_str());
    let invalid = error.map(|_| "");
    for (name, text) in [
        ("aria-label", label),
        ("aria-description", error),
        ("aria-invalid", invalid),
    ] {
        text.map_or_else(
            || element.remove_attribute(name),
            |text| element.set_attribute(name, text),
        )
        .map_err(|e| host_error(&e))?;
    }
    let (padding, size) = match props.size {
        InputSize::Sm | InputSize::Md => (12, "calc(var(--maestro-type-body-size) * 0.875)"),
        InputSize::Lg => (16, "var(--maestro-type-body-size)"),
    };
    element
        .set_attribute("class", "maestro-input-control")
        .map_err(|e| host_error(&e))?;
    element.set_attribute("style", &format!("width:100%;height:100%;box-sizing:border-box;padding:4px {padding}px;font-size:{size};")).map_err(|e| host_error(&e))
}
/// Share one stylesheet for native controls in this document.
fn install_style(element: &Element) -> Result<(), AttachmentHostError> {
    let document = element
        .owner_document()
        .ok_or_else(|| AttachmentHostError::Host("Input has no document".into()))?;
    crate::browser::install_stylesheet(
        &document,
        "maestro-input-style",
        r"
.maestro-input-control { min-width:0;border:1px solid var(--maestro-color-border);border-radius:var(--maestro-radii-control);background:transparent;color:var(--maestro-color-text);font-family:var(--maestro-type-body-font);outline:none;box-shadow:0 1px 2px color-mix(in srgb,var(--maestro-color-text) 5%,transparent);transition:color .15s,box-shadow .15s; }
.maestro-input-control::placeholder { color:var(--maestro-color-muted); }
.maestro-input-control::selection { background:var(--maestro-color-accent);color:var(--maestro-color-on-accent); }
.maestro-input-control:focus-visible { border-color:var(--maestro-color-accent);box-shadow:0 0 0 3px color-mix(in srgb,var(--maestro-color-accent) 50%,transparent); }
.maestro-input-control[aria-invalid] { border-color:var(--maestro-color-error);box-shadow:0 0 0 3px color-mix(in srgb,var(--maestro-color-error) 20%,transparent); }
.dark .maestro-input-control { background:color-mix(in srgb,var(--maestro-color-border) 30%,transparent); }
.dark .maestro-input-control[aria-invalid] { box-shadow:0 0 0 3px color-mix(in srgb,var(--maestro-color-error) 40%,transparent); }
.maestro-input-control:disabled { pointer-events:none;cursor:not-allowed;opacity:.5; }
",
    )
}

/// Share exact native-key admission between editable fields and semantic controls.
pub(super) fn admit_escape(
    event: &KeyboardEvent,
    pending: &std::cell::Cell<bool>,
    context: &egui::Context,
) {
    if event.key() == "Escape" && !event.cancel_bubble() {
        pending.set(true);
        context.request_repaint();
    }
}

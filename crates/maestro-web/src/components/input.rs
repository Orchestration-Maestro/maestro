//! Native single-line field that keeps edited text across operand updates.
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    dom::DomView,
    tools::types::ToolUiError,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlInputElement, KeyboardEvent};

/// Accepted native input modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputType {
    /// Plain text.
    #[default]
    Text,
    /// Electronic mail.
    Email,
    /// Masked text.
    Password,
    /// Native number.
    Number,
    /// Web address.
    Url,
    /// Telephone.
    Tel,
    /// Search text.
    Search,
}
/// Field height and text size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputSize {
    /// Small.
    Sm,
    /// Medium.
    #[default]
    Md,
    /// Large.
    Lg,
}
/// Callback with the actual browser event.
pub type EventCallback = Rc<dyn Fn(Event)>;
/// Callback with the actual keyboard event.
pub type KeyCallback = Rc<dyn Fn(KeyboardEvent)>;
/// Field operands; the callbacks in effect are the most recently supplied ones.
#[derive(Clone, Default)]
pub struct InputProps {
    /// Native element identifier; empty omits it.
    pub id: String,
    /// Native mode.
    pub input_type: InputType,
    /// Dimensions.
    pub size: InputSize,
    /// Text assigned when it differs from the previously supplied value.
    pub value: String,
    /// Empty-text hint.
    pub placeholder: String,
    /// Visible label naming the field; empty omits it.
    pub label: String,
    /// Error text below the field; empty omits it and clears the invalid state.
    pub error: String,
    /// Whether the field refuses interaction.
    pub disabled: bool,
    /// Whether the text cannot be edited.
    pub readonly: bool,
    /// Whether a value is required; a labelled field shows the localized marker.
    pub required: bool,
    /// Native form name.
    pub name: String,
    /// Native autocomplete setting.
    pub autocomplete: String,
    /// Lower bound.
    pub min: Option<f64>,
    /// Upper bound.
    pub max: Option<f64>,
    /// Native numeric step.
    pub step: Option<f64>,
    /// Input callback.
    pub on_input: Option<EventCallback>,
    /// Change callback.
    pub on_change: Option<EventCallback>,
    /// Key-down callback.
    pub on_key_down: Option<KeyCallback>,
    /// Key-up callback.
    pub on_key_up: Option<KeyCallback>,
    /// Wrapper classes appended after the field layout class.
    pub class_name: String,
}
/// Native event listener retained with the field.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// A retained field: its wrapper, optional label, input and optional error.
pub struct Input {
    /// Wrapper the caller places.
    root: Element,
    /// Label naming the field, attached only while a label is supplied.
    label: Element,
    /// Label text.
    label_text: Element,
    /// Required marker, attached only for a required labelled field.
    marker: Element,
    /// The native field.
    field: HtmlInputElement,
    /// Error text, attached only while an error is supplied.
    error: Element,
    /// Current operands read by the native listeners.
    props: Rc<RefCell<InputProps>>,
    /// Value supplied by the last update, compared before overwriting native edits.
    supplied: RefCell<String>,
    /// Native listeners released with the field.
    _listeners: Vec<Listener>,
}
thread_local! {
    /// Counter for page-unique label identifiers.
    static NEXT_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
/// A page-unique element identifier with `prefix`.
pub(crate) fn unique_id(prefix: &str) -> String {
    let id = NEXT_ID.with(|next| {
        let id = next.get();
        next.set(id + 1);
        id
    });
    format!("{prefix}-{id}")
}
impl Input {
    /// Mount the field and apply `props`.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn new(view: &DomView, props: InputProps) -> Result<Self, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "")?;
        let label = element(document, "label", "maestro-input-label")?;
        label.set_id(&unique_id("maestro-input-label"));
        let label_text = element(document, "span", "")?;
        let marker = element(document, "span", "maestro-input-required")?;
        append(&label, &label_text)?;
        let field: HtmlInputElement = document
            .create_element("input")
            .and_then(|node| node.dyn_into().map_err(Into::into))
            .map_err(|error| host_error(&error))?;
        let error = element(document, "span", "maestro-input-error")?;
        append(&root, &field)?;
        let shared = Rc::new(RefCell::new(InputProps::default()));
        let listeners = listeners(&field, &shared)?;
        let input = Self {
            root,
            label,
            label_text,
            marker,
            field,
            error,
            props: shared,
            supplied: RefCell::default(),
            _listeners: listeners,
        };
        input.field.set_value(&props.value);
        input.supplied.replace(props.value.clone());
        input.update(view, props)?;
        Ok(input)
    }
    /// Apply `props`, keeping edited text unless the supplied value changed.
    /// # Errors
    /// Returns a localization or native failure.
    pub fn update(&self, view: &DomView, props: InputProps) -> Result<(), ToolUiError> {
        let marker = if props.required && !props.label.is_empty() {
            Some(view.i18n.text("*")?)
        } else {
            None
        };
        if *self.supplied.borrow() != props.value {
            self.field.set_value(&props.value);
            self.supplied.replace(props.value.clone());
        }
        self.root
            .set_class_name(&classes("maestro-field", &props.class_name));
        self.show_label(&props.label, marker.as_deref())?;
        attributes(&self.field, &props)?;
        set_text_if_changed(&self.error, &props.error);
        if props.error.is_empty() {
            self.error.remove();
        } else if self.error.parent_node().is_none() {
            append(&self.root, &self.error)?;
        }
        let previous = self.props.replace(props);
        drop(previous);
        Ok(())
    }
    /// The wrapper the caller places.
    #[must_use]
    pub fn root(&self) -> &Element {
        &self.root
    }
    /// The operands of the last update.
    #[must_use]
    pub fn props(&self) -> InputProps {
        self.props.borrow().clone()
    }
    /// The native field, for focus and live text.
    #[must_use]
    pub fn input(&self) -> &HtmlInputElement {
        &self.field
    }
    /// Attach the label naming the field, or detach it when no label is supplied.
    fn show_label(&self, label: &str, marker: Option<&str>) -> Result<(), ToolUiError> {
        if label.is_empty() {
            self.label.remove();
            return self
                .field
                .remove_attribute("aria-labelledby")
                .map_err(|error| host_error(&error).into());
        }
        set_text_if_changed(&self.label_text, label);
        match marker {
            Some(marker) => {
                set_text_if_changed(&self.marker, marker);
                if self.marker.parent_node().is_none() {
                    append(&self.label, &self.marker)?;
                }
            }
            None => self.marker.remove(),
        }
        if self.label.parent_node().is_none() {
            self.root
                .insert_before(&self.label, Some(&self.field))
                .map_err(|error| host_error(&error))?;
        }
        self.field
            .set_attribute("aria-labelledby", &self.label.id())
            .map_err(|error| host_error(&error).into())
    }
}
/// `base` followed by the caller's classes, when there are any.
pub(crate) fn classes(base: &str, extra: &str) -> String {
    if extra.is_empty() {
        base.to_owned()
    } else {
        format!("{base} {extra}")
    }
}
/// Register the four native listeners, each reading the current callback at delivery.
fn listeners(
    field: &HtmlInputElement,
    props: &Rc<RefCell<InputProps>>,
) -> Result<Vec<Listener>, ToolUiError> {
    let mut listeners = Vec::new();
    for name in ["input", "change", "keydown", "keyup"] {
        let props = Rc::clone(props);
        let callback = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            let (plain, key) = {
                let props = props.borrow();
                match name {
                    "input" => (props.on_input.clone(), None),
                    "change" => (props.on_change.clone(), None),
                    "keydown" => (None, props.on_key_down.clone()),
                    _ => (None, props.on_key_up.clone()),
                }
            };
            if let Some(callback) = plain {
                callback(event);
            } else if let (Some(callback), Ok(event)) = (key, event.dyn_into::<KeyboardEvent>()) {
                callback(event);
            }
        });
        listeners.push(
            EventSubscription::new(field.clone().into(), name, callback)
                .map_err(|error| host_error(&error))?,
        );
    }
    Ok(listeners)
}
/// The native spelling of an input mode.
fn input_type(kind: InputType) -> &'static str {
    match kind {
        InputType::Text => "text",
        InputType::Email => "email",
        InputType::Password => "password",
        InputType::Number => "number",
        InputType::Url => "url",
        InputType::Tel => "tel",
        InputType::Search => "search",
    }
}
/// Reflect the field's native properties and attributes from `props`.
fn attributes(field: &HtmlInputElement, props: &InputProps) -> Result<(), ToolUiError> {
    let size = match props.size {
        InputSize::Sm => "sm",
        InputSize::Md => "md",
        InputSize::Lg => "lg",
    };
    let mut class = format!("maestro-input maestro-input-{size}");
    if !props.error.is_empty() {
        class.push_str(" maestro-input-invalid");
    }
    field.set_class_name(&class);
    field.set_type(input_type(props.input_type));
    field.set_disabled(props.disabled);
    field.set_read_only(props.readonly);
    field.set_required(props.required);
    let number = |value: Option<f64>| value.map(|n| ryu_js::Buffer::new().format(n).to_owned());
    let texts = [
        ("id", Some(props.id.clone())),
        ("placeholder", Some(props.placeholder.clone())),
        ("name", Some(props.name.clone())),
        ("autocomplete", Some(props.autocomplete.clone())),
        ("min", number(props.min)),
        ("max", number(props.max)),
        ("step", number(props.step)),
        (
            "aria-invalid",
            (!props.error.is_empty()).then(|| "true".to_owned()),
        ),
    ];
    for (name, value) in texts {
        match value.filter(|value| !value.is_empty()) {
            Some(value) => field.set_attribute(name, &value),
            None => field.remove_attribute(name),
        }
        .map_err(|error| host_error(&error))?;
    }
    Ok(())
}

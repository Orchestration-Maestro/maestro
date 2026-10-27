//! Retained fields, type choice, connection test and actions of the provider dialog.
use super::{Editor, type_name};
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
    },
    components::{
        button::{ButtonProps, ButtonVariant, button},
        input::{Input, InputProps, InputType, unique_id},
    },
    dom::report,
    tools::types::ToolUiError,
};
use maestro_app::CustomProviderType;
use std::rc::{Rc, Weak};
use wasm_bindgen::{JsCast as _, closure::Closure};
use web_sys::{Element, Event, HtmlButtonElement};

/// Provider types in offered order, with their literal labels.
const PROVIDER_TYPES: [(&str, &str); 7] = [
    ("ollama", "Ollama (auto-discovery)"),
    ("llama.cpp", "llama.cpp (auto-discovery)"),
    ("vllm", "vLLM (auto-discovery)"),
    ("lmstudio", "LM Studio (auto-discovery)"),
    ("openai-completions", "OpenAI Completions Compatible"),
    ("openai-responses", "OpenAI Responses Compatible"),
    ("anthropic-messages", "Anthropic Messages Compatible"),
];
/// Discovered model names listed before the remainder is summarized.
const LISTED: usize = 5;
/// Native listener retained with the form.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// The retained form.
pub(super) struct Form {
    /// Heading.
    heading: Element,
    /// Name, address and key fields.
    fields: [Input; 3],
    /// Type choice.
    select: Element,
    /// Connection test column, shown for discovery types.
    discovery: Element,
    /// Test control.
    test: HtmlButtonElement,
    /// Test failure.
    error: Element,
    /// Discovered model summary.
    found: Element,
    /// Guidance for manual types.
    manual: Element,
    /// Parent of the discovery column or the guidance.
    body: Element,
    /// Save control.
    save: HtmlButtonElement,
    /// Native listeners.
    _listeners: Vec<Listener>,
}
impl Form {
    /// Build the form for `editor`, returning it with the dialog content.
    pub(super) fn new(editor: &Rc<Editor>) -> Result<(Self, Element), ToolUiError> {
        let view = &editor.view;
        let document = &view.document;
        let root = element(document, "div", "maestro-provider-dialog")?;
        let top = element(document, "div", "maestro-provider-dialog-top")?;
        let heading = element(document, "h2", "maestro-provider-dialog-title")?;
        append(&top, &heading)?;
        let body = element(document, "div", "maestro-provider-dialog-body")?;
        let weak = Rc::downgrade(editor);
        let fields = [
            field(editor, &weak, 0, InputType::Text)?,
            field(editor, &weak, 1, InputType::Text)?,
            field(editor, &weak, 2, InputType::Password)?,
        ];
        let select = type_select(document)?;
        let i18n = &view.i18n;
        labelled(editor, &body, &fields, &select)?;
        let discovery = element(document, "div", "maestro-field-group")?;
        let test = action(editor, ButtonVariant::Outline)?;
        let error = element(document, "div", "maestro-provider-dialog-error")?;
        let found = element(document, "div", "maestro-provider-dialog-found")?;
        append(&discovery, &test)?;
        let manual = element(document, "div", "maestro-provider-dialog-muted")?;
        manual.set_text_content(Some(
            &i18n.text("For manual provider types, add models after saving the provider.")?,
        ));
        let footer = element(document, "div", "maestro-provider-dialog-footer")?;
        let cancel = action(editor, ButtonVariant::Ghost)?;
        cancel.set_text_content(Some(&i18n.text("Cancel")?));
        let save = action(editor, ButtonVariant::Default)?;
        save.set_text_content(Some(&i18n.text("Save")?));
        append(&footer, &cancel)?;
        append(&footer, &save)?;
        for part in [&top, &body, &footer] {
            append(&root, part)?;
        }
        let listeners = listeners(&weak, &select, [&test, &cancel, &save])?;
        let form = Self {
            heading,
            fields,
            select,
            discovery,
            test,
            error,
            found,
            manual,
            body,
            save,
            _listeners: listeners,
        };
        Ok((form, root))
    }
    /// Show the edited values, the type's section and the control states.
    pub(super) fn render(&self, editor: &Editor) -> Result<(), ToolUiError> {
        let view = &editor.view;
        let i18n = &view.i18n;
        let heading = if editor.provider.is_some() {
            "Edit Provider"
        } else {
            "Add Provider"
        };
        set_text_if_changed(&self.heading, &i18n.text(heading)?);
        let values = [
            (
                editor.name.borrow().clone(),
                i18n.text("e.g., My Ollama Server")?,
            ),
            (
                editor.base_url.borrow().clone(),
                i18n.text("e.g., http://localhost:11434")?,
            ),
            (
                editor.api_key.borrow().clone(),
                i18n.text("Leave empty if not required")?,
            ),
        ];
        for (input, (value, placeholder)) in self.fields.iter().zip(values) {
            let mut props = input.props();
            props.value = value;
            props.placeholder = placeholder;
            input.update(view, props)?;
        }
        js_sys::Reflect::set(
            &self.select,
            &"value".into(),
            &type_name(editor.kind.get()).into(),
        )
        .map_err(|e| host_error(&e))?;
        let base_url = editor.base_url.borrow().clone();
        let testing = editor.testing.get();
        self.save
            .set_disabled(editor.name.borrow().is_empty() || base_url.is_empty());
        if editor.discovery().is_none() {
            self.discovery.remove();
            return self.show(&self.manual);
        }
        self.manual.remove();
        self.test.set_disabled(testing || base_url.is_empty());
        let label = if testing {
            "Testing..."
        } else {
            "Test Connection"
        };
        set_text_if_changed(&self.test, &i18n.text(label)?);
        let error = editor.test_error.borrow().clone();
        set_text_if_changed(&self.error, &error);
        attach(&self.discovery, &self.error, !error.is_empty())?;
        self.render_found(editor)?;
        self.show(&self.discovery)
    }
    /// Summarize the discovered models: the count, five names and how many more.
    fn render_found(&self, editor: &Editor) -> Result<(), ToolUiError> {
        let i18n = &editor.view.i18n;
        let models = editor.discovered_models.borrow();
        self.found.set_text_content(None);
        if models.is_empty() {
            self.found.remove();
            return Ok(());
        }
        let document = &editor.view.document;
        let summary = format!(
            "{} {} {}:",
            i18n.text("Discovered")?,
            models.len(),
            i18n.text("models")?
        );
        let caption = document.create_text_node(&summary);
        self.found
            .append_child(&caption)
            .map_err(|e| host_error(&e))?;
        let list = element(document, "ul", "")?;
        for model in models.iter().take(LISTED) {
            let item = element(document, "li", "")?;
            item.set_text_content(Some(&model.name));
            append(&list, &item)?;
        }
        if models.len() > LISTED {
            let item = element(document, "li", "")?;
            let more = format!(
                "...{} {} {}",
                i18n.text("and")?,
                models.len() - LISTED,
                i18n.text("more")?
            );
            item.set_text_content(Some(&more));
            append(&list, &item)?;
        }
        append(&self.found, &list)?;
        attach(&self.discovery, &self.found, true)
    }
    /// Attach `section` at the end of the body.
    fn show(&self, section: &Element) -> Result<(), ToolUiError> {
        attach(&self.body, section, true)
    }
}
/// Attach `child` at the end of `parent` while `shown`, otherwise detach it.
fn attach(parent: &Element, child: &Element, shown: bool) -> Result<(), ToolUiError> {
    if !shown {
        child.remove();
    } else if !child
        .parent_element()
        .is_some_and(|current| current.is_same_node(Some(parent)))
    {
        append(parent, child)?;
    }
    Ok(())
}
/// The native type choice listing every provider type.
fn type_select(document: &web_sys::Document) -> Result<Element, ToolUiError> {
    let select = document
        .create_element("select")
        .map_err(|e| host_error(&e))?;
    select.set_class_name("maestro-select");
    for (value, label) in PROVIDER_TYPES {
        let option = element(document, "option", "")?;
        option
            .set_attribute("value", value)
            .map_err(|e| host_error(&e))?;
        option.set_text_content(Some(label));
        append(&select, &option)?;
    }
    Ok(select)
}
/// Append each field to `body` under a label naming it.
fn labelled(
    editor: &Editor,
    body: &Element,
    fields: &[Input; 3],
    select: &Element,
) -> Result<(), ToolUiError> {
    let document = &editor.view.document;
    for (label, control) in [
        ("Provider Name", fields[0].root().clone()),
        ("Provider Type", select.clone()),
        ("Base URL", fields[1].root().clone()),
        ("API Key (Optional)", fields[2].root().clone()),
    ] {
        let group = element(document, "div", "maestro-field-group")?;
        let caption = element(document, "label", "maestro-input-label")?;
        caption.set_text_content(Some(&editor.view.i18n.text(label)?));
        let target = if control.is_same_node(Some(select)) {
            control.clone()
        } else {
            control
                .query_selector("input")
                .map_err(|e| host_error(&e))?
                .ok_or_else(|| ToolUiError::Presentation("Missing field".into()))?
        };
        if target.id().is_empty() {
            target.set_id(&unique_id("maestro-provider-field"));
        }
        let id = target.id();
        caption
            .set_attribute("for", &id)
            .map_err(|e| host_error(&e))?;
        append(&group, &caption)?;
        append(&group, &control)?;
        append(body, &group)?;
    }
    Ok(())
}
/// The name (0), address (1) or key (2) field, storing its edits in `editor`.
fn field(
    editor: &Editor,
    weak: &Weak<Editor>,
    index: usize,
    input_type: InputType,
) -> Result<Input, ToolUiError> {
    let weak = weak.clone();
    Input::new(
        &editor.view,
        InputProps {
            id: unique_id("maestro-provider-field"),
            input_type,
            on_input: Some(Rc::new(move |event: Event| {
                let Some(editor) = weak.upgrade() else {
                    return;
                };
                let value = event
                    .target()
                    .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .map(|input| input.value())
                    .unwrap_or_default();
                let slot = [&editor.name, &editor.base_url, &editor.api_key][index];
                slot.replace(value);
                report(editor.render());
            })),
            ..InputProps::default()
        },
    )
}
/// A native action control of `variant`.
fn action(editor: &Editor, variant: ButtonVariant) -> Result<HtmlButtonElement, ToolUiError> {
    button(
        &editor.view.document,
        &ButtonProps {
            variant,
            ..ButtonProps::default()
        },
        &[],
    )
    .map_err(|e| host_error(&e).into())
}
/// Register the type choice and the Test, Cancel and Save activations.
fn listeners(
    weak: &Weak<Editor>,
    select: &Element,
    [test, cancel, save]: [&HtmlButtonElement; 3],
) -> Result<Vec<Listener>, ToolUiError> {
    let handler = |action: fn(&Rc<Editor>, &Event)| {
        let weak = weak.clone();
        Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if let Some(editor) = weak.upgrade() {
                action(&editor, &event);
            }
        })
    };
    let changed: fn(&Rc<Editor>, &Event) = |editor, event| {
        let value = event
            .target()
            .and_then(|target| js_sys::Reflect::get(&target, &"value".into()).ok())
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        if let Ok(kind) = serde_json::from_value::<CustomProviderType>(value.into()) {
            editor.select_type(kind);
        }
    };
    [
        (select.clone().into(), "change", handler(changed)),
        (
            test.clone().into(),
            "click",
            handler(|editor, _| editor.test_connection()),
        ),
        (
            cancel.clone().into(),
            "click",
            handler(|editor, _| editor.base.close()),
        ),
        (
            save.clone().into(),
            "click",
            handler(|editor, _| editor.save()),
        ),
    ]
    .into_iter()
    .map(|(target, name, callback): (web_sys::EventTarget, _, _)| {
        EventSubscription::new(target, name, callback).map_err(|e| host_error(&e).into())
    })
    .collect()
}

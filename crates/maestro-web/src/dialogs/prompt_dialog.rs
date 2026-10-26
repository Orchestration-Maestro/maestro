//! A dialog asking for one line of text, settled once by Confirm, Cancel or any close.
use super::{
    dialog::{DialogHeaderProps, dialog_content, dialog_footer, dialog_header},
    dialog_base::DialogBase,
};
use crate::{
    browser::{attachment_content::browser::host_error, event_subscription::EventSubscription},
    components::{
        button::{ButtonProps, ButtonVariant, button},
        input::{Input, InputProps, InputType, unique_id},
    },
    dom::DomView,
    tools::types::ToolUiError,
};
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
};
use tokio::sync::oneshot;
use wasm_bindgen::closure::Closure;
use web_sys::Node;

/// Panel width of a prompt.
const WIDTH: &str = "min(400px, 90vw)";
/// Native click listener retained with the prompt.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// Retained state of one open prompt.
struct Prompt {
    /// The shown dialog.
    base: DialogBase,
    /// The field, kept for its live text and listeners.
    input: Input,
    /// Settlement, taken by the first outcome.
    settle: RefCell<Option<oneshot::Sender<Option<String>>>>,
    /// Cancel and Confirm activation.
    buttons: RefCell<Vec<Click>>,
}
impl Prompt {
    /// Deliver `outcome` unless an earlier outcome was delivered, then close.
    fn finish(&self, outcome: Option<String>) {
        if let Some(settle) = self.settle.borrow_mut().take() {
            let _ = settle.send(outcome);
        }
        self.base.close();
    }
}
/// A text prompt.
pub struct PromptDialog;
impl PromptDialog {
    /// Show the prompt now, returning the confirmed text or `None` once cancelled or closed.
    ///
    /// Enter confirms and Escape cancels; the backdrop, the Close control and window Escape
    /// also settle the prompt as cancelled, so it never stays pending after it closes.
    /// An empty `title` uses the localized default.
    /// # Errors
    /// Returns a localization or native failure; nothing is shown then.
    pub fn ask(
        view: &Rc<DomView>,
        title: &str,
        message: &str,
        default_value: &str,
        is_password: bool,
    ) -> Result<impl Future<Output = Option<String>> + use<>, ToolUiError> {
        let receiver = Self::show(view, title, message, default_value, is_password)?;
        Ok(async move { receiver.await.unwrap_or(None) })
    }
    /// Open the prompt and return its settlement.
    fn show(
        view: &Rc<DomView>,
        title: &str,
        message: &str,
        default_value: &str,
        is_password: bool,
    ) -> Result<oneshot::Receiver<Option<String>>, ToolUiError> {
        let document = &view.document;
        let title = if title.is_empty() {
            view.i18n.text("Input Required")?
        } else {
            title.to_owned()
        };
        let (sender, receiver) = oneshot::channel();
        let prompt = Rc::new(Prompt {
            base: DialogBase::new(Rc::clone(view), WIDTH, "auto"),
            input: Input::new(view, InputProps::default())?,
            settle: RefCell::new(Some(sender)),
            buttons: RefCell::default(),
        });
        let weak = Rc::downgrade(&prompt);
        prompt
            .input
            .update(view, field(&weak, default_value, is_password))?;
        let nodes = actions(view, &prompt)?;
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title,
                description: Some(message.to_owned()),
                class_name: String::new(),
            },
        )?;
        label_field(&prompt.input, &header, message)?;
        let footer = dialog_footer(document, &nodes, "")?;
        let content = dialog_content(
            document,
            &[
                header.into(),
                prompt.input.root().clone().into(),
                footer.into(),
            ],
            "",
        )?;
        prompt.base.set_on_close(Some(Rc::new(move || {
            if let Some(prompt) = weak.upgrade() {
                prompt.finish(None);
            }
        })));
        let component: Rc<dyn Any> = Rc::clone(&prompt) as Rc<dyn Any>;
        prompt.base.open(vec![content.into()], Some(component))?;
        let _ = prompt.input.input().focus();
        Ok(receiver)
    }
}
/// Cancel and Confirm, retained with `prompt`.
fn actions(view: &DomView, prompt: &Rc<Prompt>) -> Result<Vec<Node>, ToolUiError> {
    let mut nodes = Vec::new();
    for (label, variant, confirm) in [
        ("Cancel", ButtonVariant::Outline, false),
        ("Confirm", ButtonVariant::Default, true),
    ] {
        let control = button(
            &view.document,
            &ButtonProps {
                variant,
                ..ButtonProps::default()
            },
            &[],
        )
        .map_err(|error| host_error(&error))?;
        control.set_text_content(Some(&view.i18n.text(label)?));
        let target = Rc::downgrade(prompt);
        let click = Closure::<dyn Fn()>::new(move || activate(&target, confirm));
        prompt.buttons.borrow_mut().push(
            EventSubscription::new(control.clone().into(), "click", click)
                .map_err(|error| host_error(&error))?,
        );
        nodes.push(control.into());
    }
    Ok(nodes)
}
/// The prompt's field: masked when asked, Enter confirms and Escape cancels.
fn field(prompt: &Weak<Prompt>, value: &str, is_password: bool) -> InputProps {
    let keys = prompt.clone();
    InputProps {
        input_type: if is_password {
            InputType::Password
        } else {
            InputType::Text
        },
        value: value.to_owned(),
        class_name: "maestro-full".into(),
        on_key_down: Some(Rc::new(move |event| match event.key().as_str() {
            "Enter" => activate(&keys, true),
            "Escape" => activate(&keys, false),
            _ => {}
        })),
        ..InputProps::default()
    }
}
/// Confirm with the field's live text, or cancel.
fn activate(prompt: &Weak<Prompt>, confirm: bool) {
    if let Some(prompt) = prompt.upgrade() {
        let outcome = confirm.then(|| prompt.input.input().value());
        prompt.finish(outcome);
    }
}

/// Name the field from the prompt text, falling back to its title for an empty message.
fn label_field(input: &Input, header: &web_sys::Element, message: &str) -> Result<(), ToolUiError> {
    let label = header
        .query_selector(if message.is_empty() { "h2" } else { "p" })
        .map_err(|error| host_error(&error))?
        .ok_or_else(|| ToolUiError::Presentation("Missing prompt label".into()))?;
    let id = unique_id("maestro-prompt-label");
    label.set_id(&id);
    input
        .input()
        .set_attribute("aria-labelledby", &id)
        .map_err(|error| host_error(&error).into())
}

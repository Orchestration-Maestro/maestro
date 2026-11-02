//! The example's system notification: its record, transcript row and model conversion.
use maestro_app::{AgentMessage, CustomAgentMessages, Message, UserContent, UserMessage};
use maestro_web::{
    MessageRenderer, default_convert_to_llm,
    dom::{DomContent, DomRoot, DomView},
    register_message_renderer,
    tools::types::ToolUiError,
};
use serde::{Deserialize, Serialize};
use std::{
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};
use wasm_bindgen::JsCast as _;

/// The browser records of a session that also holds notifications.
pub type ExampleMessages =
    maestro_app::browser::components::messages::CustomAgentMessages<SystemNotificationMessage>;

/// How a notification is presented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    /// An ordinary notice.
    #[default]
    Default,
    /// A warning.
    Destructive,
}

/// A notice shown in the transcript; the model reads it as system text in a user message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename = "system-notification")]
pub struct SystemNotificationMessage {
    /// The notice.
    pub message: String,
    /// Its presentation.
    pub variant: Variant,
    /// When it was created, as an ISO 8601 timestamp.
    pub timestamp: String,
}
impl CustomAgentMessages for SystemNotificationMessage {
    fn role(&self) -> &'static str {
        "system-notification"
    }
}

/// Draws a notification as an alert with its local creation time.
struct NotificationRenderer;
impl MessageRenderer<ExampleMessages> for NotificationRenderer {
    fn render(
        &self,
        message: &AgentMessage<ExampleMessages>,
    ) -> Result<Box<dyn DomContent>, ToolUiError> {
        let AgentMessage::Custom(custom) = message else {
            return Err(ToolUiError::Presentation("Not a notification".into()));
        };
        let custom = custom.read().unwrap_or_else(PoisonError::into_inner);
        let ExampleMessages::Other(notification) = &*custom else {
            return Err(ToolUiError::Presentation("Not a notification".into()));
        };
        Ok(Box::new(Notification(notification.clone())))
    }
}
/// A notification's transcript row.
struct Notification(SystemNotificationMessage);
impl DomContent for Notification {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let native = |error: wasm_bindgen::JsValue| ToolUiError::Presentation(format!("{error:?}"));
        let create = |class: &str| {
            let element = view.document.create_element("div").map_err(native)?;
            element.set_class_name(class);
            Ok::<_, ToolUiError>(element)
        };
        let root = create("example-notification-row")?;
        let alert = create("example-notification")?;
        alert.set_attribute("role", "alert").map_err(native)?;
        for part in [
            create("example-notification-text")?,
            create("example-notification-time")?,
        ] {
            alert.append_child(&part).map_err(native)?;
        }
        root.append_child(&alert).map_err(native)?;
        let root = DomRoot::new(root);
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        let root: &web_sys::Element = root.node.unchecked_ref();
        let part = |selector: &str| {
            root.query_selector(selector)
                .ok()
                .flatten()
                .ok_or_else(|| ToolUiError::Presentation("Missing notification part".into()))
        };
        let alert = part(".example-notification")?;
        let destructive = self.0.variant == Variant::Destructive;
        alert
            .class_list()
            .toggle_with_force("example-notification-destructive", destructive)
            .map_err(|error| ToolUiError::Presentation(format!("{error:?}")))?;
        part(".example-notification-text")?.set_text_content(Some(&self.0.message));
        let time = js_sys::Date::new(&self.0.timestamp.as_str().into())
            .to_locale_time_string_with_options("default", &js_sys::Object::new());
        part(".example-notification-time")?.set_text_content(time.as_string().as_deref());
        Ok(())
    }
}

/// Register the notification row for the example's sessions.
pub fn register_custom_message_renderers() {
    register_message_renderer::<ExampleMessages, Box<dyn DomContent>>(
        "system-notification",
        Rc::new(NotificationRenderer),
    );
}

/// A notification created now.
#[must_use]
pub fn create_system_notification(message: &str, variant: Variant) -> SystemNotificationMessage {
    SystemNotificationMessage {
        message: message.into(),
        variant,
        timestamp: js_sys::Date::new_0().to_iso_string().into(),
    }
}

/// The model's view of the history: each notification becomes a user message with the notice
/// in `<system>` tags, and everything else converts as usual.
#[must_use]
pub fn custom_convert_to_llm(messages: &[AgentMessage<ExampleMessages>]) -> Vec<Message> {
    let processed: Vec<AgentMessage<ExampleMessages>> = messages
        .iter()
        .map(|message| {
            let AgentMessage::Custom(custom) = message else {
                return message.clone();
            };
            let custom = custom.read().unwrap_or_else(PoisonError::into_inner);
            let ExampleMessages::Other(notification) = &*custom else {
                return message.clone();
            };
            AgentMessage::User(Arc::new(RwLock::new(UserMessage {
                content: UserContent::Text(format!("<system>{}</system>", notification.message)),
                timestamp: js_sys::Date::now(),
            })))
        })
        .collect();
    default_convert_to_llm(&processed)
}

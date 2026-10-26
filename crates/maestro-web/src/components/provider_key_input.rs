//! Masked credential entry that tests a key with one short completion before saving it.
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error, set_text_if_changed},
        event_subscription::EventSubscription,
        timer::{Timer, native},
    },
    components::{
        button::{ButtonProps, ButtonSize, button},
        input::{Input, InputProps, InputType, unique_id},
    },
    dom::{DomContent, DomRoot, DomView, report},
    tools::types::ToolUiError,
};
use maestro_app::{
    Context, Message, ProviderStreamOptions, StopReason, StreamOptions, UserContent, UserMessage,
    browser::utils::proxy_utils::apply_proxy_if_needed, complete, get_app_storage, get_model,
};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, HtmlButtonElement};

/// The model each known provider's key is tested against.
const TEST_MODELS: [(&str, &str); 9] = [
    ("anthropic", "claude-haiku-4-5"),
    ("openai", "gpt-4o-mini"),
    ("google", "gemini-2.5-flash"),
    ("groq", "openai/gpt-oss-20b"),
    ("openrouter", "z-ai/glm-4.6"),
    ("vercel-ai-gateway", "anthropic/claude-opus-4.5"),
    ("cerebras", "gpt-oss-120b"),
    ("xai", "grok-4-fast-non-reasoning"),
    ("zai", "glm-4.5-air"),
];
/// How long the invalid-key badge stays visible, in milliseconds.
const FAILURE_MS: i32 = 5000;
/// Placeholder shown once a key is stored.
const STORED: &str = "••••••••••••";
/// Native click listener retained with the entry.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// One provider's key entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderKeyInput {
    /// Provider identifier whose stored key is checked, tested and replaced.
    pub provider: String,
}
impl ProviderKeyInput {
    /// The key entry for `provider`.
    #[must_use]
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
        }
    }
}
/// Nodes and entry state retained across updates.
struct KeyInput {
    /// Page operands for later updates.
    view: RefCell<DomView>,
    /// Current provider identifier.
    provider: String,
    /// Typed key.
    typed: RefCell<String>,
    /// Whether a test is running.
    testing: Cell<bool>,
    /// Whether the last test or save failed, until the feedback expires.
    failed: Cell<bool>,
    /// Whether a key is stored.
    has_key: Cell<bool>,
    /// Whether the key was edited since the entry appeared or the last successful save.
    input_changed: Cell<bool>,
    /// Provider name.
    name: Element,
    /// Status after the name: testing badge or stored mark.
    status: Element,
    /// Invalid-key badge.
    invalid: Element,
    /// Masked field.
    input: Input,
    /// Save control.
    save: HtmlButtonElement,
    /// Pending expiry of the failure feedback.
    failure: RefCell<Option<Timer>>,
    /// Stored-key lookup revision, invalidated by a successful save.
    check: Cell<u64>,
    /// Save activation.
    click: RefCell<Option<Click>>,
}
impl DomContent for ProviderKeyInput {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "maestro-key-input")?;
        let heading = element(document, "div", "maestro-key-heading")?;
        let name = element(document, "label", "maestro-key-provider")?;
        let status = element(document, "span", "")?;
        let invalid = element(document, "span", "maestro-badge maestro-badge-destructive")?;
        append(&heading, &name)?;
        let row = element(document, "div", "maestro-key-row")?;
        let input = Input::new(view, InputProps::default())?;
        let save = button(
            document,
            &ButtonProps {
                size: ButtonSize::Sm,
                ..ButtonProps::default()
            },
            &[],
        )
        .map_err(|error| host_error(&error))?;
        append(&row, input.root())?;
        append(&row, &save)?;
        append(&root, &heading)?;
        append(&root, &row)?;
        let entry = Rc::new(KeyInput {
            view: RefCell::new(view.clone()),
            provider: self.provider.clone(),
            typed: RefCell::default(),
            testing: Cell::new(false),
            failed: Cell::new(false),
            has_key: Cell::new(false),
            input_changed: Cell::new(false),
            name,
            status,
            invalid,
            input,
            save,
            failure: RefCell::default(),
            check: Cell::new(0),
            click: RefCell::default(),
        });
        entry.bind()?;
        entry.render()?;
        entry.check_key_status();
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(entry)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let entry = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<KeyInput>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing key input".into()))?;
        entry.view.replace(view.clone());
        entry.render()
    }
    fn key(&self) -> Option<&str> {
        Some(&self.provider)
    }
}
impl KeyInput {
    /// Register the field and Save listeners, holding this entry weakly.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let weak = Rc::downgrade(self);
        let typed = weak.clone();
        let id = unique_id("maestro-provider-key");
        self.name
            .set_attribute("for", &id)
            .map_err(|error| host_error(&error))?;
        let props = InputProps {
            id,
            input_type: InputType::Password,
            class_name: "maestro-grow".into(),
            on_input: Some(Rc::new(move |_| {
                if let Some(entry) = typed.upgrade() {
                    entry.typed.replace(entry.input.input().value());
                    entry.input_changed.set(true);
                    report(entry.render());
                }
            })),
            ..InputProps::default()
        };
        self.input.update(&self.view.borrow(), props)?;
        let click = Closure::<dyn Fn()>::new(move || {
            if let Some(entry) = weak.upgrade() {
                entry.save_key();
            }
        });
        let click = EventSubscription::new(self.save.clone().into(), "click", click)
            .map_err(|error| host_error(&error))?;
        self.click.replace(Some(click));
        Ok(())
    }
    /// Show the current state: status, invalid badge, placeholder and Save availability.
    fn render(&self) -> Result<(), ToolUiError> {
        let view = self.view.borrow().clone();
        let i18n = &view.i18n;
        set_text_if_changed(&self.name, &self.provider);
        let (status, class) = if self.testing.get() {
            (
                i18n.text("Testing...")?,
                "maestro-badge maestro-badge-secondary",
            )
        } else if self.has_key.get() {
            ("✓".to_owned(), "maestro-key-stored")
        } else {
            (String::new(), "")
        };
        self.status.set_class_name(class);
        set_text_if_changed(&self.status, &status);
        self.place(&self.status, !status.is_empty())?;
        set_text_if_changed(&self.invalid, &i18n.text("✗ Invalid")?);
        self.place(&self.invalid, self.failed.get())?;
        set_text_if_changed(&self.save, &i18n.text("Save")?);
        let key = self.typed.borrow().clone();
        self.save.set_disabled(
            key.is_empty()
                || self.testing.get()
                || (self.has_key.get() && !self.input_changed.get()),
        );
        let placeholder = if self.has_key.get() {
            STORED.to_owned()
        } else {
            i18n.text("Enter API key")?
        };
        let mut props = self.input.props();
        props.value = key;
        props.placeholder = placeholder;
        self.input.update(&view, props)
    }
    /// Attach `node` at the end of the heading while `shown`, otherwise detach it.
    fn place(&self, node: &Element, shown: bool) -> Result<(), ToolUiError> {
        if !shown {
            node.remove();
            return Ok(());
        }
        if let Some(heading) = self.name.parent_element() {
            if node.parent_node().is_none() {
                append(&heading, node)?;
            }
            if node.is_same_node(Some(&self.status)) && self.invalid.parent_node().is_some() {
                heading
                    .insert_before(node, Some(&self.invalid))
                    .map_err(|error| host_error(&error))?;
            }
        }
        Ok(())
    }
    /// Look up whether the current provider has a stored key.
    fn check_key_status(self: &Rc<Self>) {
        let generation = self.check.get() + 1;
        self.check.set(generation);
        let provider = self.provider.clone();
        let weak = Rc::downgrade(self);
        wasm_bindgen_futures::spawn_local(async move {
            let stored = match get_app_storage() {
                Ok(storage) => storage.provider_keys.get(&provider).await,
                Err(error) => Err(error),
            };
            let Some(entry) = weak
                .upgrade()
                .filter(|entry| entry.check.get() == generation)
            else {
                return;
            };
            match stored {
                Ok(key) => {
                    entry.has_key.set(key.is_some_and(|key| !key.is_empty()));
                    report(entry.render());
                }
                Err(error) => web_sys::console::error_2(
                    &"Failed to check key status:".into(),
                    &error.to_string().into(),
                ),
            }
        });
    }
    /// Test the typed key and store it only when the test succeeds.
    fn save_key(self: &Rc<Self>) {
        let key = self.typed.borrow().clone();
        if key.is_empty() {
            return;
        }
        self.testing.set(true);
        self.failed.set(false);
        report(self.render());
        let provider = self.provider.clone();
        let weak = Rc::downgrade(self);
        wasm_bindgen_futures::spawn_local(async move {
            let success = test_api_key(&provider, &key).await;
            let saved = if success {
                store(&provider, &key).await
            } else {
                false
            };
            if let Some(entry) = weak.upgrade() {
                entry.finish(saved, &key);
            }
        });
    }
    /// Show the saved state, or the invalid badge until the feedback expires.
    fn finish(self: &Rc<Self>, saved: bool, submitted: &str) {
        self.testing.set(false);
        if saved {
            self.check.set(self.check.get() + 1);
            self.has_key.set(true);
            self.input_changed.set(*self.typed.borrow() != submitted);
        } else {
            self.failed.set(true);
            self.expire_failure(&Rc::downgrade(self));
        }
        report(self.render());
    }
    /// Clear the failure feedback after its display time.
    fn expire_failure(&self, weak: &Weak<Self>) {
        let weak = weak.clone();
        let expire = Rc::new(move || {
            if let Some(entry) = weak.upgrade() {
                entry.failed.set(false);
                report(entry.render());
            }
        });
        let timer = self
            .view
            .borrow()
            .document
            .default_view()
            .ok_or_else(|| wasm_bindgen::JsValue::from_str("No window"))
            .and_then(|window| native(window, FAILURE_MS, false, expire));
        match timer {
            Ok(timer) => {
                let previous = self.failure.replace(Some(timer));
                drop(previous);
            }
            Err(error) => web_sys::console::error_1(&error),
        }
    }
}
/// Store `key` for `provider`, logging a failure.
async fn store(provider: &str, key: &str) -> bool {
    let result = match get_app_storage() {
        Ok(storage) => storage.provider_keys.set(provider, key).await,
        Err(error) => Err(error),
    };
    if let Err(error) = &result {
        web_sys::console::error_2(&"Failed to save API key:".into(), &error.to_string().into());
    }
    result.is_ok()
}
/// Whether `api_key` completes a short request with the provider's test model.
///
/// A provider without a test model is accepted without a request, as the right model to try is
/// unknown. The proxy settings are read when the test runs.
async fn test_api_key(provider: &str, api_key: &str) -> bool {
    let Some((_, model_id)) = TEST_MODELS.iter().find(|(known, _)| *known == provider) else {
        return true;
    };
    let Some(model) = get_model(provider, model_id) else {
        return false;
    };
    match completes(model, api_key).await {
        Ok(stopped) => stopped,
        Err(error) => {
            web_sys::console::error_2(
                &format!("API key test failed for {provider}:").into(),
                &error.into(),
            );
            false
        }
    }
}
/// Send the test request through the proxy the settings select, reporting a normal stop.
async fn completes(model: maestro_app::Model, api_key: &str) -> Result<bool, String> {
    let settings = &get_app_storage()
        .map_err(|error| error.to_string())?
        .settings;
    let enabled = settings
        .get::<bool>("proxy.enabled")
        .await
        .map_err(|error| error.to_string())?;
    let url = settings
        .get::<String>("proxy.url")
        .await
        .map_err(|error| error.to_string())?;
    let proxy = url.filter(|url| enabled == Some(true) && !url.is_empty());
    let model = apply_proxy_if_needed(model, api_key, proxy.as_deref());
    let context = Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            content: UserContent::Text("Reply with: ok".into()),
            timestamp: js_sys::Date::now(),
        })],
        tools: None,
    };
    let options = ProviderStreamOptions {
        common: StreamOptions {
            api_key: Some(api_key.to_owned()),
            max_tokens: Some(200.0),
            ..StreamOptions::default()
        },
        ..ProviderStreamOptions::default()
    };
    let result = complete(model, context, Some(options))
        .await
        .map_err(|error| error.message)?;
    let stopped = result
        .read()
        .map_err(|error| error.to_string())?
        .stop_reason
        == StopReason::Stop;
    Ok(stopped)
}

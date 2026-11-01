//! Built-in settings tabs: provider keys and the CORS proxy.
use super::SettingsTab;
use crate::{
    browser::{
        attachment_content::browser::{append, element, host_error},
        event_subscription::EventSubscription,
    },
    components::{
        input::{Input, InputProps},
        provider_key_input::ProviderKeyInput,
    },
    dom::{DomContent, DomRoot, DomView, MountedContent, report},
    tools::types::ToolUiError,
};
use maestro_app::{get_app_storage, get_providers};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, HtmlButtonElement};

/// Proxy address used until one is stored.
const DEFAULT_PROXY_URL: &str = "http://localhost:3001";
/// The key entries of every built-in provider.
#[derive(Clone, Copy, Debug, Default)]
pub struct ApiKeysTab;
impl SettingsTab for ApiKeysTab {
    fn get_tab_name(&self, view: &DomView) -> Result<String, ToolUiError> {
        Ok(view.i18n.text("API Keys")?)
    }
}
impl DomContent for ApiKeysTab {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let root = element(document, "div", "maestro-settings-keys")?;
        let intro = element(document, "p", "maestro-settings-muted")?;
        intro.set_text_content(Some(&view.i18n.text(
            "Configure API keys for LLM providers. Keys are stored locally in your browser.",
        )?));
        append(&root, &intro)?;
        let mut entries = Vec::new();
        for provider in get_providers() {
            let mut slot = MountedContent::default();
            slot.show(&ProviderKeyInput::new(provider), &root, view)?;
            entries.push(slot);
        }
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(entries)),
        })
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
/// Native click listener retained with the tab.
type Click = EventSubscription<Closure<dyn Fn()>>;
/// The CORS proxy switch and address.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyTab;
/// Proxy values and the retained controls.
struct Proxy {
    /// Page operands for later updates.
    view: DomView,
    /// Whether requests that need the proxy use it.
    enabled: Cell<bool>,
    /// Proxy address.
    url: RefCell<String>,
    /// Switch control.
    switch: HtmlButtonElement,
    /// Address field.
    input: Input,
    /// Switch activation.
    click: RefCell<Option<Click>>,
}
impl SettingsTab for ProxyTab {
    fn get_tab_name(&self, view: &DomView) -> Result<String, ToolUiError> {
        Ok(view.i18n.text("Proxy")?)
    }
}
impl DomContent for ProxyTab {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let i18n = &view.i18n;
        let root = element(document, "div", "maestro-settings-proxy")?;
        let intro = element(document, "p", "maestro-settings-muted")?;
        intro.set_text_content(Some(&i18n.text("Allows browser-based apps to bypass CORS restrictions when calling LLM providers. Required for Z-AI and Anthropic with OAuth token.")?));
        let row = element(document, "div", "maestro-settings-row")?;
        let label = element(document, "span", "maestro-input-label")?;
        label.set_text_content(Some(&i18n.text("Use CORS Proxy")?));
        let switch = switch(view, &label)?;
        append(&row, &label)?;
        append(&row, &switch)?;
        let group = element(document, "div", "maestro-field-group")?;
        let caption = element(document, "label", "maestro-input-label")?;
        caption.set_text_content(Some(&i18n.text("Proxy URL")?));
        let input = Input::new(view, InputProps::default())?;
        let field = input.input();
        field.set_id(&crate::components::input::unique_id("maestro-proxy-url"));
        caption
            .set_attribute("for", &field.id())
            .map_err(|e| host_error(&e))?;
        let hint = element(document, "p", "maestro-settings-hint")?;
        hint.set_text_content(Some(&i18n.text(
            "Format: The proxy must accept requests as <proxy-url>/?url=<target-url>",
        )?));
        for part in [&caption, input.root(), &hint] {
            append(&group, part)?;
        }
        for part in [&intro, &row, &group] {
            append(&root, part)?;
        }
        let proxy = Rc::new(Proxy {
            view: view.clone(),
            enabled: Cell::new(false),
            url: RefCell::new(DEFAULT_PROXY_URL.into()),
            switch,
            input,
            click: RefCell::default(),
        });
        proxy.bind()?;
        proxy.render()?;
        wasm_bindgen_futures::spawn_local(loaded(Rc::downgrade(&proxy)));
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(proxy)),
        })
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
impl Proxy {
    /// Save on switching, keep typed addresses and save the address when it changes.
    fn bind(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let typed = Rc::downgrade(self);
        let changed = Rc::downgrade(self);
        let props = InputProps {
            id: self.input.input().id(),
            on_input: Some(Rc::new(move |_| {
                if let Some(proxy) = typed.upgrade() {
                    proxy.url.replace(proxy.input.input().value());
                }
            })),
            on_change: Some(Rc::new(move |_| {
                if let Some(proxy) = changed.upgrade() {
                    proxy.save_proxy_settings();
                }
            })),
            ..InputProps::default()
        };
        self.input.update(&self.view, props)?;
        let weak = Rc::downgrade(self);
        let click = Closure::<dyn Fn()>::new(move || {
            if let Some(proxy) = weak.upgrade() {
                proxy.enabled.set(!proxy.enabled.get());
                report(proxy.render());
                proxy.save_proxy_settings();
            }
        });
        let click = EventSubscription::new(self.switch.clone().into(), "click", click)
            .map_err(|error| host_error(&error))?;
        self.click.replace(Some(click));
        Ok(())
    }
    /// Show the switch state and the address, editable only while the proxy is on.
    fn render(&self) -> Result<(), ToolUiError> {
        let enabled = self.enabled.get();
        let state = if enabled { "checked" } else { "unchecked" };
        for (name, value) in [
            ("aria-checked", if enabled { "true" } else { "false" }),
            ("data-state", state),
        ] {
            self.switch
                .set_attribute(name, value)
                .map_err(|e| host_error(&e))?;
        }
        let mut props = self.input.props();
        props.value.clone_from(&self.url.borrow());
        props.disabled = !enabled;
        self.input.update(&self.view, props)
    }
    /// Store the switch state, then the address.
    fn save_proxy_settings(&self) {
        let enabled = self.enabled.get();
        let url = self.url.borrow().clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(error) = save(enabled, &url).await {
                web_sys::console::error_2(
                    &"Failed to save proxy settings:".into(),
                    &error.to_string().into(),
                );
            }
        });
    }
}
/// Store the switch state, then the address.
async fn save(enabled: bool, url: &str) -> std::io::Result<()> {
    let storage = get_app_storage()?;
    storage.settings.set("proxy.enabled", &enabled).await?;
    storage.settings.set("proxy.url", &url).await
}
/// The stored switch state and address.
async fn stored() -> std::io::Result<(Option<bool>, Option<String>)> {
    let storage = get_app_storage()?;
    let enabled = storage.settings.get::<bool>("proxy.enabled").await?;
    Ok((enabled, storage.settings.get::<String>("proxy.url").await?))
}
/// Show the stored switch state and address while the tab exists; absent values keep defaults.
async fn loaded(proxy: Weak<Proxy>) {
    match (stored().await, proxy.upgrade()) {
        (Ok((enabled, url)), Some(proxy)) => {
            if let Some(enabled) = enabled {
                proxy.enabled.set(enabled);
            }
            if let Some(url) = url {
                proxy.url.replace(url);
            }
            report(proxy.render());
        }
        (Err(error), _) => web_sys::console::error_2(
            &"Failed to load proxy settings:".into(),
            &error.to_string().into(),
        ),
        (Ok(_), None) => {}
    }
}
/// A switch named by `label`.
fn switch(view: &DomView, label: &Element) -> Result<HtmlButtonElement, ToolUiError> {
    let control: HtmlButtonElement = wasm_bindgen::JsCast::dyn_into(
        view.document
            .create_element("button")
            .map_err(|e| host_error(&e))?,
    )
    .map_err(|_| ToolUiError::Presentation("Created element is not a button".into()))?;
    control.set_type("button");
    control.set_class_name("maestro-switch");
    control
        .set_attribute("role", "switch")
        .map_err(|e| host_error(&e))?;
    label.set_id(&crate::components::input::unique_id("maestro-switch-label"));
    control
        .set_attribute("aria-labelledby", &label.id())
        .map_err(|e| host_error(&e))?;
    let thumb = element(&view.document, "span", "maestro-switch-thumb")?;
    append(&control, &thumb)?;
    Ok(control)
}

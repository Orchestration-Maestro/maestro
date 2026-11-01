//! Settings tab listing built-in provider keys and the configured custom servers.
use super::{custom_provider_dialog::CustomProviderDialog, settings_dialog::SettingsTab};
use crate::{
    CustomProviderCard,
    browser::{
        attachment_content::browser::{append, element, host_error},
        discovery::discover,
        event_subscription::EventSubscription,
    },
    components::{custom_provider_card::ProviderStatus, provider_key_input::ProviderKeyInput},
    dom::{DomContent, DomRoot, DomView, MountedContent, report},
    tools::types::ToolUiError,
};
use maestro_app::{
    AutoDiscoveryProviderType, CustomProvider, CustomProviderType, Presence, get_app_storage,
    get_providers,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event};

/// Choices of the Add Provider menu: stored type spelling, label and whether it is localized.
const ADD_CHOICES: [(&str, &str, bool); 7] = [
    ("ollama", "Ollama", false),
    ("llama.cpp", "llama.cpp", false),
    ("vllm", "vLLM", false),
    ("lmstudio", "LM Studio", false),
    ("openai-completions", "OpenAI Completions Compatible", true),
    ("openai-responses", "OpenAI Responses Compatible", true),
    ("anthropic-messages", "Anthropic Messages Compatible", true),
];
/// Native listener retained with the tab.
type Listener = EventSubscription<Closure<dyn Fn(Event)>>;
/// A card action receiving the card's provider.
type CardAction = Rc<dyn Fn(Rc<RefCell<CustomProvider>>)>;
/// The providers and models settings tab.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProvidersModelsTab;
/// Loaded providers, their discovery status and the retained sections.
struct Tab {
    /// Page operands, also handed to the dialogs this tab opens.
    view: Rc<DomView>,
    /// Configured custom providers in stored order.
    custom_providers: RefCell<Vec<CustomProvider>>,
    /// Discovery status by provider identifier.
    provider_status: RefCell<HashMap<String, ProviderStatus>>,
    /// Mounted key entries of the built-in providers.
    _keys: Vec<MountedContent>,
    /// Empty-state text.
    empty: Element,
    /// Card column.
    cards: Element,
    /// Parent of the empty state or the cards.
    custom: Element,
    /// Mounted cards by provider identifier, each in its own slot.
    mounted: RefCell<Vec<(String, Element, MountedContent)>>,
    /// Add Provider menu change.
    add: RefCell<Option<Listener>>,
}
impl SettingsTab for ProvidersModelsTab {
    fn get_tab_name(&self, _: &DomView) -> Result<String, ToolUiError> {
        Ok("Providers & Models".into())
    }
}
impl DomContent for ProvidersModelsTab {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let document = &view.document;
        let view = Rc::new(view.clone());
        let root = element(document, "div", "maestro-providers-tab")?;
        let known = section(
            &view,
            "Cloud Providers",
            "Cloud LLM providers with predefined models. API keys are stored locally in your browser.",
        )?;
        let keys = element(document, "div", "maestro-providers-keys")?;
        let mut mounted_keys = Vec::new();
        for provider in get_providers() {
            let mut slot = MountedContent::default();
            slot.show(&ProviderKeyInput::new(provider), &keys, &view)?;
            mounted_keys.push(slot);
        }
        append(&known, &keys)?;
        let separator = element(document, "div", "maestro-providers-separator")?;
        let custom = section(
            &view,
            "Custom Providers",
            "User-configured servers with auto-discovered or manually defined models.",
        )?;
        let menu = add_menu(&view)?;
        if let Some(heading) = custom.first_element_child() {
            append(&heading, &menu)?;
        }
        let empty = element(document, "div", "maestro-providers-empty")?;
        empty.set_text_content(Some(
            "No custom providers configured. Click 'Add Provider' to get started.",
        ));
        let cards = element(document, "div", "maestro-providers-cards")?;
        for part in [&known, &separator, &custom] {
            append(&root, part)?;
        }
        let tab = Rc::new(Tab {
            view: Rc::clone(&view),
            custom_providers: RefCell::default(),
            provider_status: RefCell::default(),
            _keys: mounted_keys,
            empty,
            cards,
            custom,
            mounted: RefCell::default(),
            add: RefCell::default(),
        });
        tab.add
            .replace(Some(add_listener(&menu, Rc::downgrade(&tab))?));
        tab.render()?;
        tab.load_custom_providers();
        Ok(DomRoot {
            node: root.into(),
            owner: Some(Box::new(tab)),
        })
    }
    fn update(&self, _: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        Ok(())
    }
}
impl Tab {
    /// Reload the custom providers, checking each discovery server.
    fn load_custom_providers(self: &Rc<Self>) {
        wasm_bindgen_futures::spawn_local(loaded(Rc::downgrade(self)));
    }
    /// Show the loaded providers and start checking each discovery server.
    fn show_providers(self: &Rc<Self>, providers: Vec<CustomProvider>) {
        self.custom_providers.replace(providers.clone());
        report(self.render());
        for provider in providers {
            self.check_provider_status(provider, false);
        }
    }
    /// Show checking, then the discovered model count or disconnection.
    ///
    /// An explicit refresh also reports a failure with an alert.
    fn check_provider_status(self: &Rc<Self>, provider: CustomProvider, refresh: bool) {
        let Some(kind) = discovery(&provider) else {
            return;
        };
        self.set_status(&provider.id, ProviderStatus::Checking);
        wasm_bindgen_futures::spawn_local(checked(Rc::downgrade(self), provider, kind, refresh));
    }
    /// Show a finished check, alerting a failed explicit refresh.
    fn show_check(
        self: &Rc<Self>,
        provider: &CustomProvider,
        result: Result<usize, String>,
        refresh: bool,
    ) {
        match result {
            Ok(model_count) => {
                self.set_status(&provider.id, ProviderStatus::Connected { model_count });
            }
            Err(error) => {
                self.set_status(&provider.id, ProviderStatus::Disconnected);
                if refresh {
                    web_sys::console::error_2(
                        &format!("Failed to refresh provider {}:", provider.name).into(),
                        &error.clone().into(),
                    );
                    self.alert(&format!("Failed to refresh provider: {error}"));
                }
            }
        }
    }
    /// Record and show one provider's status.
    fn set_status(self: &Rc<Self>, id: &str, status: ProviderStatus) {
        self.provider_status
            .borrow_mut()
            .insert(id.to_owned(), status);
        report(self.render());
    }
    /// Show the empty state or one card per provider, reusing each provider's card.
    fn render(self: &Rc<Self>) -> Result<(), ToolUiError> {
        let providers = self.custom_providers.borrow().clone();
        if providers.is_empty() {
            self.cards.remove();
            if self.empty.parent_node().is_none() {
                append(&self.custom, &self.empty)?;
            }
        } else {
            self.empty.remove();
            if self.cards.parent_node().is_none() {
                append(&self.custom, &self.cards)?;
            }
        }
        let mut previous = std::mem::take(&mut *self.mounted.borrow_mut());
        let mut mounted = Vec::new();
        for provider in providers {
            let card = self.card(&provider);
            let (id, slot, mut content) =
                match previous.iter().position(|(id, ..)| *id == provider.id) {
                    Some(index) => previous.swap_remove(index),
                    None => (
                        provider.id.clone(),
                        element(&self.view.document, "div", "")?,
                        MountedContent::default(),
                    ),
                };
            content.show(&card, &slot, &self.view)?;
            append(&self.cards, &slot)?;
            mounted.push((id, slot, content));
        }
        for (_, slot, _) in previous {
            slot.remove();
        }
        self.mounted.replace(mounted);
        Ok(())
    }
    /// The card for `provider` with this tab's actions.
    fn card(self: &Rc<Self>, provider: &CustomProvider) -> CustomProviderCard {
        let mut card = CustomProviderCard::new(Rc::new(RefCell::new(provider.clone())));
        card.is_auto_discovery = discovery(provider).is_some();
        card.status = self.provider_status.borrow().get(&provider.id).copied();
        let weak = Rc::downgrade(self);
        card.on_refresh = Some(action(&weak, |tab, provider| {
            tab.check_provider_status(provider, true);
        }));
        card.on_edit = Some(action(&weak, |tab, provider| {
            tab.open_dialog(Some(provider), None);
        }));
        card.on_delete = Some(action(&weak, Tab::delete_provider));
        card
    }
    /// Open the provider dialog, reloading the providers after a save.
    fn open_dialog(
        self: &Rc<Self>,
        provider: Option<CustomProvider>,
        kind: Option<CustomProviderType>,
    ) {
        let weak = Rc::downgrade(self);
        let on_save: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(tab) = weak.upgrade() {
                tab.load_custom_providers();
            }
        });
        if let Err(error) = CustomProviderDialog::open(&self.view, provider, kind, Some(on_save)) {
            web_sys::console::error_1(&error.to_string().into());
        }
    }
    /// Delete `provider` after confirmation, then reload the providers.
    fn delete_provider(self: &Rc<Self>, provider: CustomProvider) {
        let confirmed = self
            .view
            .document
            .default_view()
            .and_then(|window| {
                window
                    .confirm_with_message("Are you sure you want to delete this provider?")
                    .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        wasm_bindgen_futures::spawn_local(deleted(Rc::downgrade(self), provider.id));
    }
    /// Show a platform alert.
    fn alert(&self, message: &str) {
        if let Some(window) = self.view.document.default_view() {
            let _ = window.alert_with_message(message);
        }
    }
}
/// Load the custom providers and show them while the tab exists.
async fn loaded(tab: Weak<Tab>) {
    let loaded = match get_app_storage() {
        Ok(storage) => storage.custom_providers.get_all().await,
        Err(error) => Err(error),
    };
    match (loaded, tab.upgrade()) {
        (Ok(providers), Some(tab)) => tab.show_providers(providers),
        (Err(error), _) => web_sys::console::error_2(
            &"Failed to load custom providers:".into(),
            &error.to_string().into(),
        ),
        (Ok(_), None) => {}
    }
}
/// Check one discovery server and show the outcome while the tab exists.
async fn checked(
    tab: Weak<Tab>,
    provider: CustomProvider,
    kind: AutoDiscoveryProviderType,
    refresh: bool,
) {
    let key = match &provider.api_key {
        Presence::Present(key) => Some(key.as_str()),
        Presence::Missing | Presence::Null => None,
    };
    let result = discover(kind, &provider.base_url, key)
        .await
        .map(|models| models.len())
        .map_err(|error| error.to_string());
    if let Some(tab) = tab.upgrade() {
        tab.show_check(&provider, result, refresh);
    }
}
/// Delete one provider, then reload the list while the tab exists.
async fn deleted(tab: Weak<Tab>, id: String) {
    let deleted = match get_app_storage() {
        Ok(storage) => storage.custom_providers.delete(&id).await,
        Err(error) => Err(error),
    };
    match (deleted, tab.upgrade()) {
        (Ok(()), Some(tab)) => tab.load_custom_providers(),
        (Err(error), _) => web_sys::console::error_2(
            &"Failed to delete provider:".into(),
            &error.to_string().into(),
        ),
        (Ok(()), None) => {}
    }
}
/// A card action running `run` on this tab with a copy of the card's provider.
fn action(tab: &Weak<Tab>, run: fn(&Rc<Tab>, CustomProvider)) -> CardAction {
    let tab = tab.clone();
    Rc::new(move |provider: Rc<RefCell<CustomProvider>>| {
        let provider = provider.borrow().clone();
        if let Some(tab) = tab.upgrade() {
            run(&tab, provider);
        }
    })
}
/// The discovery family of an auto-discovery provider.
fn discovery(provider: &CustomProvider) -> Option<AutoDiscoveryProviderType> {
    match provider.r#type {
        CustomProviderType::AutoDiscovery(kind) => Some(kind),
        CustomProviderType::Manual(_) => None,
    }
}
/// A section whose first child holds its heading and description.
fn section(view: &DomView, title: &str, description: &str) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let root = element(document, "div", "maestro-providers-section")?;
    let head = element(document, "div", "maestro-providers-head")?;
    let text = element(document, "div", "")?;
    let heading = element(document, "h3", "maestro-providers-title")?;
    heading.set_text_content(Some(title));
    let paragraph = element(document, "p", "maestro-providers-description")?;
    paragraph.set_text_content(Some(description));
    append(&text, &heading)?;
    append(&text, &paragraph)?;
    append(&head, &text)?;
    append(&root, &head)?;
    Ok(root)
}
/// The Add Provider menu, whose placeholder stays selected between choices.
fn add_menu(view: &DomView) -> Result<Element, ToolUiError> {
    let document = &view.document;
    let menu = document
        .create_element("select")
        .map_err(|e| host_error(&e))?;
    menu.set_class_name("maestro-select maestro-select-sm");
    let placeholder = element(document, "option", "")?;
    placeholder
        .set_attribute("value", "")
        .map_err(|e| host_error(&e))?;
    placeholder
        .set_attribute("disabled", "")
        .map_err(|e| host_error(&e))?;
    placeholder
        .set_attribute("selected", "")
        .map_err(|e| host_error(&e))?;
    placeholder.set_text_content(Some(&view.i18n.text("Add Provider")?));
    append(&menu, &placeholder)?;
    for (value, label, localized) in ADD_CHOICES {
        let option = element(document, "option", "")?;
        option
            .set_attribute("value", value)
            .map_err(|e| host_error(&e))?;
        let label = if localized {
            view.i18n.text(label)?
        } else {
            label.to_owned()
        };
        option.set_text_content(Some(&label));
        append(&menu, &option)?;
    }
    menu.set_attribute("aria-label", &view.i18n.text("Add Provider")?)
        .map_err(|e| host_error(&e))?;
    Ok(menu)
}
/// Open the provider dialog for the chosen type and show the placeholder again.
fn add_listener(menu: &Element, tab: Weak<Tab>) -> Result<Listener, ToolUiError> {
    let target = menu.clone();
    let callback = Closure::<dyn Fn(Event)>::new(move |_: Event| {
        let value = js_sys::Reflect::get(&target, &"value".into())
            .ok()
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        let _ = js_sys::Reflect::set(&target, &"value".into(), &"".into());
        let kind = serde_json::from_value::<CustomProviderType>(value.into()).ok();
        if let (Some(tab), Some(kind)) = (tab.upgrade(), kind) {
            tab.open_dialog(None, Some(kind));
        }
    });
    EventSubscription::new(menu.clone().into(), "change", callback)
        .map_err(|error| host_error(&error).into())
}

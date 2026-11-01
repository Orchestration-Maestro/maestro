//! Add or edit one custom provider, testing discovery servers before saving.
mod form;

use crate::{
    browser::{attachment_content::browser::host_error, discovery::discover},
    dialogs::dialog_base::DialogBase,
    dom::{DomView, report},
    tools::types::ToolUiError,
};
use form::Form;
use maestro_app::browser::utils::model_discovery::DiscoveryError;
use maestro_app::{
    AutoDiscoveryProviderType, CustomProvider, CustomProviderType, ManualProviderType, Model,
    Presence, get_app_storage,
};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

/// Panel width of the provider dialog.
const WIDTH: &str = "min(800px, 90vw)";
/// Panel height of the provider dialog.
const HEIGHT: &str = "min(700px, 90vh)";
/// An open provider dialog. Clones share one dialog.
#[derive(Clone)]
pub struct CustomProviderDialog {
    /// Shared dialog state.
    state: Rc<Editor>,
}
/// Edited values and the retained form.
struct Editor {
    /// Page operands.
    view: Rc<DomView>,
    /// The shown dialog.
    base: DialogBase,
    /// The edited provider, absent when adding one.
    provider: Option<CustomProvider>,
    /// Notified after a successful save, before the dialog closes.
    on_save: Option<Rc<dyn Fn()>>,
    /// Display name.
    name: RefCell<String>,
    /// Server family or model protocol.
    kind: Cell<CustomProviderType>,
    /// Server address.
    base_url: RefCell<String>,
    /// Optional key.
    api_key: RefCell<String>,
    /// Whether a connection test is running.
    testing: Cell<bool>,
    /// The last connection test's failure.
    test_error: RefCell<String>,
    /// Models found by the last connection test, or the edited provider's models.
    discovered_models: RefCell<Vec<Model>>,
    /// Retained fields and controls.
    form: RefCell<Option<Form>>,
}
impl CustomProviderDialog {
    /// Open the dialog for `provider`, or for a new provider of `initial_type`.
    ///
    /// A new provider starts with its type's default local address; saving keeps an edited
    /// provider's identifier and manual models.
    /// # Errors
    /// Returns a localization or native failure; the dialog then stays closed.
    pub fn open(
        view: &Rc<DomView>,
        provider: Option<CustomProvider>,
        initial_type: Option<CustomProviderType>,
        on_save: Option<Rc<dyn Fn()>>,
    ) -> Result<Self, ToolUiError> {
        let editor = Rc::new(Editor {
            view: Rc::clone(view),
            base: DialogBase::new(Rc::clone(view), WIDTH, HEIGHT),
            provider,
            on_save,
            name: RefCell::default(),
            kind: Cell::new(CustomProviderType::Manual(
                ManualProviderType::OpenaiCompletions,
            )),
            base_url: RefCell::default(),
            api_key: RefCell::default(),
            testing: Cell::new(false),
            test_error: RefCell::default(),
            discovered_models: RefCell::default(),
            form: RefCell::default(),
        });
        editor.initialize_from_provider(initial_type);
        let (form, content) = Form::new(&editor)?;
        editor.form.replace(Some(form));
        editor.render()?;
        let component: Rc<dyn Any> = Rc::clone(&editor) as Rc<dyn Any>;
        editor.base.open(vec![content.into()], Some(component))?;
        Ok(Self { state: editor })
    }
    /// Close without saving.
    pub fn close(&self) {
        self.state.base.close();
    }
    /// Whether the dialog is shown.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.base.is_open()
    }
}
impl Editor {
    /// Start from the edited provider's values, or from an empty provider of `initial_type`.
    fn initialize_from_provider(&self, initial_type: Option<CustomProviderType>) {
        if let Some(provider) = &self.provider {
            self.name.replace(provider.name.clone());
            self.kind.set(provider.r#type);
            self.base_url.replace(provider.base_url.clone());
            self.api_key
                .replace(present(&provider.api_key).cloned().unwrap_or_default());
            self.discovered_models
                .replace(present(&provider.models).cloned().unwrap_or_default());
        } else {
            if let Some(kind) = initial_type {
                self.kind.set(kind);
            }
            self.update_default_base_url();
        }
    }
    /// Fill an empty address with the type's default local server.
    fn update_default_base_url(&self) {
        if !self.base_url.borrow().is_empty() {
            return;
        }
        let default = match self.kind.get() {
            CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Ollama) => {
                "http://localhost:11434"
            }
            CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::LlamaCpp) => {
                "http://localhost:8080"
            }
            CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Vllm) => {
                "http://localhost:8000"
            }
            CustomProviderType::AutoDiscovery(AutoDiscoveryProviderType::Lmstudio) => {
                "http://localhost:1234"
            }
            CustomProviderType::Manual(_) => "",
        };
        self.base_url.replace(default.to_owned());
    }
    /// The discovery family of an auto-discovery type.
    fn discovery(&self) -> Option<AutoDiscoveryProviderType> {
        match self.kind.get() {
            CustomProviderType::AutoDiscovery(kind) => Some(kind),
            CustomProviderType::Manual(_) => None,
        }
    }
    /// Choose another type, starting again from its default address.
    fn select_type(&self, kind: CustomProviderType) {
        self.kind.set(kind);
        self.base_url.replace(String::new());
        self.update_default_base_url();
        report(self.render());
    }
    /// Show the edited values.
    fn render(&self) -> Result<(), ToolUiError> {
        match self.form.borrow().as_ref() {
            Some(form) => form.render(self),
            None => Ok(()),
        }
    }
    /// List the server's models, showing them or the failure.
    fn test_connection(self: &Rc<Self>) {
        let Some(kind) = self.discovery() else {
            return;
        };
        self.testing.set(true);
        self.test_error.replace(String::new());
        self.discovered_models.replace(Vec::new());
        report(self.render());
        let base_url = self.base_url.borrow().clone();
        let api_key = Some(self.api_key.borrow().clone()).filter(|key| !key.is_empty());
        wasm_bindgen_futures::spawn_local(tested(Rc::downgrade(self), kind, base_url, api_key));
    }
    /// Show a finished connection test's models under the provider's name, or its failure.
    fn show_test(&self, result: Result<Vec<Model>, DiscoveryError>) {
        match result {
            Ok(models) => {
                let name = self.name.borrow().clone();
                let provider = if name.is_empty() {
                    type_name(self.kind.get())
                } else {
                    name
                };
                self.discovered_models.replace(named(models, &provider));
                self.test_error.replace(String::new());
            }
            Err(error) => {
                self.test_error.replace(error.to_string());
                self.discovered_models.replace(Vec::new());
            }
        }
        self.testing.set(false);
        report(self.render());
    }
    /// Store the provider, then notify and close; a failure keeps the dialog and its edits.
    fn save(self: &Rc<Self>) {
        let name = self.name.borrow().clone();
        let base_url = self.base_url.borrow().clone();
        if name.is_empty() || base_url.is_empty() {
            self.alert("Please fill in all required fields");
            return;
        }
        let api_key = self.api_key.borrow().clone();
        let id = match &self.provider {
            Some(provider) => provider.id.clone(),
            None => match random_uuid() {
                Ok(id) => id,
                Err(error) => {
                    self.finish_save(Err(error));
                    return;
                }
            },
        };
        let models = if self.discovery().is_some() {
            Presence::Missing
        } else {
            let kept = self
                .provider
                .as_ref()
                .and_then(|provider| present(&provider.models));
            Presence::Present(kept.cloned().unwrap_or_default())
        };
        let provider = CustomProvider {
            id,
            name,
            r#type: self.kind.get(),
            base_url,
            api_key: if api_key.is_empty() {
                Presence::Missing
            } else {
                Presence::Present(api_key)
            },
            models,
        };
        wasm_bindgen_futures::spawn_local(saved(Rc::downgrade(self), provider));
    }
    /// Notify and close after a save, or keep the dialog and its edits after a failure.
    fn finish_save(&self, saved: std::io::Result<()>) {
        match saved {
            Ok(()) => {
                if let Some(on_save) = self.on_save.clone() {
                    on_save();
                }
                self.base.close();
            }
            Err(error) => {
                web_sys::console::error_2(
                    &"Failed to save provider:".into(),
                    &error.to_string().into(),
                );
                self.alert("Failed to save provider");
            }
        }
    }
    /// Show a localized platform alert.
    fn alert(&self, message: &str) {
        let text = self
            .view
            .i18n
            .text(message)
            .unwrap_or_else(|_| message.to_owned());
        if let Some(window) = self.view.document.default_view() {
            let _ = window.alert_with_message(&text);
        }
    }
}
/// Run a connection test and show its outcome while the dialog exists.
async fn tested(
    editor: Weak<Editor>,
    kind: AutoDiscoveryProviderType,
    base_url: String,
    api_key: Option<String>,
) {
    let result = discover(kind, &base_url, api_key.as_deref()).await;
    if let Some(editor) = editor.upgrade() {
        editor.show_test(result);
    }
}
/// Store `provider` and show the outcome while the dialog exists.
async fn saved(editor: Weak<Editor>, provider: CustomProvider) {
    let saved = match get_app_storage() {
        Ok(storage) => storage.custom_providers.set(&provider).await,
        Err(error) => Err(error),
    };
    if let Some(editor) = editor.upgrade() {
        editor.finish_save(saved);
    }
}
/// `models` shown under the provider name `provider`.
fn named(models: Vec<Model>, provider: &str) -> Vec<Model> {
    models
        .into_iter()
        .map(|mut model| {
            provider.clone_into(&mut model.provider);
            model
        })
        .collect()
}
/// The value of a present property.
fn present<T>(value: &Presence<T>) -> Option<&T> {
    match value {
        Presence::Present(value) => Some(value),
        Presence::Missing | Presence::Null => None,
    }
}
/// The stored spelling of a provider type.
pub(crate) fn type_name(kind: CustomProviderType) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}
/// Request a random identifier from the platform without falling back to an empty storage key.
fn random_uuid() -> std::io::Result<String> {
    js_sys::Reflect::get(&js_sys::global(), &"crypto".into())
        .and_then(|crypto| crate::browser::invoke0(&crypto, "randomUUID"))
        .map_err(|error| std::io::Error::other(host_error(&error).to_string()))?
        .as_string()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| std::io::Error::other("The browser did not provide a random identifier"))
}

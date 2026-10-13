//! Direct browser storage, navigation and diagnostics.
use super::{I18nDiagnostic, I18nError, LanguageHost};
use wasm_bindgen::JsValue;

/// Browser platform language effects.
pub struct BrowserLanguageHost;
/// Obtain the page window.
fn window() -> Result<web_sys::Window, I18nError> {
    web_sys::window().ok_or_else(|| I18nError::Host("No browser window".into()))
}
/// Obtain browser storage without hiding access failures.
fn storage() -> Result<web_sys::Storage, I18nError> {
    window()?
        .local_storage()
        .map_err(host_error)?
        .ok_or_else(|| I18nError::Host("No browser storage".into()))
}
/// Convert the browser's thrown value at the host error boundary.
fn host_error(error: JsValue) -> I18nError {
    I18nError::Host(format!("Browser operation failed: {error:?}"))
}
impl LanguageHost for BrowserLanguageHost {
    fn stored_language(&self) -> Result<Option<String>, I18nError> {
        storage()?.get_item("language").map_err(host_error)
    }
    fn browser_language(&self) -> Result<Option<String>, I18nError> {
        Ok(window()?.navigator().language())
    }
    fn legacy_language(&self) -> Result<Option<String>, I18nError> {
        js_sys::Reflect::get(&window()?.navigator(), &JsValue::from_str("userLanguage"))
            .map(|value| value.as_string())
            .map_err(host_error)
    }
    fn store_language(&self, code: &str) -> Result<(), I18nError> {
        storage()?.set_item("language", code).map_err(host_error)
    }
    fn reload(&self) -> Result<(), I18nError> {
        window()?.location().reload().map_err(host_error)
    }
    fn unknown_translation(&self, diagnostic: &I18nDiagnostic) {
        let message = match diagnostic {
            I18nDiagnostic::MissingKey {
                category: None,
                key,
            } => format!("Unknown translation key: {key}"),
            I18nDiagnostic::MissingKey {
                category: Some(category),
                key,
            } => format!("Unknown translation key: {category}.{key}"),
            I18nDiagnostic::MissingCategory { category } => {
                format!("Unknown translation category: {category}")
            }
        };
        web_sys::console::error_1(&JsValue::from_str(&message));
    }
}

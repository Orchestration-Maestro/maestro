//! A browser-local token kept in local storage and asked for through a masked prompt.
use crate::{
    browser::attachment_content::browser::host_error, dialogs::prompt_dialog::PromptDialog,
    dom::DomView, tools::types::ToolUiError,
};
use std::rc::Rc;

/// Local-storage key of the token.
const KEY: &str = "auth-token";
/// The page's local storage.
fn storage(view: &DomView) -> Result<web_sys::Storage, ToolUiError> {
    view.document
        .default_view()
        .ok_or_else(|| ToolUiError::Presentation("Document has no window".into()))?
        .local_storage()
        .map_err(|error| host_error(&error))?
        .ok_or_else(|| ToolUiError::Presentation("Local storage is unavailable".into()))
}
/// The stored token, or one asked for through a masked prompt and then stored trimmed.
///
/// A confirmed empty or blank entry asks again; cancelling or closing the prompt returns `None`
/// without storing anything.
/// # Errors
/// Returns a storage, localization or native failure.
pub async fn get_auth_token(view: &Rc<DomView>) -> Result<Option<String>, ToolUiError> {
    let storage = storage(view)?;
    let stored = storage.get_item(KEY).map_err(|error| host_error(&error))?;
    if let Some(token) = stored.filter(|token| !token.is_empty()) {
        return Ok(Some(token));
    }
    let title = view.i18n.text("Enter Auth Token")?;
    let message = view.i18n.text("Please enter your auth token.")?;
    loop {
        let Some(entered) = PromptDialog::ask(view, &title, &message, "", true)?.await else {
            return Ok(None);
        };
        let token = entered.trim();
        if !token.is_empty() {
            storage
                .set_item(KEY, token)
                .map_err(|error| host_error(&error))?;
            return Ok(Some(token.to_owned()));
        }
    }
}
/// Remove the stored token.
/// # Errors
/// Returns a storage failure.
pub fn clear_auth_token(view: &DomView) -> Result<(), ToolUiError> {
    storage(view)?
        .remove_item(KEY)
        .map_err(|error| host_error(&error).into())
}

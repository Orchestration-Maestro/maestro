//! Accepted state after authored login, observed through the controlled raw backend.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::state::Backend;
use maestro_extensions_wasm::{
    AuthStorage, ExtensionAPI, ExtensionResult, ModelRegistry, OAuthLoginCallbacks,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// Observes login's accepted state, backend text and drainable write failures separately.
pub(super) async fn control(
    api: &ExtensionAPI,
    mode: &str,
    callbacks: Rc<OAuthLoginCallbacks>,
) -> ExtensionResult<Value> {
    let backend = Rc::new(Backend::stored(r#"{"neighbor":[null,false]}"#));
    let owner = AuthStorage::from_storage(api, backend.clone())?;
    let alias = ModelRegistry::in_memory(api, owner.clone())?.auth_storage()?;
    if mode == "write-failure" {
        backend.failure.replace("write-login");
    }
    let provider = if mode == "unknown" {
        "missing-component-login"
    } else {
        "component-login"
    };
    let result = match owner.login(provider, callbacks).await {
        Ok(()) => json!({"ok":true}),
        Err(error) => json!({"error":error}),
    };
    let accepted = alias.get("component-login")?;
    let raw: Value = serde_json::from_str(
        backend
            .raw
            .borrow()
            .as_deref()
            .ok_or("missing backend text")?,
    )
    .map_err(|error| error.to_string())?;
    let errors = alias
        .drain_errors()?
        .into_iter()
        .map(|error| error.message)
        .collect::<Vec<_>>();
    backend.failure.replace("");
    alias.reload()?;
    Ok(
        json!({"result":result,"accepted":accepted,"raw":raw,"errors":errors,"reloaded":owner.get("component-login")?}),
    )
}

//! Built-in login, key and model operations use the retained native provider.
use maestro_extensions_wasm::{ExtensionAPI, ExtensionContext, ExtensionResult};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Supplies authored interaction while leaving authorization and transport with the native owner.
pub(crate) async fn run(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    id: &str,
) -> ExtensionResult<Value> {
    let registry = context.model_registry()?;
    let provider = registry
        .auth_storage()?
        .get_oauth_providers()?
        .into_iter()
        .find(|provider| provider.id() == id)
        .ok_or("missing built-in provider")?;
    let events = Rc::new(RefCell::new(Vec::new()));
    let callbacks = super::login::callbacks(api, context, events.clone(), "builtin")?;
    let credentials = provider.login(Rc::new(callbacks)).await?;
    let key = provider.get_api_key(&credentials)?;
    let mut model = registry.get_all()?.remove(0);
    model.provider = id.into();
    model.base_url = "https://before.invalid".into();
    let mut neighbor = model.clone();
    neighbor.provider = "literal-neighbor".into();
    let models = provider.modify_models(&[model, neighbor.clone()], &credentials)?;
    let models = models.map(|models| {
        json!({
            "baseUrls":models.iter().map(|model| &model.base_url).collect::<Vec<_>>(),
            "neighbor":models[1] == neighbor
        })
    });
    Ok(
        json!({"id":provider.id(), "name":provider.name(), "server":provider.uses_callback_server(),
        "credentials":credentials, "key":key, "models":models, "events":*events.borrow()}),
    )
}

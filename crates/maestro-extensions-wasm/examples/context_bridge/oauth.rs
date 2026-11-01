//! Observes the native OAuth providers without invoking a live authorization flow.
use maestro_extensions_wasm::{ExtensionContext, ExtensionResult};
use serde_json::{Value, json};

/// Reads native provider metadata and invokes its key conversion with open credentials.
pub fn providers(context: &ExtensionContext) -> ExtensionResult<Value> {
    let credentials = serde_json::from_value(json!({
        "refresh":"literal-refresh", "access":"literal-access", "expires":42.5,
        "extensionData":{"nested":[null,false,-0.0]}
    }))
    .map_err(|error| error.to_string())?;
    let providers = context
        .model_registry()?
        .auth_storage()?
        .get_oauth_providers()?;
    providers
        .iter()
        .map(|provider| {
            Ok(json!({
                "id":provider.id(), "name":provider.name(),
                "usesCallbackServer":provider.uses_callback_server(),
                "key":provider.get_api_key(&credentials)?
            }))
        })
        .collect()
}

/// Invokes native refresh and model conversion on independently retained provider handles.
pub async fn operations(context: &ExtensionContext) -> ExtensionResult<Value> {
    let providers = context
        .model_registry()?
        .auth_storage()?
        .get_oauth_providers()?;
    let provider = providers
        .iter()
        .find(|provider| provider.id() == "component-refresh")
        .ok_or("missing controlled provider")?;
    let credentials = serde_json::from_value(json!({
        "refresh":"literal-refresh", "access":"old-access", "expires":42.5,
        "enterpriseUrl":"company.ghe.com", "extensionData":{"nested":[null,false,-0.0]}
    }))
    .map_err(|error| error.to_string())?;
    let refreshed = provider.refresh_token(credentials).await?;
    let mut model = context.model_registry()?.get_all()?.remove(0);
    model.provider = "github-copilot".into();
    model.base_url = "https://before.invalid".into();
    let mut neighbor = model.clone();
    neighbor.provider = "literal-neighbor".into();
    let selected = provider
        .modify_models(&[model, neighbor.clone()], &refreshed)?
        .ok_or("missing modifier")?;
    let absent = providers
        .iter()
        .find(|provider| provider.id() == "anthropic")
        .ok_or("missing native provider")?;
    Ok(
        json!({"credentials":refreshed,"key":provider.get_api_key(&refreshed)?,
        "baseUrls":selected.iter().map(|model| &model.base_url).collect::<Vec<_>>(),
        "neighbor":selected[1] == neighbor,"absent":absent.modify_models(&selected, &refreshed)?.is_none()}),
    )
}

thread_local! {
    /// A provider retained after its acquiring command and native membership have ended.
    static RETAINED: std::cell::RefCell<Option<std::rc::Rc<dyn maestro_extensions_wasm::OAuthProviderInterface>>> = const { std::cell::RefCell::new(None) };
}

/// Observes independent provider retention and final release without reacquiring its catalog.
pub fn retained(context: &ExtensionContext, operation: &str) -> ExtensionResult<Value> {
    match operation {
        "retain" => {
            let providers = context
                .model_registry()?
                .auth_storage()?
                .get_oauth_providers()?;
            let provider = providers
                .into_iter()
                .find(|provider| provider.id() == "component-retained")
                .ok_or("missing retained provider")?;
            let metadata = json!([
                provider.id(),
                provider.name(),
                provider.uses_callback_server()
            ]);
            RETAINED.with(|slot| *slot.borrow_mut() = Some(provider));
            Ok(metadata)
        }
        "key" => {
            let provider = RETAINED
                .with(|slot| slot.borrow().clone())
                .ok_or("provider already released")?;
            let mut credentials = serde_json::from_value(
                json!({"refresh":"r","access":"literal-access","expires":42.5}),
            )
            .map_err(|error| error.to_string())?;
            let key = provider.get_api_key(&credentials)?;
            credentials.extra.insert("deny".into(), Value::Bool(true));
            Ok(json!([key, provider.get_api_key(&credentials).err()]))
        }
        "release" => Ok(json!(
            RETAINED.with(|slot| slot.borrow_mut().take().is_some())
        )),
        _ => Err("unknown retained provider request".into()),
    }
}

thread_local! {
    /// A refresh observation retained after its creating handler returns.
    static PENDING: std::cell::RefCell<Option<maestro_extensions_wasm::ExtensionFuture<'static, maestro_extensions_wasm::OAuthCredentials>>> = const { std::cell::RefCell::new(None) };
}
/// Starts a refresh whose original facade is released before the future is observed.
pub fn pending_create(context: &ExtensionContext) -> ExtensionResult<Value> {
    let providers = context
        .model_registry()?
        .auth_storage()?
        .get_oauth_providers()?;
    let provider = providers
        .into_iter()
        .find(|provider| provider.id() == "component-refresh")
        .ok_or("missing controlled provider")?;
    let credentials = serde_json::from_value(json!({
        "refresh":"literal-refresh", "access":"old-access", "expires":42.5,
        "enterpriseUrl":"company.ghe.com", "extensionData":{"nested":[null,false,-0.0]}
    }))
    .map_err(|error| error.to_string())?;
    let pending = provider.refresh_token(credentials);
    drop(provider);
    PENDING.with(|slot| *slot.borrow_mut() = Some(pending));
    Ok(json!("retained"))
}
/// Observes the original refresh without acquiring another provider or context capability.
pub async fn pending_complete() -> ExtensionResult<Value> {
    let pending = PENDING
        .with(|slot| slot.borrow_mut().take())
        .ok_or("missing pending refresh")?;
    let credentials = pending.await?;
    Ok(json!([
        credentials.access,
        credentials.extra["extensionData"]["nested"][2]
            .as_f64()
            .ok_or("missing signed zero")?
            .is_sign_negative()
    ]))
}

#[path = "oauth/login.rs"]
mod login;
pub use login::run as login;
pub use login::storage;

#[path = "oauth/registration.rs"]
mod registration;
pub(super) use registration::{Provider, run as registration};

#[path = "oauth/reverse.rs"]
mod reverse;
pub(super) use reverse::run as reverse;

#[path = "oauth/builtin.rs"]
mod builtin;
pub(super) use builtin::run as builtin;

//! Provider catalog observations through the author facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionContext, ModelRegistry, ProviderConfig};
use serde_json::{Value, json};

/// A complete provider record with one model and request authentication.
fn config(id: &str) -> Result<ProviderConfig, String> {
    serde_json::from_value(json!({
        "name":" Literal provider ", "baseUrl":"https://provider.invalid", "apiKey":"literal-key",
        "api":"openai-completions", "headers":{"x-test":"literal"}, "authHeader":true,
        "models":[{"id":id,"name":id,"reasoning":false,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16}]
    })).map_err(|error| error.to_string())
}

/// Lists only the fixture's model identities in native order.
fn ids(catalog: &ModelRegistry) -> Result<Vec<String>, String> {
    Ok(catalog
        .get_all()?
        .into_iter()
        .filter(|model| model.provider == "author-provider")
        .map(|model| model.id)
        .collect())
}

/// Registers, resolves, replaces, reloads and removes through one retained native catalog.
pub(super) async fn observe(
    api: &ExtensionAPI,
    context: &ExtensionContext,
) -> Result<Value, String> {
    let catalog = context.model_registry()?;
    let queued = ids(&catalog)?;
    api.register_provider("author-provider", config("first")?)?;
    let before = ids(&catalog)?;
    let model = catalog
        .find("author-provider", "first")?
        .ok_or("missing registered model")?;
    let label = catalog.get_provider_display_name("author-provider")?;
    let key = catalog.get_api_key_for_provider("author-provider").await?;
    let configured = catalog.has_configured_auth(model.clone())?;
    let status = catalog.get_provider_auth_status("author-provider")?;
    let oauth = catalog.is_using_oauth(model.clone())?;
    let auth = match catalog.get_api_key_and_headers(model).await? {
        maestro_extensions_wasm::ResolvedRequestAuth::Ok { api_key, headers } => {
            json!({"ok":true,"apiKey":api_key,"headers":headers})
        }
        maestro_extensions_wasm::ResolvedRequestAuth::Error { error } => {
            json!({"ok":false,"error":error})
        }
    };
    catalog.register_provider("author-provider", config("second")?)?;
    let replaced = ids(&catalog)?;
    catalog.refresh()?;
    let refreshed = ids(&catalog)?;
    let error = catalog.get_error()?;
    api.unregister_provider("author-provider")?;
    let removed = ids(&catalog)?.is_empty();
    Ok(
        json!({"label":label,"key":key,"configured":configured,"status":status,"oauth":oauth,
        "queued":queued,"before":before,"replaced":replaced,"refreshed":refreshed,"removed":removed,
        "error":error,"auth":auth}),
    )
}

thread_local! {
    /// Independently retained credentials, not the context that returned them.
    static CREDENTIALS: std::cell::RefCell<Option<maestro_extensions_wasm::AuthStorage>> = const { std::cell::RefCell::new(None) };
}

/// Mutates credentials after catalog acquisition and keeps them beyond this callback.
pub(super) async fn credentials(
    context: &ExtensionContext,
    retained: bool,
) -> Result<Value, String> {
    if retained {
        let owner = CREDENTIALS
            .with(|slot| slot.borrow().clone())
            .ok_or("missing retained credentials")?;
        return Ok(json!(owner.get_api_key("author-credential", None).await?));
    }
    let catalog = context.model_registry()?;
    let owner = catalog.auth_storage()?;
    owner.set("author-credential", stored_key())?;
    owner.set_runtime_api_key("author-credential", "runtime-key")?;
    let runtime = catalog
        .get_api_key_for_provider("author-credential")
        .await?;
    if runtime.as_deref() != Some("runtime-key") {
        return Err("runtime key not shared".into());
    }
    owner.remove_runtime_api_key("author-credential")?;
    let result = json!({"key":catalog.get_api_key_for_provider("author-credential").await?,
        "stored":owner.get("author-credential")?,"listed":owner.list()?,
        "has":owner.has("author-credential")?,"auth":owner.has_auth("author-credential")?,
        "status":owner.get_auth_status("author-credential")?,"errors":owner.drain_errors()?});
    let all = owner.get_all()?;
    if all.len() != 1 || !all.contains_key("author-credential") {
        return Err("typed credential view detached".into());
    }
    owner.reload()?;
    owner.remove("author-credential")?;
    if owner.has("author-credential")? || owner.get("author-credential")?.is_some() {
        return Err("credential removal did not reach owner".into());
    }
    owner.set("author-credential", stored_key())?;
    owner.logout("author-credential")?;
    if !owner.list()?.is_empty() {
        return Err("logout did not remove stored credential".into());
    }
    owner.set("author-credential", stored_key())?;
    CREDENTIALS.with(|slot| slot.replace(Some(owner)));
    Ok(result)
}

/// The stored key used to compare catalog and retained-owner reads.
fn stored_key() -> maestro_extensions_wasm::AuthCredential {
    maestro_extensions_wasm::AuthCredential::ApiKey(maestro_extensions_wasm::ApiKeyCredential {
        env: None,
        key: "!checkpoint-credential".into(),
    })
}

/// Selects the provider and credential observations used by the native host tests.
pub(super) async fn command(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    request: &str,
) -> Result<Value, String> {
    match request {
        request if request.starts_with("credential files ") => {
            file_constructors(api, &request[17..]).await
        }
        #[cfg(target_arch = "wasm32")]
        "provider boundary" => boundary(api, context).await,
        "provider stale" => stale(api).await,
        "credential factory" => factory_result().await,
        "credential constructors" => constructors(api).await,
        "provider catalog" => observe(api, context).await,
        "credential owner" => credentials(context, false).await,
        "credential retained" => credentials(context, true).await,
        "credential release" => {
            CREDENTIALS.with(std::cell::RefCell::take);
            Ok(json!("released"))
        }
        _ => Err("unknown provider fixture request".into()),
    }
}

/// Constructs independent memory owners and checks live sharing after construction.
pub(super) async fn constructors(api: &ExtensionAPI) -> Result<Value, String> {
    use maestro_extensions_wasm::AuthStorage;
    let owner = AuthStorage::in_memory(api, maestro_extensions_wasm::AuthStorageData::default())?;
    let catalog = ModelRegistry::in_memory(api, owner.clone())?;
    owner.set_runtime_api_key("constructor-provider", "changed-after-construction")?;
    let alias = catalog.auth_storage()?;
    Ok(json!({
        "key": catalog.get_api_key_for_provider("constructor-provider").await?,
        "alias": alias.get_api_key("constructor-provider", None).await?,
        "errors": catalog.get_error()?, "stored": owner.list()?
    }))
}

/// Reads independently selected files, then changes credentials through an alias.
async fn file_constructors(api: &ExtensionAPI, input: &str) -> Result<Value, String> {
    let input: Value = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let auth = maestro_extensions_wasm::AuthStorage::create(api, input["auth"].as_str())?;
    let catalog = ModelRegistry::create(api, auth.clone(), input["models"].as_str())?;
    let key = catalog
        .get_api_key_for_provider("constructor-provider")
        .await?;
    let before = catalog
        .get_all()?
        .into_iter()
        .filter(|model| model.provider == "constructor-provider")
        .map(|model| model.id)
        .collect::<Vec<_>>();
    catalog
        .auth_storage()?
        .set_runtime_api_key("constructor-provider", "later-key")?;
    let after = catalog
        .get_api_key_for_provider("constructor-provider")
        .await?;
    Ok(json!({"key":key,"models":before,"after":after,"error":catalog.get_error()?}))
}

thread_local! {
    /// Catalog constructed during factory loading, before runtime actions are bound.
    static FACTORY_CATALOG: std::cell::RefCell<Option<ModelRegistry>> = const { std::cell::RefCell::new(None) };
}
/// Admits optional constructor controls without affecting ordinary component setup.
pub(super) fn factory(api: &ExtensionAPI) -> Result<(), String> {
    use maestro_extensions_wasm::{AuthStorage, FlagOptions, FlagType, FlagValue};
    factory_operations(api)?;
    for (name, type_) in [
        ("catalog-construction", FlagType::Boolean),
        ("constructor-auth-path", FlagType::String),
        ("constructor-model-path", FlagType::String),
    ] {
        api.register_flag(
            name,
            FlagOptions {
                description: None,
                type_,
                default: None,
            },
        )?;
    }
    if !matches!(
        api.get_flag("catalog-construction")?,
        Some(FlagValue::Boolean(true))
    ) {
        return Ok(());
    }
    let path = |name| -> Result<Option<String>, String> {
        Ok(match api.get_flag(name)? {
            Some(FlagValue::String(path)) => Some(path),
            _ => None,
        })
    };
    let auth = AuthStorage::create(api, path("constructor-auth-path")?.as_deref())?;
    let catalog = ModelRegistry::create(
        api,
        auth.clone(),
        path("constructor-model-path")?.as_deref(),
    )?;
    auth.set_runtime_api_key("constructor-provider", "changed-in-factory")?;
    FACTORY_CATALOG.with(|slot| slot.replace(Some(catalog)));
    Ok(())
}
/// Observes the factory-selected files and the same credentials after the factory returned.
async fn factory_result() -> Result<Value, String> {
    let catalog = FACTORY_CATALOG
        .with(|slot| slot.borrow().clone())
        .ok_or("missing factory catalog")?;
    let models = catalog
        .get_all()?
        .into_iter()
        .filter(|model| model.provider == "constructor-provider")
        .map(|model| model.id)
        .collect::<Vec<_>>();
    Ok(
        json!({"models":models,"stored":catalog.auth_storage()?.get("constructor-provider")?,"key":catalog.get_api_key_for_provider("constructor-provider").await?}),
    )
}

/// Applies literal provider operations while the facade is still loading.
fn factory_operations(api: &ExtensionAPI) -> Result<(), String> {
    use maestro_extensions_wasm::{FlagOptions, FlagType, FlagValue};
    api.register_flag(
        "provider-operations",
        FlagOptions {
            description: None,
            type_: FlagType::String,
            default: None,
        },
    )?;
    let Some(FlagValue::String(text)) = api.get_flag("provider-operations")? else {
        return Ok(());
    };
    let operations: Vec<Value> = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    for operation in operations {
        let name = operation["name"].as_str().ok_or("missing provider name")?;
        if let Some(config) = operation.get("config") {
            let config =
                serde_json::from_value(config.clone()).map_err(|error| error.to_string())?;
            api.register_provider(name, config)?;
        } else {
            api.unregister_provider(name)?;
        }
    }
    Ok(())
}

/// Rejects malformed imports without modifying an independently retained valid owner.
#[cfg(target_arch = "wasm32")]
async fn boundary(api: &ExtensionAPI, context: &ExtensionContext) -> Result<Value, String> {
    use maestro_extensions_wasm::{AuthStorage, bindings::maestro::extension::host};
    let owner = AuthStorage::in_memory(api, maestro_extensions_wasm::AuthStorageData::default())?;
    owner.set_runtime_api_key("boundary-valid", "surviving-key")?;
    let catalog = ModelRegistry::in_memory(api, owner.clone())?;
    let json_error = host::auth_storage_in_memory("{")
        .err()
        .ok_or("invalid JSON admitted")?;
    let route = host::register_provider("boundary-invalid", "{", (None, None));
    let host::RouteStep::Done(Err(provider_json)) = route.next() else {
        return Err("invalid provider JSON admitted".into());
    };
    let mut invalid = config("nonfinite")?;
    invalid.models.as_mut().ok_or("missing models")?[0]
        .cost
        .input = f64::NAN;
    let nonfinite = api.register_provider("boundary-invalid", invalid).err();
    let models = context
        .model_registry()?
        .get_all()?
        .into_iter()
        .filter(|model| model.provider == "boundary-invalid")
        .map(|model| model.id)
        .collect::<Vec<_>>();
    drop(catalog);
    let key = owner.get_api_key("boundary-valid", None).await?;
    let errors = owner.drain_errors()?;
    CREDENTIALS.with(|slot| slot.replace(Some(owner)));
    Ok(
        json!({"json":json_error,"providerJson":provider_json,"nonfinite":nonfinite,"models":models,"key":key,"errors":errors}),
    )
}

/// A stale facade cannot mutate providers while a previously acquired credential still works.
async fn stale(api: &ExtensionAPI) -> Result<Value, String> {
    let error = api
        .register_provider("boundary-stale", config("rejected")?)
        .err();
    let owner = CREDENTIALS
        .with(|slot| slot.borrow().clone())
        .ok_or("missing boundary owner")?;
    Ok(json!({"error":error,"key":owner.get_api_key("boundary-valid",None).await?}))
}

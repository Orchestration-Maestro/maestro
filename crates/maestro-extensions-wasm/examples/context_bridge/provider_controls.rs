//! Literal provider operations observed before the author call returns.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{AuthStorage, ExtensionAPI, ExtensionContext, ModelRegistry};
use serde_json::{Value, json};

thread_local! {
    /// Catalog and credential aliases retained independently of their creating context.
    static RETAINED: std::cell::RefCell<Option<(ModelRegistry, AuthStorage)>> = const { std::cell::RefCell::new(None) };
}

/// Changes one provider through the facade or its independently retained catalog.
pub(super) async fn control(
    api: &ExtensionAPI,
    context: &ExtensionContext,
    text: &str,
) -> Result<Value, String> {
    let input: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let provider = input["provider"].as_str().ok_or("missing provider")?;
    let (catalog, credentials) = if input["retained"] == true {
        let pair = RETAINED
            .with(|slot| slot.borrow().clone())
            .ok_or("missing retained catalog")?;
        (pair.0, Some(pair.1))
    } else {
        (context.model_registry()?, None)
    };
    let result = mutate(api, &catalog, &input);
    let mut answer = inspect(&catalog, provider, &input).await?;
    answer["error"] = json!(result.err());
    if let Some(credentials) = credentials {
        answer["credentialKey"] = json!(credentials.get_api_key(provider, None).await?);
    }
    Ok(answer)
}

/// Uses only the supplied owner; direct mutations never pass through the facade.
fn mutate(api: &ExtensionAPI, catalog: &ModelRegistry, input: &Value) -> Result<(), String> {
    let provider = input["provider"].as_str().ok_or("missing provider")?;
    match input["op"].as_str() {
        Some("register" | "local-register") => {
            let config = serde_json::from_value(input["config"].clone())
                .map_err(|error| error.to_string())?;
            if input["op"] == "register" {
                api.register_provider(provider, config)
            } else {
                catalog.register_provider(provider, config)
            }
        }
        Some("remove") => api.unregister_provider(provider),
        Some("local-remove") => catalog.unregister_provider(provider),
        Some("refresh") => catalog.refresh(),
        Some("retain") => {
            let auth = catalog.auth_storage()?;
            RETAINED.with(|slot| slot.replace(Some((catalog.clone(), auth))));
            Ok(())
        }
        Some("inspect") => Ok(()),
        _ => Err("unknown provider operation".into()),
    }
}

/// Resolves authentication for actual or independently supplied model metadata.
async fn inspect(catalog: &ModelRegistry, provider: &str, input: &Value) -> Result<Value, String> {
    let models = catalog
        .get_all()?
        .into_iter()
        .filter(|model| model.provider == provider)
        .collect::<Vec<_>>();
    let model = match input.get("model") {
        Some(model) => {
            Some(serde_json::from_value(model.clone()).map_err(|error| error.to_string())?)
        }
        None => models.first().cloned(),
    };
    let configured = model
        .as_ref()
        .map(|model| catalog.has_configured_auth(model.clone()))
        .transpose()?;
    let oauth = model
        .as_ref()
        .map(|model| catalog.is_using_oauth(model.clone()))
        .transpose()?;
    let auth = match model {
        Some(model) => Some(match catalog.get_api_key_and_headers(model).await? {
            maestro_extensions_wasm::ResolvedRequestAuth::Ok {
                api_key, headers, ..
            } => {
                json!({"ok":true,"apiKey":api_key,"headers":headers})
            }
            maestro_extensions_wasm::ResolvedRequestAuth::Error { error } => {
                json!({"ok":false,"error":error})
            }
        }),
        None => None,
    };
    let credentials = catalog.auth_storage()?;
    let included = credentials.get_api_key(provider, None).await?;
    let excluded = credentials
        .get_api_key(
            provider,
            Some(maestro_extensions_wasm::GetApiKeyOptions {
                include_fallback: Some(false),
            }),
        )
        .await?;
    Ok(
        json!({"models":models, "configured":configured,"oauth":oauth,"credentialKeys":[included,excluded],"label":catalog.get_provider_display_name(provider)?,
        "key":catalog.get_api_key_for_provider(provider).await?, "status":catalog.get_provider_auth_status(provider)?,
        "auth":auth, "loadError":catalog.get_error()?}),
    )
}

//! Authored OAuth operations installed through the ordinary provider registration.
use maestro_extensions_wasm::{
    AuthStorage, AuthStorageData, ExtensionAPI, ExtensionContext, ExtensionFuture, ExtensionResult,
    Model, ModelRegistry, OAuthCredentials, OAuthLoginCallbacks, OAuthProviderInterface,
    ProviderCallbacks, ProviderConfig,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// Computes a key which is not present in any credential field.
pub(crate) struct Provider;
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "ignored-author-id"
    }
    fn name(&self) -> &'static str {
        " Guest account "
    }
    fn uses_callback_server(&self) -> Option<bool> {
        Some(false)
    }
    fn login(
        &self,
        callbacks: Rc<OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        Box::pin(async move {
            (callbacks.on_auth)(
                serde_json::from_value(json!({"url":"https://guest.invalid/auth"}))
                    .map_err(|e| e.to_string())?,
            )?;
            let answer = (callbacks.on_prompt)(
                serde_json::from_value(json!({"message":"Guest code"}))
                    .map_err(|e| e.to_string())?,
            )
            .await?;
            credentials(&answer)
        })
    }
    fn refresh_token(
        &self,
        mut credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        Box::pin(async move {
            credentials.access.push_str("-rotated");
            credentials.expires = 4_102_444_500_000.0;
            Ok(credentials)
        })
    }
    fn modify_models(
        &self,
        models: &[Model],
        _: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>> {
        let mut models = models.to_vec();
        for model in &mut models {
            model.base_url = "https://guest.invalid/modified".into();
        }
        Ok(Some(models))
    }
    fn get_api_key(&self, credentials: &OAuthCredentials) -> ExtensionResult<String> {
        Ok(format!(
            "guest-computed:{}:{}",
            credentials.access, credentials.refresh
        ))
    }
}
/// Keeps open credential data beside the fields used by the guest implementation.
pub(super) fn credentials(access: &str) -> ExtensionResult<OAuthCredentials> {
    serde_json::from_value(json!({"refresh":"r", "access":access, "expires":0,
        "open":{"nested":[null,false,-0.0]}}))
    .map_err(|e| e.to_string())
}
/// Registers through the retained catalog and resolves through its native credential owner.
pub(crate) async fn run(api: &ExtensionAPI, context: &ExtensionContext) -> ExtensionResult<Value> {
    let facade = facade(api, context)?;
    let owner = AuthStorage::in_memory(api, AuthStorageData::default())?;
    let catalog = ModelRegistry::in_memory(api, owner.clone())?;
    catalog.register_provider(
        "guest-computed",
        ProviderConfig {
            callbacks: ProviderCallbacks {
                oauth: Some(Rc::new(Provider)),
                ..ProviderCallbacks::default()
            },
            ..ProviderConfig::default()
        },
    )?;
    let provider = owner
        .get_oauth_providers()?
        .into_iter()
        .find(|p| p.id() == "guest-computed")
        .ok_or("missing guest provider")?;
    let original = credentials("a")?;
    let key = provider.get_api_key(&original)?;
    let rotated = provider.refresh_token(original).await?;
    let rotated_key = provider.get_api_key(&rotated)?;
    let model = context.model_registry()?.get_all()?.remove(0);
    let models = provider
        .modify_models(&[model], &rotated)?
        .ok_or("missing guest modifier")?;
    let callbacks = Rc::new(OAuthLoginCallbacks {
        on_auth: Rc::new(|_| Ok(())),
        on_prompt: Rc::new(|_| Box::pin(async { Ok("code".into()) })),
        on_progress: None,
        on_manual_code_input: None,
        on_select: None,
        signal: None,
    });
    owner.login("guest-computed", callbacks).await?;
    let logged_in = owner.get("guest-computed")?;
    let answer = json!({"models":models.iter().map(|model| &model.base_url).collect::<Vec<_>>(),"facade":facade,"loggedIn":logged_in,"id":provider.id(),"name":provider.name(),"server":provider.uses_callback_server(),
        "key":key,"rotatedKey":rotated_key,"credentials":rotated});
    catalog.unregister_provider("guest-computed")?;
    Ok(answer)
}

/// Installs an authored implementation through the facade's bound provider action.
fn facade(api: &ExtensionAPI, context: &ExtensionContext) -> ExtensionResult<String> {
    api.register_provider(
        "guest-facade",
        ProviderConfig {
            callbacks: ProviderCallbacks {
                oauth: Some(Rc::new(Provider)),
                ..ProviderCallbacks::default()
            },
            ..ProviderConfig::default()
        },
    )?;
    let key = context
        .model_registry()?
        .auth_storage()?
        .get_oauth_providers()?
        .into_iter()
        .find(|p| p.id() == "guest-facade")
        .ok_or("missing facade provider")?
        .get_api_key(&credentials("facade")?)?;
    api.unregister_provider("guest-facade")?;
    Ok(key)
}

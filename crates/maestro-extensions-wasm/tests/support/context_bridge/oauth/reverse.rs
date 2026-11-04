//! Guest authorization invokes native interaction through retained host resources.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionFuture, ExtensionResult, Model, OAuthCredentials, OAuthLoginCallbacks,
    OAuthProviderInterface, ProviderCallbacks, ProviderConfig,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// Native interaction retained independently of its initiating login.
    static RETAINED: RefCell<Option<Rc<OAuthLoginCallbacks>>> = const { RefCell::new(None) };
    /// An input future retains the original native interaction without its callback record.
    static INPUT: RefCell<Option<ExtensionFuture<'static, String>>> = const { RefCell::new(None) };
}

/// Uses the same guest key and refresh operations with observable native interaction.
struct Provider;
impl OAuthProviderInterface for Provider {
    fn id(&self) -> &'static str {
        "ignored-reverse-id"
    }
    fn name(&self) -> &'static str {
        "Reverse interaction"
    }
    fn uses_callback_server(&self) -> Option<bool> {
        None
    }
    fn modify_models(
        &self,
        _: &[Model],
        _: &OAuthCredentials,
    ) -> ExtensionResult<Option<Vec<Model>>> {
        Ok(None)
    }
    fn login(
        &self,
        callbacks: Rc<OAuthLoginCallbacks>,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        RETAINED.with(|slot| *slot.borrow_mut() = Some(callbacks.clone()));
        Box::pin(login(callbacks))
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
    ) -> ExtensionFuture<'static, OAuthCredentials> {
        super::registration::Provider.refresh_token(credentials)
    }
    fn get_api_key(&self, credentials: &OAuthCredentials) -> ExtensionResult<String> {
        super::registration::Provider.get_api_key(credentials)
    }
}

/// Preserves literal inputs and observes absent callbacks separately from cancelled selection.
async fn login(callbacks: Rc<OAuthLoginCallbacks>) -> ExtensionResult<OAuthCredentials> {
    let presence = callbacks.callback_presence();
    let initial_aborted = callbacks
        .signal
        .as_ref()
        .map(maestro_extensions_wasm::AbortSignal::aborted);
    (callbacks.on_auth)(
        serde_json::from_value(
            json!({"url":"https://guest.invalid/ literal", "instructions":" Guest instructions "}),
        )
        .map_err(|e| e.to_string())?,
    )?;
    let answer = (callbacks.on_prompt)(
        serde_json::from_value(
            json!({"message":" Guest prompt ", "placeholder":"", "allowEmpty":false}),
        )
        .map_err(|e| e.to_string())?,
    )
    .await?;
    if let Some(progress) = &callbacks.on_progress {
        progress(" Guest progress ")?;
    }
    let manual = match &callbacks.on_manual_code_input {
        Some(manual) => Some(manual().await?),
        None => None,
    };
    let selection = match &callbacks.on_select {
        Some(select) => select(serde_json::from_value(json!({"message":" Guest selection ","options":[{"id":"repeated","label":" First "},{"id":"repeated","label":" Second "}]})).map_err(|e| e.to_string())?).await?,
        None => None,
    };
    let mut credentials = super::registration::credentials(&answer)?;
    credentials
        .extra
        .insert("initialAborted".into(), json!(initial_aborted));
    credentials.extra.insert("presence".into(), json!(presence));
    credentials.extra.insert("manual".into(), json!(manual));
    credentials
        .extra
        .insert("selection".into(), json!(selection));
    credentials.extra.insert(
        "aborted".into(),
        json!(
            callbacks
                .signal
                .as_ref()
                .map(maestro_extensions_wasm::AbortSignal::aborted)
        ),
    );
    Ok(credentials)
}

/// Observes retained interaction without reacquiring its original context or provider.
pub(crate) async fn run(api: &ExtensionAPI, operation: &str) -> ExtensionResult<Value> {
    match operation {
        "install" => install(api),
        "inspect" => {
            let callbacks = RETAINED.with(|slot| slot.borrow().clone());
            Ok(json!([
                callbacks.is_some(),
                callbacks.and_then(|callbacks| callbacks
                    .signal
                    .as_ref()
                    .map(maestro_extensions_wasm::AbortSignal::aborted))
            ]))
        }
        "pending-create" => {
            let callbacks = RETAINED
                .with(|slot| slot.borrow_mut().take())
                .ok_or("missing retained native callbacks")?;
            let prompt = serde_json::from_value(json!({"message":"Retained prompt"}))
                .map_err(|e| e.to_string())?;
            let input = (callbacks.on_prompt)(prompt);
            INPUT.with(|slot| *slot.borrow_mut() = Some(input));
            Ok(json!(true))
        }
        "pending-complete" => {
            let input = INPUT
                .with(|slot| slot.borrow_mut().take())
                .ok_or("missing retained native input")?;
            Ok(json!(input.await?))
        }
        "release" => Ok(json!(
            RETAINED.with(|slot| slot.borrow_mut().take().is_some())
        )),
        _ => Err("unknown reverse OAuth request".into()),
    }
}

/// Installs interaction on the original bound facade rather than a stand-in host.
fn install(api: &ExtensionAPI) -> ExtensionResult<Value> {
    api.register_provider(
        "guest-reverse",
        ProviderConfig {
            callbacks: ProviderCallbacks {
                oauth: Some(Rc::new(Provider)),
                ..ProviderCallbacks::default()
            },
            ..ProviderConfig::default()
        },
    )?;
    Ok(json!(true))
}

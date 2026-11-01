//! Real catalog consumers reached through the action runtime.
#![cfg(test)]
use crate::extension_actions as tests;
use maestro_extensions::{ExtensionExecution, create_extension_runtime, types::ProviderActions};
use maestro_models::*;
use std::{
    borrow::Cow,
    sync::{Arc, Mutex, Weak},
};

#[path = "../../../maestro-models/tests/support/original_options.rs"]
pub(super) mod original_options;

/// Account operations with observable reentrant transformation and deterministic tokens.
struct Account {
    catalog: Weak<maestro_catalog::ModelRegistry>,
    throws: bool,
    events: Arc<Mutex<Vec<&'static str>>>,
}
fn tokens() -> OAuthCredentials {
    OAuthCredentials {
        access: "access".into(),
        refresh: "refresh".into(),
        expires: f64::MAX,
        extra: JsonObject::new(),
    }
}
impl OAuthProviderInterface for Account {
    fn id(&self) -> &'static str {
        "uninstalled"
    }
    fn name(&self) -> &'static str {
        "Account label"
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.events.lock().unwrap().push("login");
        Box::pin(async { Ok(tokens()) })
    }
    fn refresh_token(
        &self,
        mut credentials: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        self.events.lock().unwrap().push("refresh");
        credentials.access = "refreshed".into();
        Box::pin(async move { Ok(credentials) })
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a OAuthCredentials,
    ) -> Result<Cow<'a, str>, OAuthError> {
        self.events.lock().unwrap().push("key");
        Ok(Cow::Borrowed(&credentials.access))
    }
    fn modify_models<'m>(
        &self,
        models: &'m [Model],
        _: &OAuthCredentials,
    ) -> Option<Result<Vec<Cow<'m, Model>>, OAuthError>> {
        self.events.lock().unwrap().push("transform");
        let registry = self.catalog.upgrade().unwrap();
        assert!(registry.find("outer", "outer-model").is_some());
        registry
            .register_provider("nested", tests::good("nested-model"))
            .unwrap();
        assert!(registry.find("nested", "nested-model").is_some());
        Some(if self.throws {
            Err(DiagnosticErrorInfo::error("transform refused").into())
        } else {
            Ok(models.iter().map(Cow::Borrowed).collect())
        })
    }
}
pub fn catalog_callbacks_can_read_and_register_during_a_bound_registration() {
    for modes in [(false, false), (true, false), (false, true), (true, true)] {
        reentry_case(modes);
    }
}

/// Login callbacks that the deterministic account never needs.
struct NoInteraction;
impl OAuthLoginCallbacks for NoInteraction {
    fn on_auth(&self, _: OAuthAuthInfo) -> Result<(), OAuthError> {
        panic!("no authorization requested")
    }
    fn on_prompt(&self, _: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        panic!("no prompt requested")
    }
}
/// Supply every retained provider capability through the actual catalog record.
fn consumer_config(
    registry: &Arc<maestro_catalog::ModelRegistry>,
    original: &Arc<original_options::OptionsProvider>,
    events: &Arc<Mutex<Vec<&'static str>>>,
) -> SharedHandle<std::sync::RwLock<maestro_extensions::ProviderConfig>> {
    let config = tests::good("m");
    {
        let mut input = config.write().unwrap();
        input.name = Some(String::new());
        input.api = Some("extension-original".into());
        let model = &mut input.models.as_mut().unwrap()[0];
        model.name = " raw model ".into();
        model.base_url = Some("https://model.invalid/endpoint".into());
        model.api = Some("extension-original".into());
        model.reasoning = true;
        model.thinking_level_map =
            Some(serde_json::from_value(serde_json::json!({"low":null,"high":"deep"})).unwrap());
        model.input = vec![ModelInput::Image, ModelInput::Text];
        model.cost = ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 3.0,
            cache_write: 4.0,
        };
        model.compat = Some(ModelCompat(
            serde_json::from_value(serde_json::json!({"custom":false})).unwrap(),
        ));
        let mut override_model = input.models.as_ref().unwrap()[0].clone();
        override_model.id = "override".into();
        override_model.api = Some("model-protocol".into());
        input.models.as_mut().unwrap().push(override_model);
        input.api_key = Some("literal-key".into());
        input.headers = Some(
            serde_json::from_value(
                serde_json::json!({"X-Provider":"provider","X-Order":"provider"}),
            )
            .unwrap(),
        );
        input.auth_header = Some(true);
        input.models.as_mut().unwrap()[0].headers = Some(
            serde_json::from_value(serde_json::json!({"X-Model":"model","X-Order":"model"}))
                .unwrap(),
        );
        input.callbacks.stream_simple = Some(original.clone());
        input.callbacks.oauth = Some(Arc::new(Account {
            catalog: Arc::downgrade(registry),
            throws: false,
            events: events.clone(),
        }));
    }
    config
}

pub fn bound_provider_configuration_reaches_stream_oauth_and_request_consumers() {
    let runtime = create_extension_runtime::<Local>();
    let registry = tests::catalog();
    let runner = tests::runner_local(runtime.clone(), registry.clone());
    let original = Arc::new(original_options::OptionsProvider::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let config = consumer_config(&registry, &original, &events);
    let (_owner, api) = tests::local_api(runtime, "/work");
    api.register_provider("p".into(), config.clone()).unwrap();
    runner
        .bind_core(
            Local::unbound_actions(),
            maestro_extensions::ExtensionContextActions::default(),
            ProviderActions::default(),
        )
        .unwrap();
    let model = registry.find("p", "m").unwrap();
    assert_descriptor(&model);
    assert_eq!(
        registry.find("p", "override").unwrap().read().unwrap().api,
        "model-protocol"
    );
    assert_eq!(registry.get_provider_display_name("p"), "");
    let provider = get_api_provider("extension-original").unwrap();
    let account = get_oauth_provider("p").unwrap();
    assert_eq!(account.id(), "p");
    assert_eq!(account.name(), "Account label");
    let executor = crate::runtime_support::runtime();
    let stream_model = model.read().unwrap().clone();
    executor.block_on(async {
        exercise_account(&account).await;
        assert_request(&registry, &model).await;
        tokio::task::LocalSet::new()
            .run_until(original_options::exercise(
                &provider,
                &stream_model,
                &original,
            ))
            .await;
    });
    assert_eq!(*events.lock().unwrap(), ["login", "refresh", "key"]);
    request_variants(&registry, &config);
    registry.refresh().unwrap();
    assert_descriptor(&registry.find("p", "m").unwrap());
    registry.unregister_provider("p").unwrap();
    assert!(get_api_provider("extension-original").is_none());
    assert!(get_oauth_provider("p").is_none());
    config.write().unwrap().callbacks.stream_simple = None;
    let weak = Arc::downgrade(&original);
    drop(original);
    executor.block_on(
        tokio::task::LocalSet::new().run_until(original_options::exercise(
            &provider,
            &stream_model,
            &weak.upgrade().unwrap(),
        )),
    );
    drop(provider);
    assert!(weak.upgrade().is_none());
}

/// Resolve the configured key and all header layers at their real consumer.
async fn assert_request(
    registry: &maestro_catalog::ModelRegistry,
    model: &Arc<std::sync::RwLock<Model>>,
) {
    match registry.get_api_key_and_headers(model).await {
        maestro_catalog::ResolvedRequestAuth::Ok {
            api_key, headers, ..
        } => {
            assert!(matches!(api_key.as_deref(), Some("literal-key")));
            let headers = headers.unwrap();
            assert!(matches!(headers["X-Order"].as_str(), "model"));
            assert!(matches!(
                headers["Authorization"].as_str(),
                "Bearer literal-key"
            ));
            assert_eq!(
                headers.keys().map(String::as_str).collect::<Vec<_>>(),
                ["X-Provider", "X-Order", "X-Model", "Authorization"]
            );
        }
        maestro_catalog::ResolvedRequestAuth::Error { .. } => panic!("request must resolve"),
    }
}

fn reentry_case((throws, queued): (bool, bool)) {
    let runtime = create_extension_runtime::<Local>();
    let registry = tests::catalog();
    registry.get_all().write().unwrap().clear();
    registry.auth_storage().set(
        "outer",
        maestro_credentials::AuthCredential::OAuth(tokens()),
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let config = tests::good("outer-model");
    config.write().unwrap().callbacks.oauth = Some(Arc::new(Account {
        catalog: Arc::downgrade(&registry),
        throws,
        events: events.clone(),
    }));
    let runner = tests::runner_local(runtime.clone(), registry.clone());
    if !queued {
        tests::bind_empty(&runner, ProviderActions::default()).unwrap();
    }
    let (_owner, api) = tests::local_api(runtime, "/work");
    assert!(SharedHandle::ptr_eq(
        &runner.create_context().model_registry().unwrap(),
        &registry
    ));
    let reported = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = reported.clone();
    let off = runner.on_error(std::rc::Rc::new(move |error| {
        seen.borrow_mut().push(error.error.clone());
        Ok(())
    }));
    let result = if queued {
        api.register_provider("outer".into(), config).unwrap();
        tests::bind_empty(&runner, ProviderActions::default())
    } else {
        api.register_provider("outer".into(), config)
    };
    assert_eq!(
        result.err().map(|e| e.to_string()),
        (throws && !queued).then(|| "transform refused".into())
    );
    assert_eq!(
        *reported.borrow(),
        if throws && queued {
            vec!["transform refused"]
        } else {
            vec![]
        }
    );
    off();
    assert!(registry.find("outer", "outer-model").is_some());
    assert_eq!(registry.find("nested", "nested-model").is_some(), throws);
    assert_eq!(*events.lock().unwrap(), ["transform"]);
    registry.unregister_provider("nested").unwrap();
    assert_eq!(registry.find("outer", "outer-model").is_some(), !throws);
    reset_oauth_providers();
}

fn assert_descriptor(model: &Arc<std::sync::RwLock<Model>>) {
    let observed = model.read().unwrap();
    assert_eq!(
        serde_json::to_value(&*observed).unwrap(),
        serde_json::json!({
            "id":"m", "name":" raw model ", "api":"extension-original", "provider":"p", "baseUrl":"https://model.invalid/endpoint",
            "reasoning":true, "thinkingLevelMap":{"low":null,"high":"deep"}, "input":["image","text"],
            "cost":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0}, "contextWindow":128_000.0,"maxTokens":4096.0,"compat":{"custom":false}
        })
    );
}

fn request_variants(
    registry: &maestro_catalog::ModelRegistry,
    config: &SharedHandle<std::sync::RwLock<maestro_extensions::ProviderConfig>>,
) {
    for (auth, maps) in [(false, false), (true, false), (false, true), (true, true)] {
        {
            let mut supplied = config.write().unwrap();
            supplied.auth_header = Some(auth);
            supplied.headers = maps.then(Default::default);
            supplied.models.as_mut().unwrap()[0].headers = maps.then(Default::default);
        }
        registry.register_provider("p", config.clone()).unwrap();
        let model = registry.find("p", "m").unwrap();
        match crate::runtime_support::runtime().block_on(registry.get_api_key_and_headers(&model)) {
            maestro_catalog::ResolvedRequestAuth::Ok {
                api_key, headers, ..
            } => {
                assert!(matches!(api_key.as_deref(), Some("literal-key")));
                match headers {
                    Some(headers) => {
                        assert!(auth);
                        assert_eq!(headers.len(), 1);
                        assert!(matches!(
                            headers.get("Authorization").map(String::as_str),
                            Some("Bearer literal-key")
                        ));
                    }
                    None => assert!(!auth),
                }
            }
            maestro_catalog::ResolvedRequestAuth::Error { .. } => panic!("request must resolve"),
        }
    }
}

/// Exercises installed account operations without printing credential values.
async fn exercise_account(account: &OAuthProviderHandle) {
    let credentials = account.login(Arc::new(NoInteraction), None).await.unwrap();
    let credentials = account.refresh_token(credentials, None).await.unwrap();
    assert!(matches!(
        account.get_api_key(&credentials).unwrap().as_ref(),
        "refreshed"
    ));
}

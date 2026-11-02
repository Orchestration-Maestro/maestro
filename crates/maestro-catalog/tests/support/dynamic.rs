//! Typed dynamic definitions and subprocess isolation.
#[cfg(not(target_arch = "wasm32"))]
mod native {
    pub use crate::isolation::isolated;
    use maestro_catalog::{ProviderConfigInput, ProviderModelInput};
    use maestro_models::{ModelCost, ModelInput, SharedHandle};
    use std::sync::{Arc, RwLock};
    /// Complete definition with distinguishable supplied metadata.
    #[must_use]
    pub fn model(id: &str) -> ProviderModelInput {
        ProviderModelInput {
            id: id.into(),
            name: format!("Name {id}"),
            api: None,
            base_url: None,
            reasoning: false,
            thinking_level_map: None,
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 1.0,
                output: 2.0,
                cache_read: 3.0,
                cache_write: 4.0,
            },
            context_window: 12345.0,
            max_tokens: 678.0,
            headers: None,
            compat: None,
        }
    }
    /// Valid provider input over supplied definitions.
    #[must_use]
    pub fn config(ids: &[&str]) -> SharedHandle<RwLock<ProviderConfigInput>> {
        SharedHandle::new(RwLock::new(ProviderConfigInput {
            api: Some("openai-completions".into()),
            base_url: Some("https://dynamic.invalid/v1".into()),
            api_key: Some("configured".into()),
            models: Some(ids.iter().map(|id| model(id)).collect()),
            ..Default::default()
        }))
    }
    /// Controlled model transformation callback.
    pub type Transform = dyn for<'m> Fn(
            Vec<std::borrow::Cow<'m, maestro_models::Model>>,
        ) -> Result<
            Vec<std::borrow::Cow<'m, maestro_models::Model>>,
            maestro_models::OAuthError,
        > + Send
        + Sync;
    /// Controlled account implementation with an optional model transform.
    pub struct OAuth {
        /// Optional operation observations for delegation tests.
        pub calls: Option<Arc<std::sync::Mutex<Vec<&'static str>>>>,
        /// Original account identifier.
        pub id: &'static str,
        /// Display label.
        pub name: &'static str,
        /// Supplied value transformation.
        pub transform: Arc<Transform>,
    }
    impl maestro_models::OAuthProviderInterface for OAuth {
        fn id(&self) -> &str {
            self.id
        }
        fn name(&self) -> &str {
            self.name
        }
        fn uses_callback_server(&self) -> Option<bool> {
            Some(true)
        }
        fn login(
            &self,
            _: maestro_models::OAuthCallbacks,
            _: Option<maestro_models::Fetch>,
        ) -> maestro_models::BoxFuture<
            Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
        > {
            self.record("login");
            Box::pin(async { Ok(tokens()) })
        }
        fn refresh_token(
            &self,
            mut credentials: maestro_models::OAuthCredentials,
            _: Option<maestro_models::Fetch>,
        ) -> maestro_models::BoxFuture<
            Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
        > {
            self.record("refresh");
            credentials.expires = 42.0;
            Box::pin(async move { Ok(credentials) })
        }
        fn get_api_key<'a>(
            &self,
            credentials: &'a maestro_models::OAuthCredentials,
        ) -> Result<std::borrow::Cow<'a, str>, maestro_models::OAuthError> {
            self.record("key");
            Ok(std::borrow::Cow::Borrowed(&credentials.access))
        }
        fn modify_models<'m>(
            &self,
            models: &'m [maestro_models::Model],
            _: &maestro_models::OAuthCredentials,
        ) -> Option<
            Result<Vec<std::borrow::Cow<'m, maestro_models::Model>>, maestro_models::OAuthError>,
        > {
            self.record("transform");
            Some((self.transform)(
                models.iter().map(std::borrow::Cow::Borrowed).collect(),
            ))
        }
    }
    /// Complete controlled credentials.
    #[must_use]
    pub fn tokens() -> maestro_models::OAuthCredentials {
        maestro_models::OAuthCredentials {
            access: "unused-access".into(),
            refresh: "unused-refresh".into(),
            expires: 0.0,
            extra: maestro_models::JsonObject::default(),
        }
    }
    /// Account handle whose input identifier differs from registration.
    #[must_use]
    pub fn oauth(name: &'static str) -> maestro_models::OAuthProviderHandle {
        SharedHandle::new(OAuth {
            calls: None,
            id: "ignored",
            name,
            transform: Arc::new(|models| Ok(models)),
        })
    }
    /// Read dynamic descriptor identities in published order.
    /// # Panics
    /// Panics if a shared descriptor guard is poisoned.
    #[must_use]
    pub fn selected(
        registry: &maestro_catalog::ModelRegistry,
        providers: &[&str],
    ) -> Vec<(String, String, String)> {
        registry
            .get_all()
            .read()
            .unwrap()
            .iter()
            .filter_map(|model| {
                let model = model.read().unwrap();
                providers.contains(&model.provider.as_str()).then(|| {
                    (
                        model.provider.clone(),
                        model.id.clone(),
                        model.base_url.clone(),
                    )
                })
            })
            .collect()
    }
    /// Install an error callback labeling which ordered registration won.
    /// # Panics
    /// Panics if the configuration guard is poisoned.
    pub fn callback(config: &SharedHandle<RwLock<ProviderConfigInput>>, message: &'static str) {
        config.write().unwrap().callbacks.stream_simple =
            Some(SharedHandle::new(FailedProvider(message)));
    }
    /// Invoke the current protocol through the raw public callback.
    /// # Panics
    /// Panics when the configured model or protocol is absent or unexpectedly succeeds.
    #[must_use]
    pub fn failure(registry: &maestro_catalog::ModelRegistry, provider: &str, id: &str) -> String {
        let model = registry.find(provider, id).unwrap().read().unwrap().clone();
        let callback = maestro_models::get_api_provider(&model.api).unwrap();
        callback
            .stream
            .call(
                model,
                maestro_models::Context {
                    messages: vec![],
                    system_prompt: None,
                    tools: None,
                },
                maestro_models::ProviderInvocation::Threaded(None, None),
            )
            .err()
            .unwrap()
            .message
    }
    /// File effects with an observable read boundary.
    pub struct CountFile(pub Arc<std::sync::atomic::AtomicUsize>);
    impl maestro_catalog::ModelFileOperations for CountFile {
        fn exists(&self, _: &str) -> bool {
            true
        }
        fn read_to_string(&self, _: &str) -> std::io::Result<String> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok("{\"providers\":{}}".into())
        }
    }
    /// Terminal assistant value for result identity observations.
    /// # Panics
    /// Panics if the inline typed message cannot be decoded.
    #[must_use]
    pub fn terminal() -> maestro_models::SharedAssistantMessage {
        Arc::new(RwLock::new(serde_json::from_value(serde_json::json!({
            "role":"assistant", "content":[], "api":"openai-completions", "provider":"p", "model":"m", "stopReason":"stop", "timestamp":12,
            "usage":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"totalTokens":10,"cost":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4,"total":10}}
        })).unwrap()))
    }
    impl OAuth {
        /// Record a delegated operation after registration guards have been released.
        fn record(&self, name: &'static str) {
            if let Some(calls) = &self.calls {
                calls.lock().unwrap().push(name);
            }
        }
    }
    /// Interaction which must never be requested by the controlled login operation.
    pub struct NoInteraction;
    impl maestro_models::OAuthLoginCallbacks for NoInteraction {
        fn on_auth(
            &self,
            _: maestro_models::OAuthAuthInfo,
        ) -> Result<(), maestro_models::OAuthError> {
            panic!("unexpected authorization")
        }
        fn on_prompt(
            &self,
            _: maestro_models::OAuthPrompt,
        ) -> maestro_models::BoxFuture<Result<String, maestro_models::OAuthError>> {
            panic!("unexpected prompt")
        }
    }
    /// Retained invocation owner for the controlled provider.
    struct FailedProvider(&'static str);
    impl<E: maestro_models::ProviderExecution, O>
        maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for FailedProvider
    {
        type Return = Result<
            maestro_models::AssistantMessageEventStream,
            maestro_models::DiagnosticErrorInfo,
        >;
        fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
            Err(super::diagnostic(self.0))
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
/// Supplied diagnostic preserved by invocation and transformation.
#[must_use]
pub fn diagnostic(message: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo {
        message: message.into(),
        name: Some("Controlled".into()),
        stack: Some("supplied stack".into()),
        code: Some(maestro_models::DiagnosticCode::Number(7.0)),
    }
}
/// Rich account failure with two supplied nested causes.
#[must_use]
pub fn account_failure(message: &str) -> maestro_models::OAuthError {
    let root = maestro_models::DiagnosticErrorInfo {
        message: "root cause".into(),
        name: None,
        stack: None,
        code: None,
    }
    .into();
    let inner = maestro_models::OAuthError {
        diagnostic: Box::new(maestro_models::DiagnosticErrorInfo {
            message: "inner cause".into(),
            name: Some("Inner".into()),
            stack: Some("inner stack".into()),
            code: Some(maestro_models::DiagnosticCode::Text("EINNER".into())),
        }),
        errno: Some(maestro_models::DiagnosticCode::Number(5.0)),
        cause: Some(Box::new(root)),
    };
    maestro_models::OAuthError {
        diagnostic: Box::new(diagnostic(message)),
        errno: Some(maestro_models::DiagnosticCode::Number(13.0)),
        cause: Some(Box::new(inner)),
    }
}
/// Inspect the account wrapper and each supplied cause through native errors.
/// # Panics
/// Panics if any retained field or source differs.
pub fn assert_account_failure(error: &maestro_catalog::CatalogError, message: &str) {
    use std::error::Error;
    assert!(matches!(error, maestro_catalog::CatalogError::OAuth(_)));
    assert_eq!(error.to_string(), message);
    assert_eq!(error.diagnostic(), &diagnostic(message));
    let account = error
        .source()
        .unwrap()
        .downcast_ref::<maestro_models::OAuthError>()
        .unwrap();
    assert!(std::ptr::eq(
        error.diagnostic(),
        account.diagnostic.as_ref()
    ));
    assert_eq!(
        account.errno,
        Some(maestro_models::DiagnosticCode::Number(13.0))
    );
    let inner = account
        .source()
        .unwrap()
        .downcast_ref::<maestro_models::OAuthError>()
        .unwrap();
    assert_eq!(
        *inner.diagnostic,
        maestro_models::DiagnosticErrorInfo {
            message: "inner cause".into(),
            name: Some("Inner".into()),
            stack: Some("inner stack".into()),
            code: Some(maestro_models::DiagnosticCode::Text("EINNER".into())),
        }
    );
    assert_eq!(
        inner.errno,
        Some(maestro_models::DiagnosticCode::Number(5.0))
    );
    let root = inner
        .source()
        .unwrap()
        .downcast_ref::<maestro_models::OAuthError>()
        .unwrap();
    assert_eq!(
        *root.diagnostic,
        maestro_models::DiagnosticErrorInfo {
            message: "root cause".into(),
            name: None,
            stack: None,
            code: None,
        }
    );
    assert!(root.errno.is_none());
    assert!(root.source().is_none());
}

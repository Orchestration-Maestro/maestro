//! Supplied account credentials and retained registration handles in fresh processes.

use crate::{
    bundled,
    chat::{TestResult, block_on},
    child_process::{child_case, rerun},
};
use maestro_models::oauth::{OAuthCallbacks, OAuthLoginCallbacks, OAuthProviderInterface};
use maestro_models::{
    BoxFuture, Fetch, OAuthAuthInfo, OAuthCredentials, OAuthError, OAuthPrompt, get_oauth_api_key,
    register_oauth_provider,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// A custom account provider with owned refresh state and a distinguishing result.
struct Account {
    /// Replacement access text supplied by refresh.
    access: &'static str,
    /// Completed refresh operations.
    refreshes: Arc<AtomicUsize>,
}

impl OAuthProviderInterface for Account {
    fn id(&self) -> &'static str {
        "guide-account"
    }
    fn name(&self) -> &'static str {
        "Guide account"
    }
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        Ok(&credentials.access)
    }
    fn refresh_token(
        &self,
        mut credentials: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let refreshes = Arc::clone(&self.refreshes);
        credentials.access = self.access.into();
        credentials.expires = f64::MAX;
        Box::pin(async move {
            refreshes.fetch_add(1, Ordering::SeqCst);
            Ok(credentials)
        })
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async move {
            callbacks.on_auth(OAuthAuthInfo {
                url: "https://controlled.invalid/authorize".into(),
                instructions: None,
            })?;
            let answer = callbacks
                .on_prompt(OAuthPrompt {
                    message: "Paste code".into(),
                    placeholder: None,
                    allow_empty: Some(false),
                })
                .await?;
            assert_eq!(answer, "controlled-code");
            Ok(credentials(f64::MAX))
        })
    }
}

/// Caller interaction that observes both phases without opening a browser.
struct Interaction(Arc<AtomicUsize>);
impl OAuthLoginCallbacks for Interaction {
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        assert_eq!(info.url, "https://controlled.invalid/authorize");
        assert_eq!(self.0.fetch_add(1, Ordering::SeqCst), 0);
        Ok(())
    }
    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        assert_eq!(prompt.message, "Paste code");
        assert_eq!(self.0.fetch_add(1, Ordering::SeqCst), 1);
        Box::pin(async { Ok("controlled-code".into()) })
    }
}

/// Fake account data; the caller, not the library, owns persistence.
fn credentials(expires: f64) -> OAuthCredentials {
    OAuthCredentials {
        access: "supplied-access".into(),
        refresh: "supplied-refresh".into(),
        expires,
        extra: maestro_models::JsonObject::default(),
    }
}

#[test]
fn maestro_guide_refreshes_supplied_account_credentials() -> TestResult {
    if child_case().is_none() {
        return rerun(
            "maestro_guide_refreshes_supplied_account_credentials",
            "account",
            &[],
        );
    }
    block_on(false, async {
        let refreshes = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(Account {
            access: "refreshed-access",
            refreshes: Arc::clone(&refreshes),
        });
        register_oauth_provider(provider.clone());
        let phases = Arc::new(AtomicUsize::new(0));
        let login = provider
            .login(Arc::new(Interaction(Arc::clone(&phases))), None)
            .await?;
        assert_eq!(phases.load(Ordering::SeqCst), 2);
        let mut supplied = [("guide-account".into(), login)].into();
        let key = get_oauth_api_key("guide-account", &supplied, None)
            .await?
            .ok_or("key")?;
        assert!(key.api_key == "supplied-access", "nonexpired extraction");
        assert_eq!(refreshes.load(Ordering::SeqCst), 0);
        supplied.insert("guide-account".into(), credentials(0.0));
        let key = get_oauth_api_key("guide-account", &supplied, None)
            .await?
            .ok_or("refreshed key")?;
        assert!(key.api_key == "refreshed-access", "refreshed extraction");
        assert!(
            key.new_credentials.access == "refreshed-access",
            "returned updated credentials"
        );
        assert!(
            key.new_credentials.refresh == "supplied-refresh",
            "retained refresh data"
        );
        assert_eq!(key.new_credentials.expires.to_bits(), f64::MAX.to_bits());
        assert_eq!(refreshes.load(Ordering::SeqCst), 1);
        assert!(
            get_oauth_api_key("guide-account", &indexmap::IndexMap::default(), None)
                .await?
                .is_none()
        );
        Ok(())
    })
}

/// Pure registered callback, using the public message builder and event producer.
fn api_callback<O: 'static>(text: &'static str) -> maestro_models::StreamFunction<O> {
    Arc::new(move |_, _, _| {
        let message = Arc::new(std::sync::RwLock::new(
            maestro_models::faux_assistant_message(
                text,
                maestro_models::FauxAssistantMessageOptions::default(),
            ),
        ));
        let events = maestro_models::create_assistant_message_event_stream();
        events.push(maestro_models::AssistantMessageEvent::Done {
            reason: maestro_models::DoneReason::Stop,
            message,
        });
        Ok(events)
    })
}

/// Both callback forms share the same selected result, not a new executor.
fn api_provider(text: &'static str) -> maestro_models::ApiProvider {
    maestro_models::ApiProvider {
        api: "guide-api".into(),
        stream: api_callback(text),
        stream_simple: api_callback(text),
    }
}

#[test]
fn maestro_guide_registration_keeps_existing_handles() -> TestResult {
    use maestro_models::{
        clear_api_providers, get_api_provider, get_api_providers, register_api_provider,
        reset_api_providers, unregister_api_providers,
    };
    if child_case().is_none() {
        return rerun(
            "maestro_guide_registration_keeps_existing_handles",
            "registrations",
            &[],
        );
    }
    let _registry = bundled::registry();
    block_on(false, async {
        reset_api_providers();
        assert_eq!(
            get_api_providers()
                .iter()
                .map(|p| p.api.as_str())
                .collect::<Vec<_>>(),
            bundled::registered_apis()
        );
        register_api_provider(api_provider("old result"), Some("guide-old".into()));
        let old = get_api_provider("guide-api").ok_or("old callback")?;
        register_api_provider(api_provider("new result"), Some("guide-new".into()));
        let model = crate::chat::model(&serde_json::json!({"api":"guide-api"}))?;
        let current =
            maestro_models::complete(model.clone(), bundled::conversation()?, None).await?;
        assert_eq!(bundled::text_of(&current), "new result");
        assert_eq!(
            get_api_providers().last().ok_or("appended API")?.api,
            "guide-api"
        );
        unregister_api_providers("guide-old");
        assert!(get_api_provider("guide-api").is_some());
        unregister_api_providers("guide-new");
        assert!(get_api_provider("guide-api").is_none());
        clear_api_providers();
        assert!(get_api_providers().is_empty());
        let retained = (old.stream)(model, bundled::conversation()?, None)?
            .result()
            .await;
        assert_eq!(bundled::text_of(&retained), "old result");
        reset_api_providers();
        assert_eq!(
            get_api_providers()
                .iter()
                .map(|p| p.api.as_str())
                .collect::<Vec<_>>(),
            bundled::registered_apis()
        );
        check_account_handles().await?;
        Ok(())
    })
}

/// Retained account implementations survive replacement and reset.
async fn check_account_handles() -> TestResult {
    use maestro_models::{
        get_oauth_provider, get_oauth_providers, reset_oauth_providers, unregister_oauth_provider,
    };
    reset_oauth_providers();
    assert_eq!(
        get_oauth_providers()
            .iter()
            .map(|p| p.id().to_owned())
            .collect::<Vec<_>>(),
        ["anthropic", "github-copilot", "openai-codex"]
    );
    let refreshes = Arc::new(AtomicUsize::new(0));
    register_oauth_provider(Arc::new(Account {
        access: "old account",
        refreshes: Arc::clone(&refreshes),
    }));
    let old = get_oauth_provider("guide-account").ok_or("old account")?;
    register_oauth_provider(Arc::new(Account {
        access: "new account",
        refreshes,
    }));
    assert_eq!(
        get_oauth_providers().last().ok_or("last account")?.id(),
        "guide-account"
    );
    let current = get_oauth_provider("guide-account")
        .ok_or("new account")?
        .refresh_token(credentials(0.0), None)
        .await?;
    assert!(
        current.access == "new account",
        "replacement implementation"
    );
    unregister_oauth_provider("guide-account");
    assert!(get_oauth_provider("guide-account").is_none());
    reset_oauth_providers();
    let retained = old.refresh_token(credentials(0.0), None).await?;
    assert!(retained.access == "old account", "retained implementation");
    assert_eq!(
        get_oauth_providers()
            .iter()
            .map(|p| p.id().to_owned())
            .collect::<Vec<_>>(),
        ["anthropic", "github-copilot", "openai-codex"]
    );
    Ok(())
}

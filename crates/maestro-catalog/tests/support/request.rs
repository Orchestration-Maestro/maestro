//! Controlled request effects and inline source-derived observations.
use indexmap::IndexMap;
use maestro_catalog::{ModelRegistry, ResolvedRequestAuth};
use maestro_credentials::ConfigValueOperations;
use std::sync::Mutex;

/// Completed helper outcomes and the commands asked for.
#[derive(Default)]
pub struct Operations {
    pub commands: Mutex<IndexMap<String, Option<Vec<u8>>>>,
    pub calls: Mutex<Vec<String>>,
}
impl ConfigValueOperations for Operations {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        self.calls.lock().unwrap().push(command.into());
        self.commands
            .lock()
            .unwrap()
            .get(command)
            .cloned()
            .flatten()
    }
}
impl Operations {
    /// # Panics
    /// Panics when a controlled test operation or assertion fails.
    pub fn command(&self, name: &str, output: Option<&str>) {
        self.commands
            .lock()
            .unwrap()
            .insert(name.into(), output.map(|s| s.as_bytes().to_vec()));
    }
}
/// Drive a future to completion without external services.
/// # Panics
/// Panics when runtime construction fails or the supplied future panics; a future's
/// returned `Err` is passed through unchanged.
pub fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}
/// File document with a custom descriptor and caller-supplied provider fields.
#[must_use]
pub fn document(provider_fields: &str, model_fields: &str) -> String {
    format!(
        r#"{{"providers":{{"p":{{"baseUrl":"url","api":"test",{provider_fields},"models":[{{"id":"m"{model_fields}}}]}}}}}}"#
    )
}
/// Select request values through the shared model boundary.
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn request(registry: &ModelRegistry) -> maestro_request::model_registry::ResolvedRequestAuth {
    run(registry.get_api_key_and_headers(&registry.find("p", "m").unwrap()))
}
/// Assert values without including credential values in failure output.
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn success(result: ResolvedRequestAuth, key: Option<&str>, headers: &[(&str, &str)]) {
    let ResolvedRequestAuth::Ok {
        api_key,
        headers: actual,
        ..
    } = result
    else {
        panic!("expected request success")
    };
    assert!(api_key.as_deref().eq(&key), "selected key");
    if headers.is_empty() {
        assert!(actual.is_none(), "headers absent");
        return;
    }
    let actual = actual.expect("headers present");
    assert_eq!(
        actual.keys().map(String::as_str).collect::<Vec<_>>(),
        headers.iter().map(|h| h.0).collect::<Vec<_>>()
    );
    assert!(
        actual
            .values()
            .map(String::as_str)
            .eq(headers.iter().map(|h| h.1)),
        "header values"
    );
}
/// Assert the winning diagnostic without printing a request record.
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn error(result: ResolvedRequestAuth, expected: &str) {
    let ResolvedRequestAuth::Error { error } = result else {
        panic!("expected request failure")
    };
    assert_eq!(error, expected);
}
/// Controlled OAuth operations reused by request and metadata witnesses.
pub struct OAuth {
    pub id: &'static str,
    pub name: &'static str,
    pub fail_key: bool,
    pub fail_transform: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub refresh: Option<Box<Refresh>>,
}
impl OAuth {
    #[must_use]
    pub fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            id,
            name,
            fail_key: false,
            fail_transform: std::sync::Arc::default(),
            refresh: None,
        }
    }
    #[must_use]
    pub fn register(self) -> Registration {
        let id = self.id;
        maestro_models::register_oauth_provider(maestro_models::SharedHandle::new(self));
        Registration(id)
    }
}
impl maestro_models::OAuthProviderInterface for OAuth {
    fn id(&self) -> &str {
        self.id
    }
    fn name(&self) -> &str {
        self.name
    }
    fn login(
        &self,
        _: maestro_models::OAuthCallbacks,
        _: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        panic!("unexpected login")
    }
    fn refresh_token(
        &self,
        credentials: maestro_models::OAuthCredentials,
        _: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        self.refresh.as_ref().expect("unexpected refresh")(credentials)
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_models::OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, maestro_models::OAuthError> {
        if self.fail_key {
            Err(super::dynamic::account_failure("extraction failed"))
        } else {
            Ok(std::borrow::Cow::Borrowed(&credentials.access))
        }
    }
    fn modify_models<'m>(
        &self,
        models: &'m [maestro_models::Model],
        _: &maestro_models::OAuthCredentials,
    ) -> Option<Result<Vec<std::borrow::Cow<'m, maestro_models::Model>>, maestro_models::OAuthError>>
    {
        Some(
            if self
                .fail_transform
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                Err(diagnostic("transform failed"))
            } else {
                Ok(models.iter().map(std::borrow::Cow::Borrowed).collect())
            },
        )
    }
}
/// Registry cleanup before dropping any captured test state.
pub struct Registration(&'static str);
impl Drop for Registration {
    fn drop(&mut self) {
        maestro_models::unregister_oauth_provider(self.0);
    }
}
/// Complete credential with caller-selected expiry.
#[must_use]
pub fn tokens(expires: f64) -> maestro_credentials::AuthCredential {
    maestro_credentials::AuthCredential::OAuth(maestro_models::OAuthCredentials {
        refresh: "unused-refresh".into(),
        access: "unused-access".into(),
        expires,
        extra: maestro_models::JsonObject::new(),
    })
}
/// Ordinary provider error shared by controlled OAuth witnesses.
fn diagnostic(message: &str) -> maestro_models::OAuthError {
    maestro_models::DiagnosticErrorInfo {
        name: None,
        message: message.into(),
        stack: None,
        code: None,
    }
    .into()
}

/// Controlled token refresh callback.
type Refresh = dyn Fn(
        maestro_models::OAuthCredentials,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > + Send
    + Sync;

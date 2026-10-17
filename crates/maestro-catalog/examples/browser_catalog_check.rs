//! Compile-only caller-local browser catalog effects.
#[cfg(target_arch = "wasm32")]
mod browser {
    //! Compile-only browser callers with real non-Send effects.
    use maestro_catalog::{CatalogError, ModelFileOperations, ModelRegistry, ProviderConfigInput};
    use maestro_credentials::AuthStorage;
    use maestro_models::OAuthCredentials;
    use std::sync::{Arc, RwLock};
    use std::{cell::Cell, rc::Rc};
    /// Caller-local observations.
    struct Files(Rc<Cell<usize>>);
    impl ModelFileOperations for Files {
        fn exists(&self, _: &str) -> bool {
            self.0.set(self.0.get() + 1);
            false
        }
        fn read_to_string(&self, _: &str) -> std::io::Result<String> {
            Ok(String::new())
        }
    }
    /// Caller-local account callback.
    struct Account(Rc<Cell<usize>>);
    impl maestro_models::OAuthProviderInterface for Account {
        fn id(&self) -> &'static str {
            "browser"
        }
        fn name(&self) -> &'static str {
            "Browser"
        }
        fn login(
            &self,
            _: maestro_models::OAuthCallbacks,
            _: Option<maestro_models::Fetch>,
        ) -> maestro_models::BoxFuture<Result<OAuthCredentials, maestro_models::OAuthError>>
        {
            Box::pin(async { Err(maestro_models::DiagnosticErrorInfo::new(None, "unused").into()) })
        }
        fn refresh_token(
            &self,
            _: OAuthCredentials,
            _: Option<maestro_models::Fetch>,
        ) -> maestro_models::BoxFuture<Result<OAuthCredentials, maestro_models::OAuthError>>
        {
            Box::pin(async { Err(maestro_models::DiagnosticErrorInfo::new(None, "unused").into()) })
        }
        fn get_api_key<'a>(
            &self,
            credentials: &'a OAuthCredentials,
        ) -> Result<&'a str, maestro_models::OAuthError> {
            self.0.set(self.0.get() + 1);
            Ok(&credentials.access)
        }
    }
    /// Compile the existing browser family without adding native auto-trait bounds to it.
    pub fn qualify(
        auth: Arc<AuthStorage>,
        count: Rc<Cell<usize>>,
    ) -> Result<Rc<ModelRegistry>, CatalogError> {
        let registry = Rc::new(ModelRegistry::with_operations(
            auth,
            "browser-models",
            Files(count.clone()),
        )?);
        let config = Arc::from(Box::new(RwLock::new(ProviderConfigInput {
            oauth: Some(Rc::new(Account(count))),
            ..Default::default()
        })));
        registry.register_provider("browser", config)?;
        registry.refresh()?;
        registry.unregister_provider("browser")?;
        Ok(registry)
    }
}
/// Compile the browser-local call path without running a native browser harness.
#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_arch = "wasm32")]
    browser::qualify(
        std::sync::Arc::from(Box::new(maestro_credentials::AuthStorage::in_memory(
            maestro_credentials::AuthStorageData::new(),
        ))),
        std::rc::Rc::new(std::cell::Cell::new(0)),
    )?;
    Ok(())
}

/// Native builds do not run the browser-only caller.
#[cfg(not(target_arch = "wasm32"))]
fn main() {}

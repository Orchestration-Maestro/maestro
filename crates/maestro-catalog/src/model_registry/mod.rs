//! Published local model descriptors and file operations.
mod composition;
mod loading;
use maestro_credentials::{AuthCredential, AuthStorage};
use maestro_models::{Model, OAuthError, get_models, get_oauth_providers, get_providers};
use std::sync::{Arc, PoisonError, RwLock};

/// Two synchronous effects used to load a model file.
pub trait ModelFileOperations {
    /// Test whether the supplied path exists.
    fn exists(&self, path: &str) -> bool;
    /// Read model-file text.
    ///
    /// # Errors
    /// Returns a file read failure.
    fn read_to_string(&self, path: &str) -> std::io::Result<String>;
}
/// Native file operations with replacement decoding for invalid UTF-8.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeModelFileOperations;
#[cfg(not(target_arch = "wasm32"))]
impl ModelFileOperations for NativeModelFileOperations {
    fn exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    fn read_to_string(&self, path: &str) -> std::io::Result<String> {
        std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }
}
/// Shared mutable descriptor list published by the registry.
type ModelList = Arc<RwLock<Vec<Arc<RwLock<Model>>>>>;
/// Local catalog with a retained load diagnostic.
pub struct ModelRegistry {
    /// Accepted credential owner.
    auth_storage: Arc<AuthStorage>,
    /// Explicit model-file path; empty disables file access.
    path: String,
    /// Caller-selected file effects.
    operations: Box<dyn ModelFileOperations>,
    /// Currently published list.
    models: ModelList,
    /// Current file failure, separate from OAuth failures.
    load_error: Option<String>,
}
impl ModelRegistry {
    /// Load immediately from the explicit native file path.
    ///
    /// # Errors
    /// Returns an OAuth transformation failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create(
        auth_storage: Arc<AuthStorage>,
        models_json_path: impl Into<String>,
    ) -> Result<Self, OAuthError> {
        Self::with_operations(auth_storage, models_json_path, NativeModelFileOperations)
    }
    /// Construct an offline catalog without file access.
    ///
    /// # Errors
    /// Returns an OAuth transformation failure.
    pub fn in_memory(auth_storage: Arc<AuthStorage>) -> Result<Self, OAuthError> {
        Self::with_operations(auth_storage, "", MemoryOperations)
    }
    /// Load using caller-selected synchronous file effects.
    ///
    /// # Errors
    /// Returns an OAuth transformation failure.
    pub fn with_operations(
        auth_storage: Arc<AuthStorage>,
        models_json_path: impl Into<String>,
        operations: impl ModelFileOperations + 'static,
    ) -> Result<Self, OAuthError> {
        let mut registry = Self {
            auth_storage,
            path: models_json_path.into(),
            operations: Box::new(operations),
            models: Arc::default(),
            load_error: None,
        };
        registry.refresh()?;
        Ok(registry)
    }
    /// Borrow the credential owner supplied at construction.
    #[must_use]
    pub fn auth_storage(&self) -> &Arc<AuthStorage> {
        &self.auth_storage
    }
    /// Reload the file, clearing its previous diagnostic before OAuth transforms.
    ///
    /// # Errors
    /// Returns a transformation failure without replacing the published list.
    pub fn refresh(&mut self) -> Result<(), OAuthError> {
        self.load_error = None;
        let config = loading::load(&self.path, self.operations.as_ref());
        let mut models: Vec<Model> = get_providers().iter().flat_map(|p| get_models(p)).collect();
        match config {
            Ok(Some(providers)) => models = composition::compose(models, providers),
            Ok(None) => {}
            Err(error) => self.load_error = Some(error),
        }
        for provider in get_oauth_providers() {
            if let Some(AuthCredential::OAuth(credentials)) = self.auth_storage.get(provider.id()) {
                models = provider.modify_models(models, &credentials)?;
            }
        }
        self.models = Arc::new(RwLock::new(
            models
                .into_iter()
                .map(|m| Arc::new(RwLock::new(m)))
                .collect(),
        ));
        Ok(())
    }
    /// Borrow the current file-load diagnostic.
    #[must_use]
    pub fn get_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }
    /// Share the current mutable list; reload publishes a different list.
    #[must_use]
    pub fn get_all(&self) -> ModelList {
        Arc::clone(&self.models)
    }
    /// Share the first descriptor matching the exact provider and model ID.
    #[must_use]
    pub fn find(&self, provider: &str, model_id: &str) -> Option<Arc<RwLock<Model>>> {
        self.models
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|handle| {
                let model = handle.read().unwrap_or_else(PoisonError::into_inner);
                model.provider == provider && model.id == model_id
            })
            .cloned()
    }
}
/// File effects unused by memory construction.
struct MemoryOperations;
impl ModelFileOperations for MemoryOperations {
    fn exists(&self, _: &str) -> bool {
        false
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        Ok(String::new())
    }
}

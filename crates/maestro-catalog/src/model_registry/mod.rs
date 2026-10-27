//! Published local model descriptors and file operations.
mod auth_metadata;
mod composition;
mod dynamic_providers;
mod error;
pub use error::CatalogError;
mod loading;
mod provider_config;
pub use provider_config::{ProviderCallbacks, ProviderConfigInput, ProviderModelInput};
use std::borrow::Cow;
mod request_auth;
use indexmap::IndexMap;
use maestro_credentials::{AuthCredential, AuthStorage, ConfigValueOperations};
use maestro_models::{
    Model, OAuthCredentials, OAuthProviderHandle, SharedHandle, get_models, get_oauth_providers,
    get_providers,
};
use request_auth::ProviderRequestConfig;
pub use request_auth::ResolvedRequestAuth;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// Native file effects may be shared across threads.
#[cfg(not(target_arch = "wasm32"))]
pub trait ThreadBound: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> ThreadBound for T {}
/// Local browser effects carry no thread-safety bound.
#[cfg(target_arch = "wasm32")]
pub trait ThreadBound {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> ThreadBound for T {}
/// Synchronous model-file effects with native thread-safe ownership.
pub trait ModelFileOperations: ThreadBound {
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
/// Runs the commands that configured credential values name.
#[cfg(not(target_arch = "wasm32"))]
pub type ConfigValues = SharedHandle<dyn ConfigValueOperations + Send + Sync>;
/// Runs the commands that configured credential values name.
#[cfg(target_arch = "wasm32")]
pub type ConfigValues = SharedHandle<dyn ConfigValueOperations>;
/// Shared mutable descriptor list published by the registry.
type ModelList = Arc<RwLock<Vec<Arc<RwLock<Model>>>>>;
/// Local catalog with a retained load diagnostic.
pub struct ModelRegistry {
    /// Accepted credential owner, shared natively and local in the browser.
    auth_storage: SharedHandle<AuthStorage>,
    /// Ordered model-file paths; empty entries disable file access for that entry.
    paths: Vec<String>,
    /// Base files ranked before the construction-time files, replaced at resource reload.
    base_paths: RwLock<Vec<String>>,
    /// Caller-selected file effects.
    operations: Box<dyn ModelFileOperations>,
    /// Runs the commands that configured credentials name.
    config_values: ConfigValues,
    /// Privately synchronized published and retained catalog state.
    state: RwLock<CatalogState>,
}
/// Mutable fields sampled at their consuming catalog phase.
struct CatalogState {
    /// Current shared list.
    models: ModelList,
    /// Owned file-load diagnostic.
    load_error: Option<String>,
    /// Provider sources retained by entry identity.
    provider_configs: HashMap<String, Arc<ProviderRequestConfig>>,
    /// Live registration order with monotone visit identities.
    registered_providers: IndexMap<String, (u64, SharedHandle<RwLock<ProviderConfigInput>>)>,
    /// Next registration identity.
    next_provider: u64,
    /// Model request sources indexed by exact identity.
    model_headers: HashMap<(String, String), IndexMap<String, String>>,
}

impl ModelRegistry {
    /// Load immediately from the explicit native file path, keeping
    /// `config_values` for configured credential commands.
    ///
    /// # Errors
    /// Returns a catalog error wrapping an account transformation failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create(
        auth_storage: Arc<AuthStorage>,
        models_json_path: impl Into<String>,
        config_values: ConfigValues,
    ) -> Result<Self, CatalogError> {
        Self::with_operations(
            auth_storage,
            models_json_path,
            NativeModelFileOperations,
            config_values,
        )
    }
    /// Construct an offline catalog without file access, keeping `config_values`
    /// for configured credential commands.
    ///
    /// The credential owner is the platform handle, so a browser caller supplies its
    /// local handle while a native caller supplies the shared one.
    ///
    /// # Errors
    /// Returns a catalog error wrapping an account transformation failure.
    pub fn in_memory(
        auth_storage: SharedHandle<AuthStorage>,
        config_values: ConfigValues,
    ) -> Result<Self, CatalogError> {
        Self::with_operations(auth_storage, "", MemoryOperations, config_values)
    }
    /// Load using the supplied file effects and credential command handle.
    ///
    /// The credential owner is the platform handle, so a browser caller supplies its
    /// local handle while a native caller supplies the shared one.
    ///
    /// # Errors
    /// Returns a catalog error wrapping an account transformation failure.
    pub fn with_operations(
        auth_storage: SharedHandle<AuthStorage>,
        models_json_path: impl Into<String>,
        operations: impl ModelFileOperations + 'static,
        config_values: ConfigValues,
    ) -> Result<Self, CatalogError> {
        Self::with_model_files(
            auth_storage,
            vec![models_json_path.into()],
            operations,
            config_values,
        )
    }
    /// Loads ordered model files with later fields overriding earlier ones.
    ///
    /// # Errors
    /// Returns a catalog error wrapping an account transformation failure.
    pub fn with_model_files(
        auth_storage: SharedHandle<AuthStorage>,
        paths: Vec<String>,
        operations: impl ModelFileOperations + 'static,
        config_values: ConfigValues,
    ) -> Result<Self, CatalogError> {
        let registry = Self {
            auth_storage,
            paths,
            base_paths: RwLock::default(),
            operations: Box::new(operations),
            config_values,
            state: RwLock::new(CatalogState {
                models: Arc::default(),
                load_error: None,
                provider_configs: HashMap::new(),
                registered_providers: IndexMap::new(),
                next_provider: 0,
                model_headers: HashMap::new(),
            }),
        };
        registry.load_models()?;
        Ok(registry)
    }
    /// Replaces the base files below the construction-time files and restores dynamic providers.
    /// An empty base that was already empty leaves the existing catalog untouched.
    ///
    /// # Errors
    /// Returns a stored account transformation failure during refresh.
    pub fn set_base_model_files(&self, paths: Vec<String>) -> Result<(), CatalogError> {
        let reload = {
            let mut base = self
                .base_paths
                .write()
                .unwrap_or_else(PoisonError::into_inner);
            let reload = !base.is_empty() || !paths.is_empty();
            *base = paths;
            reload
        };
        if reload {
            self.refresh()?;
        }
        Ok(())
    }

    /// Validate and apply the incoming record, then merge defined fields into retained state.
    ///
    /// The configuration is the platform handle, retained as the browser's local owner or
    /// the native shared one.
    ///
    /// # Errors
    /// Returns the first missing prerequisite or supplied model transformation failure.
    pub fn register_provider(
        &self,
        provider_name: &str,
        config: SharedHandle<RwLock<ProviderConfigInput>>,
    ) -> Result<(), CatalogError> {
        provider_config::validate(
            &config.read().unwrap_or_else(PoisonError::into_inner),
            provider_name,
        )?;
        self.apply_provider_config(provider_name, &config)?;
        self.upsert_registered_provider(provider_name, config);
        Ok(())
    }
    /// Borrows the credential-command effects supplied at construction.
    #[must_use]
    pub fn config_values(&self) -> &ConfigValues {
        &self.config_values
    }
    /// Borrow the platform credential owner supplied at construction.
    #[must_use]
    pub fn auth_storage(&self) -> &SharedHandle<AuthStorage> {
        &self.auth_storage
    }
    /// Reset shared protocol/account registrations, load the file and replay retained providers.
    ///
    /// # Errors
    /// Returns the first transformation failure, retaining effects already applied.
    pub fn refresh(&self) -> Result<(), CatalogError> {
        {
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            state.load_error = None;
            state.provider_configs.clear();
            state.model_headers.clear();
        }
        maestro_models::reset_api_providers();
        maestro_models::reset_oauth_providers();
        self.load_models()?;
        let mut cursor = 0;
        loop {
            let next = self.next_registered(cursor);
            let Some((name, ordinal, config)) = next else {
                break;
            };
            cursor = ordinal + 1;
            self.apply_provider_config(&name, &config)?;
        }
        Ok(())
    }
    /// Load underlying descriptors and run stored account transformations before publication.
    ///
    /// # Errors
    /// Returns a transformation failure without replacing the published list.
    fn load_models(&self) -> Result<(), CatalogError> {
        let mut handles = self.prepare_load();
        for provider in get_oauth_providers() {
            if let Some(AuthCredential::OAuth(credentials)) = self.auth_storage.get(provider.id()) {
                handles = transform_handles(handles, &provider, &credentials)?;
            }
        }
        self.publish(handles);
        Ok(())
    }
    /// Prepare descriptors after supplied file effects return.
    fn prepare_load(&self) -> Vec<Arc<RwLock<Model>>> {
        let mut paths = self
            .base_paths
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        paths.extend(self.paths.iter().cloned());
        let config = loading::load(&paths, self.operations.as_ref());
        let mut models: Vec<Model> = get_providers().iter().flat_map(|p| get_models(p)).collect();
        {
            let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
            match config {
                Ok(Some(mut providers)) => {
                    state.retain_request_sources(&mut providers);
                    models = composition::compose(models, providers, &mut state.model_headers);
                }
                Ok(None) => {}
                Err(error) => state.load_error = Some(error),
            }
        }
        models
            .into_iter()
            .map(|m| Arc::new(RwLock::new(m)))
            .collect()
    }
    /// Copy the current file-load diagnostic into an owned observation.
    #[must_use]
    pub fn get_error(&self) -> Option<String> {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .load_error
            .clone()
    }
    /// Share the current mutable list; reload publishes a different list.
    #[must_use]
    pub fn get_all(&self) -> ModelList {
        Arc::clone(
            &self
                .state
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .models,
        )
    }
    /// Share the first descriptor matching the exact provider and model ID.
    #[must_use]
    pub fn find(&self, provider: &str, model_id: &str) -> Option<Arc<RwLock<Model>>> {
        self.get_all()
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
impl ModelRegistry {
    /// Replace the list and retire the previous owner outside exclusion.
    fn publish(&self, handles: Vec<Arc<RwLock<Model>>>) {
        let retired = std::mem::replace(
            &mut self
                .state
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .models,
            Arc::new(RwLock::new(handles)),
        );
        drop(retired);
    }
    /// Capture the next live registration without retaining exclusion.
    fn next_registered(
        &self,
        cursor: u64,
    ) -> Option<(String, u64, SharedHandle<RwLock<ProviderConfigInput>>)> {
        let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
        let index = state
            .registered_providers
            .as_slice()
            .partition_point(|_, (ordinal, _)| *ordinal < cursor);
        state
            .registered_providers
            .get_index(index)
            .map(|(name, (ordinal, config))| (name.clone(), *ordinal, SharedHandle::clone(config)))
    }
    /// Retain the source entry identity for a later consuming phase.
    fn request_config(&self, provider: &str) -> Option<Arc<ProviderRequestConfig>> {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .provider_configs
            .get(provider)
            .cloned()
    }
}
impl CatalogState {
    /// Move checked request sources before descriptor composition.
    fn retain_request_sources(&mut self, providers: &mut [(String, composition::ProviderConfig)]) {
        for (provider, config) in providers {
            self.provider_configs.insert(
                provider.clone(),
                Arc::new(ProviderRequestConfig {
                    api_key: config.api_key.take(),
                    headers: composition::ordered(config.headers.take()),
                    auth_header: config.auth_header,
                }),
            );
            self.retain_model_headers(provider, &mut config.model_overrides);
        }
    }
    /// Move overrides before custom definitions replace or remove their sources.
    fn retain_model_headers(
        &mut self,
        provider: &str,
        overrides: &mut HashMap<String, composition::ModelOverride>,
    ) {
        for (id, fields) in overrides {
            if let Some(headers) = composition::ordered(fields.headers.take()) {
                self.model_headers
                    .insert((provider.to_owned(), id.clone()), headers);
            }
        }
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

/// Copy only the provider operand before calling authentication effects.
fn read_provider(model: &Arc<RwLock<Model>>) -> String {
    model
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .provider
        .clone()
}

/// Project borrowed provenance onto current handles without holding descriptor guards.
fn transform_handles(
    handles: Vec<Arc<RwLock<Model>>>,
    oauth: &OAuthProviderHandle,
    credentials: &OAuthCredentials,
) -> Result<Vec<Arc<RwLock<Model>>>, CatalogError> {
    let models: Vec<_> = handles
        .iter()
        .map(|model| model.read().unwrap_or_else(PoisonError::into_inner).clone())
        .collect();
    let Some(transformed) = oauth.modify_models(&models, credentials) else {
        return Ok(handles);
    };
    let mut foreign = HashMap::new();
    Ok(transformed?
        .into_iter()
        .map(|model| match model {
            Cow::Borrowed(original) => match models.element_offset(original) {
                Some(index) => Arc::clone(&handles[index]),
                None => foreign
                    .entry(std::ptr::from_ref(original))
                    .or_insert_with(|| Arc::new(RwLock::new(original.clone())))
                    .clone(),
            },
            Cow::Owned(model) => Arc::new(RwLock::new(model)),
        })
        .collect())
}

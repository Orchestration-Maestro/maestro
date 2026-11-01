//! Application-supplied defaults used only when an author omits a file path.
use crate::types::{ExtensionAPI, ExtensionRuntime, read, write};
#[cfg(not(target_arch = "wasm32"))]
use crate::{AuthStorage, ModelRegistry, SharedHandle};
use crate::{ConfigValues, ExtensionExecution};
use std::sync::Arc;

/// Lazy file defaults and credential-command effects available during factory loading.
#[derive(Clone)]
pub struct CatalogConstruction {
    /// Resolves the application's credential path only for a file constructor without a path.
    pub auth_path: Arc<dyn Fn() -> Result<String, String> + Send + Sync>,
    /// Resolves the application's model path only for a file constructor without a path.
    pub models_path: Arc<dyn Fn() -> Result<String, String> + Send + Sync>,
    /// Existing command effects retained by newly constructed owners.
    pub config_values: ConfigValues,
}

impl CatalogConstruction {
    /// Selects the explicit model path without evaluating the application's default.
    ///
    /// # Errors
    /// Returns the application's default-path resolution failure.
    pub fn model_path(&self, path: Option<&str>) -> Result<String, String> {
        path.map_or_else(|| (self.models_path)(), |path| Ok(path.to_owned()))
    }
}

impl<E: ExtensionExecution> ExtensionRuntime<E> {
    /// Supplies constructor defaults before any factory runs.
    pub fn set_catalog_construction(&self, construction: CatalogConstruction) {
        let old = write(&self.construction).replace(construction);
        drop(old);
    }
}

impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Reads the supplied construction services without retaining the runtime guard.
    ///
    /// # Errors
    /// Returns an error if the application did not supply construction services.
    pub fn catalog_construction(&self) -> Result<CatalogConstruction, String> {
        self.owner().map_err(|error| error.to_string())?;
        read(&self.runtime.construction)
            .clone()
            .ok_or_else(|| "Extension catalog construction is not configured".to_owned())
    }

    /// Constructs file credentials using an explicit path or the application's lazy default.
    ///
    /// # Errors
    /// Returns a missing service or default-path resolution failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create_auth_storage(
        &self,
        path: Option<&str>,
    ) -> Result<SharedHandle<AuthStorage>, String> {
        self.owner().map_err(|error| error.to_string())?;
        let path = match path {
            Some(path) => path.to_owned(),
            None => (self.catalog_construction()?.auth_path)()?,
        };
        Ok(SharedHandle::new(AuthStorage::create(&path)))
    }

    /// Constructs a file catalog using the supplied credential owner, without copying it.
    ///
    /// # Errors
    /// Returns a missing service, default-path resolution or native catalog failure.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn create_model_registry(
        &self,
        auth: SharedHandle<AuthStorage>,
        path: Option<&str>,
    ) -> Result<SharedHandle<ModelRegistry>, String> {
        let services = self.catalog_construction()?;
        let path = services.model_path(path)?;
        ModelRegistry::create(auth, path, services.config_values)
            .map(SharedHandle::new)
            .map_err(|error| error.to_string())
    }
}

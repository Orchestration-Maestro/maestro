//! Local model catalog loading and composition.
mod model_registry;
/// Clear the shared credentials command cache.
pub use maestro_credentials::clear_config_value_cache as clear_api_key_cache;
pub use maestro_credentials::{
    AsyncLockUpdate, AuthCredential, AuthStorage, AuthStorageBackend, AuthStorageData,
    AuthStorageError, AuthStorageFuture, ConfigValueOperations, LockUpdate,
};
#[cfg(not(target_arch = "wasm32"))]
pub use model_registry::NativeModelFileOperations;
pub use model_registry::{
    CatalogError, ConfigValues, ModelFileOperations, ModelRegistry, ProviderCallbacks,
    ProviderConfigInput, ProviderModelInput, ResolvedRequestAuth,
};

mod defaults;
pub use defaults::DEFAULT_THINKING_LEVEL;
mod provider_display_names;
pub use provider_display_names::BUILT_IN_PROVIDER_DISPLAY_NAMES;
mod model_resolver;
pub use model_resolver::DEFAULT_MODEL_PER_PROVIDER;
pub use model_resolver::{
    FindInitialModelOptions, InitialModelResult, ModelResolverOutput,
    RestoreModelFromSessionOptions, RestoreModelFromSessionResult, ScopedModel, find_initial_model,
    resolve_model_scope, restore_model_from_session, restore_saved_model,
};
pub use model_resolver::{
    ParseModelPatternOptions, ParsedModelResult, find_exact_model_reference_match,
    model_id_comparator, parse_model_pattern,
};
pub use model_resolver::{ResolveCliModelOptions, ResolveCliModelResult, resolve_cli_model};

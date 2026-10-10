//! Local model catalog loading and composition.
mod model_registry;
/// Clear the shared credentials command cache.
pub use maestro_credentials::clear_config_value_cache as clear_api_key_cache;
#[cfg(not(target_arch = "wasm32"))]
pub use model_registry::NativeModelFileOperations;
pub use model_registry::{ModelFileOperations, ModelRegistry, ResolvedRequestAuth};

mod provider_display_names;
pub use provider_display_names::BUILT_IN_PROVIDER_DISPLAY_NAMES;

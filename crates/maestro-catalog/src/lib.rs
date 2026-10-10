//! Local model catalog loading and composition.
mod model_registry;
#[cfg(not(target_arch = "wasm32"))]
pub use model_registry::NativeModelFileOperations;
pub use model_registry::{ModelFileOperations, ModelRegistry};

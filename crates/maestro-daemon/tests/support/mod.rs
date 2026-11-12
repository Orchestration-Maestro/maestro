//! Shared daemon state admission and invariant observations.

pub mod api;
pub mod layouts;
pub mod runtime;
pub mod spawning;

/// Builds one isolated detection snapshot for state and any pane runtimes under test.
pub fn manifest_cache()
-> std::sync::Arc<std::sync::RwLock<maestro_detection::definitions::ManifestCache>> {
    std::sync::Arc::new(std::sync::RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ))
}

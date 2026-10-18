//! Controlled caller-supplied factory initialization.
use crate::{
    EventError, ExtensionExecution,
    types::{Extension, ExtensionAPI, ExtensionFactory, ExtensionHandle},
};
use std::sync::{Arc, RwLock};
/// Initializes a supplied factory without scanning or module loading.
/// # Errors
/// Returns the original initialization error.
pub async fn load_extension_from_factory<E: ExtensionExecution>(
    factory: ExtensionFactory<E>,
    cwd: String,
    event_bus: crate::EventBus<E>,
    runtime: E::Shared<crate::types::ExtensionRuntime<E>>,
    extension_path: Option<String>,
) -> Result<ExtensionHandle<E>, EventError> {
    let path = extension_path.unwrap_or_else(|| "<inline>".into());
    let source = path
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .map_or("local", |s| {
            s.split(':')
                .next()
                .filter(|x| !x.is_empty())
                .unwrap_or("temporary")
        });
    let base_dir = (!path.starts_with('<')).then(|| maestro_path::dirname(&path));
    let source_info = Arc::new(RwLock::new(
        maestro_resources::create_synthetic_source_info(
            path.clone(),
            maestro_resources::source_info::SyntheticSourceOptions {
                source: source.into(),
                scope: None,
                origin: None,
                base_dir,
            },
        ),
    ));
    let owner = E::share(RwLock::new(Extension::new(path.clone(), path, source_info)));
    let api = ExtensionAPI::new(&owner, runtime, event_bus, cwd);
    factory(api).await?;
    Ok(owner)
}

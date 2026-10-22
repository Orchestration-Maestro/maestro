//! Controlled caller-supplied factory initialization.
use crate::{
    EventError, ExtensionExecution,
    types::{Extension, ExtensionAPI, ExtensionFactory, ExtensionHandle},
};
use std::sync::{Arc, RwLock};
/// Creates an empty owner labelled `extension_path` whose base directory is the parent
/// of `resolved_path`; a label starting with `<` has no base directory.
pub(super) fn create_extension<E: ExtensionExecution>(
    extension_path: String,
    resolved_path: String,
) -> ExtensionHandle<E> {
    let source = extension_path
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .map_or("local", |s| {
            s.split(':')
                .next()
                .filter(|x| !x.is_empty())
                .unwrap_or("temporary")
        });
    let base_dir =
        (!extension_path.starts_with('<')).then(|| maestro_path::dirname(&resolved_path));
    let source_info = Arc::new(RwLock::new(
        maestro_resources::create_synthetic_source_info(
            extension_path.clone(),
            maestro_resources::source_info::SyntheticSourceOptions {
                source: source.into(),
                scope: None,
                origin: None,
                base_dir,
            },
        ),
    ));
    E::share(RwLock::new(Extension::new(
        extension_path,
        resolved_path,
        source_info,
    )))
}
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
    let owner = create_extension::<E>(path.clone(), path);
    let api = ExtensionAPI::new(&owner, runtime, event_bus, cwd);
    factory(api).await?;
    Ok(owner)
}

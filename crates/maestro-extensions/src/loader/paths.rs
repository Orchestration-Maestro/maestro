//! Path resolution and loading through a caller-supplied module loader.
use super::factory::create_extension;
use crate::{
    EventBusController, ExtensionExecution, create_extension_runtime,
    types::{ExtensionAPI, ExtensionLoadError, LoadExtensionsResult, ModuleLoader},
};
use maestro_resources::paths::resolve;
use maestro_tools::path_utils::normalize_unicode_spaces;
use std::io;

/// Replaces Unicode spaces, then joins the rest of a leading `~` or `~/` to the home directory.
fn expand_path(path: &str) -> io::Result<String> {
    let normalized = normalize_unicode_spaces(path);
    let Some(rest) = normalized.strip_prefix('~') else {
        return Ok(normalized);
    };
    let home = maestro_tools::expand_path("~")?;
    Ok(maestro_path::join(&[
        &home,
        rest.strip_prefix('/').unwrap_or(rest),
    ]))
}
/// Keeps an absolute `path` as written and resolves a relative one against `cwd`.
pub(super) fn anchor_path(path: &str, cwd: &str) -> io::Result<String> {
    if maestro_path::is_absolute(path) {
        return Ok(path.to_owned());
    }
    resolve(&[cwd, path])
}
/// Expands a typed `extension_path`, then anchors it at `cwd`.
pub(super) fn resolve_path(extension_path: &str, cwd: &str) -> io::Result<String> {
    anchor_path(&expand_path(extension_path)?, cwd)
}
/// Loads `paths` in order through `module_loader`, with one shared runtime and one bus
/// identity per path. A failed path is recorded with its supplied spelling and loading
/// continues.
///
/// # Errors
/// Returns the home-directory or working-directory error when a path cannot be resolved.
pub async fn load_extensions<E: ExtensionExecution>(
    paths: Vec<String>,
    cwd: &str,
    event_bus: &EventBusController<E>,
    module_loader: &ModuleLoader<E>,
) -> io::Result<LoadExtensionsResult<E>> {
    load_with(paths, resolve_path, cwd, event_bus, module_loader).await
}
/// Loads `paths` as [`load_extensions`] does, resolving each with `resolve` just before
/// its module loads.
pub(super) async fn load_with<E: ExtensionExecution>(
    paths: Vec<String>,
    resolve: fn(&str, &str) -> io::Result<String>,
    cwd: &str,
    event_bus: &EventBusController<E>,
    module_loader: &ModuleLoader<E>,
) -> io::Result<LoadExtensionsResult<E>> {
    let mut result = LoadExtensionsResult {
        extensions: Vec::new(),
        errors: Vec::new(),
        runtime: create_extension_runtime::<E>(),
    };
    for path in paths {
        let resolved_path = resolve(&path, cwd)?;
        let loaded = match module_loader(&resolved_path).await {
            Ok(Some(factory)) => {
                let extension = create_extension::<E>(path.clone(), resolved_path);
                let api = ExtensionAPI::new(
                    &extension,
                    Clone::clone(&result.runtime),
                    event_bus.for_extension(),
                    cwd.to_owned(),
                );
                factory(api)
                    .await
                    .map(|()| extension)
                    .map_err(|error| format!("Failed to load extension: {error}"))
            }
            Ok(None) => Err(format!(
                "Extension does not export a valid factory function: {path}"
            )),
            Err(error) => Err(format!("Failed to load extension: {error}")),
        };
        match loaded {
            Ok(extension) => result.extensions.push(extension),
            Err(error) => result.errors.push(ExtensionLoadError { path, error }),
        }
    }
    Ok(result)
}

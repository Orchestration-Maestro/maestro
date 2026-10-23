//! Path resolution and loading through a caller-supplied module loader.
use super::factory::create_extension;
use crate::{
    EventBusController, ExtensionExecution, create_extension_runtime,
    types::{ExtensionAPI, ExtensionLoadError, LoadExtensionsResult, ModuleLoader},
};
use maestro_resources::paths::{home_directory, resolve};
use maestro_tools::path_utils::resolve_existing_spelling;
use std::io;

/// Joins the rest of a leading `~` below the home directory through `home`, then keeps an
/// absolute result as written and resolves a relative one against `cwd`.
fn anchor_path(
    path: &str,
    cwd: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    let expanded = expand_extension_path(path, home)?;
    if maestro_path::is_absolute(&expanded) {
        return Ok(expanded);
    }
    resolve(&[cwd, &expanded])
}

/// Join the rest of a leading `~` below the home directory through `home`.
///
/// The rest is always joined, an empty one included, so a home spelling that ends in `/` or `/.`
/// is normalized. Characters stay as written, so the selector still sees the written candidate
/// before it falls back to the space-replaced one. Any other path is returned as written without
/// reading the home directory.
fn expand_extension_path(
    path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    if !path.starts_with('~') {
        return Ok(path.to_owned());
    }
    let rest = path.strip_prefix('~').unwrap_or_default();
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    Ok(maestro_path::join(&[&home()?, rest]))
}
/// Anchors `extension_path` at `cwd` and resolves it through
/// [`resolve_existing_spelling`], which keeps the spelling of the directories the file
/// system holds.
pub(super) fn resolve_path(extension_path: &str, cwd: &str) -> io::Result<String> {
    resolve_existing_spelling(
        extension_path,
        |path| expand_extension_path(path, home_directory),
        |path| anchor_path(path, cwd, home_directory),
    )
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
    let mut result = LoadExtensionsResult {
        extensions: Vec::new(),
        errors: Vec::new(),
        runtime: create_extension_runtime::<E>(),
    };
    for path in paths {
        let resolved_path = resolve_path(&path, cwd)?;
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

#[cfg(test)]
mod tests {
    use super::anchor_path;
    use std::io;

    /// Each extension form returns the reference's result for the six supplied homes.
    #[test]
    fn extension_paths_follow_the_reference_table() {
        for (written, home, expected) in [
            ("~", "/h", "/h"),
            ("~/", "/h", "/h"),
            ("~/x", "/h", "/h/x"),
            ("~x", "/h", "/h/x"),
            ("~", "/h/.", "/h"),
            ("~", "", "/w"),
        ] {
            assert_eq!(
                anchor_path(written, "/w", || Ok(home.to_owned())).unwrap(),
                expected,
                "{written:?} with home {home:?}"
            );
        }
    }

    /// A path that does not start with `~` never reads the home directory.
    #[test]
    fn other_forms_read_no_home_directory() {
        let failing = || Err(io::Error::new(io::ErrorKind::NotFound, "unknown home"));
        assert_eq!(anchor_path("x/~/y", "/w", failing).unwrap(), "/w/x/~/y");
    }
}

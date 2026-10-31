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
    let expanded = maestro_path::expand_tilde(path, home)?;
    if maestro_path::is_absolute(&expanded) {
        return Ok(expanded);
    }
    resolve(&[cwd, &expanded])
}

/// Anchors `extension_path` at `cwd` and resolves it through
/// [`resolve_existing_spelling`], which keeps the spelling of the directories the file
/// system holds.
pub(super) fn resolve_path(extension_path: &str, cwd: &str) -> io::Result<String> {
    resolve_existing_spelling(
        extension_path,
        |path| maestro_path::expand_tilde(path, home_directory),
        |path| anchor_path(path, cwd, home_directory),
    )
}
/// Loads `paths` in order through `module_loader`, with one shared runtime and one bus
/// identity per path. The module loader receives a lexically normalized absolute path;
/// the extension record keeps the selected spelling. A failed path is recorded with
/// its supplied spelling and loading continues.
///
/// # Errors
/// Returns the home-directory or working-directory error when a path cannot be resolved.
pub async fn load_extensions<E: ExtensionExecution>(
    paths: Vec<String>,
    cwd: &str,
    event_bus: &EventBusController<E>,
    module_loader: &ModuleLoader<E>,
) -> io::Result<LoadExtensionsResult<E>> {
    load_extensions_with_runtime(
        paths,
        cwd,
        event_bus,
        module_loader,
        create_extension_runtime::<E>(),
    )
    .await
}

/// Loads paths on a supplied runtime whose constructor services are already available.
///
/// # Errors
/// Returns a path resolution failure; individual extension failures remain in the result.
pub async fn load_extensions_with_runtime<E: ExtensionExecution>(
    paths: Vec<String>,
    cwd: &str,
    event_bus: &EventBusController<E>,
    module_loader: &ModuleLoader<E>,
    runtime: E::Shared<crate::types::ExtensionRuntime<E>>,
) -> io::Result<LoadExtensionsResult<E>> {
    let mut result = LoadExtensionsResult {
        extensions: Vec::new(),
        errors: Vec::new(),
        runtime,
    };
    for path in paths {
        let resolved_path = resolve_path(&path, cwd)?;
        let module_path = resolve(&[&resolved_path])?;
        let loaded = match module_loader(&module_path).await {
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

    /// Each extension form returns the reference's result for the six supplied homes, spelled in
    /// the host's path flavor.
    #[test]
    fn extension_paths_follow_the_reference_table() {
        let (cwd, rows) = if cfg!(windows) {
            (
                r"C:\w",
                [
                    ("~", r"C:\h", r"C:\h"),
                    ("~/", r"C:\h", r"C:\h"),
                    ("~/x", r"C:\h", r"C:\h\x"),
                    ("~x", r"C:\h", r"C:\w\~x"),
                    ("~", r"C:\h\.", r"C:\h\."),
                    ("~", "", r"C:\w"),
                ],
            )
        } else {
            (
                "/w",
                [
                    ("~", "/h", "/h"),
                    ("~/", "/h", "/h/"),
                    ("~/x", "/h", "/h/x"),
                    ("~x", "/h", "/w/~x"),
                    ("~", "/h/.", "/h/."),
                    ("~", "", "/w"),
                ],
            )
        };
        for (written, home, expected) in rows {
            assert_eq!(
                anchor_path(written, cwd, || Ok(home.to_owned())).unwrap(),
                expected,
                "{written:?} with home {home:?}"
            );
        }
    }

    /// A path that is neither `~` nor `~` plus a separator never reads the home directory.
    #[test]
    fn other_forms_read_no_home_directory() {
        let failing = || Err(io::Error::new(io::ErrorKind::NotFound, "unknown home"));
        let (cwd, rows) = if cfg!(windows) {
            (r"C:\w", [("x/~/y", r"C:\w\x\~\y"), ("~x", r"C:\w\~x")])
        } else {
            ("/w", [("x/~/y", "/w/x/~/y"), ("~x", "/w/~x")])
        };
        for (written, expected) in rows {
            assert_eq!(anchor_path(written, cwd, failing).unwrap(), expected);
        }
    }
}

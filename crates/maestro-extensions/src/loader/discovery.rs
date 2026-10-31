//! Extension discovery in the project, user and configured locations.
use super::paths::{load_extensions, resolve_path};
use crate::{
    EventBusController, ExtensionExecution,
    types::{LoadExtensionsResult, ModuleLoader},
};
use maestro_path::join;
use maestro_resources::{
    ResourceFileType,
    package_manifest::{is_extension_file, resolve_extension_entries},
    paths::{CONFIG_DIR_NAME, resolve},
    read_entries,
};
use std::{collections::HashSet, io, path::Path};

/// One level of `dir` in name order: admitted files and links, and each child directory's
/// or link's declared entries. A listing or entry-resolution error drops the directory.
fn discover_extensions_in_dir(dir: &str) -> Vec<String> {
    let Ok(entries) = read_entries(Path::new(dir)) else {
        return Vec::new();
    };
    let mut discovered = Vec::new();
    for entry in entries {
        let name = entry.name.to_string_lossy();
        let path = join(&[dir, &name]);
        let link = entry.file_type == ResourceFileType::Symlink;
        if (link || entry.file_type == ResourceFileType::File) && is_extension_file(&name) {
            discovered.push(path);
        } else if link || entry.file_type == ResourceFileType::Directory {
            match resolve_extension_entries(&path) {
                Ok(entries) => discovered.extend(entries.into_iter().flatten()),
                Err(_) => return Vec::new(),
            }
        }
    }
    discovered
}
/// Paths in their first spelling, keyed by their resolved spelling.
#[derive(Default)]
struct Discovered {
    /// Resolved keys already added.
    seen: HashSet<String>,
    /// Paths in their first spelling.
    paths: Vec<String>,
}
impl Discovered {
    /// Adds each path whose resolved key is new.
    fn add(&mut self, paths: Vec<String>) -> io::Result<()> {
        for path in paths {
            if self.seen.insert(resolve(&[&path])?) {
                self.paths.push(path);
            }
        }
        Ok(())
    }
}
/// Discovers `<cwd>/.maestro/extensions`, then `<agent_dir>/extensions`, then each
/// configured path, and loads them as [`load_extensions`] does.
///
/// A configured directory contributes its declared entries, else a one-level scan; any
/// other configured path is added as resolved. The first spelling of each resolved path
/// wins.
///
/// # Errors
/// Returns a path-resolution error.
pub async fn discover_and_load_extensions<E: ExtensionExecution>(
    configured_paths: &[String],
    cwd: &str,
    agent_dir: &str,
    event_bus: &EventBusController<E>,
    module_loader: &ModuleLoader<E>,
) -> io::Result<LoadExtensionsResult<E>> {
    let mut found = Discovered::default();
    found.add(discover_extensions_in_dir(&join(&[
        cwd,
        CONFIG_DIR_NAME,
        "extensions",
    ])))?;
    found.add(discover_extensions_in_dir(&join(&[
        agent_dir,
        "extensions",
    ])))?;
    for configured in configured_paths {
        let resolved = resolve_path(configured, cwd)?;
        if !Path::new(&resolved).is_dir() {
            found.add(vec![resolved])?;
        } else if let Some(entries) = resolve_extension_entries(&resolved)? {
            found.add(entries)?;
        } else {
            found.add(discover_extensions_in_dir(&resolved))?;
        }
    }
    load_extensions(found.paths, cwd, event_bus, module_loader).await
}

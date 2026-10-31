//! Raw package branding selection with deferred resource-list decoding.
pub(super) use maestro_json::is_raw_truthy as truthy;
pub(super) use maestro_resources::package_manifest::{Manifest, read_maestro_manifest_file};
use std::io;

/// Reads a manifest's resource list, recording an ignored malformed field in `result`.
pub(super) fn listed_entries(
    manifest: &Manifest,
    package_path: &str,
    kind: super::resource_discovery::ResourceType,
    result: &mut super::resolution::Accumulator,
) -> Vec<String> {
    let listed = manifest.entries(kind.name());
    if let Some(reason) = listed.ignored {
        result.ignore_field(&maestro_path::join(&[package_path, "package.json"]), reason);
    }
    listed.entries
}

/// Expands positive operands before collecting their resource kinds.
pub(super) fn collect_manifest_entries(
    entries: &[String],
    root: &str,
    kind: super::resource_discovery::ResourceType,
) -> io::Result<Vec<String>> {
    use super::resource_discovery::collect_files_from_paths;
    let mut paths = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| !entry.starts_with(['!', '+', '-']))
    {
        if entry.contains(['*', '?']) {
            paths.extend(super::glob::glob_sync(entry, root)?);
        } else {
            paths.push(maestro_resources::paths::resolve(&[root, entry])?);
        }
    }
    Ok(collect_files_from_paths(paths, kind))
}

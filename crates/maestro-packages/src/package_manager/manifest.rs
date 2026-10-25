//! Raw package branding selection with deferred resource-list decoding.
pub(super) use maestro_json::is_raw_truthy as truthy;
pub(super) use maestro_resources::package_manifest::{Manifest, read_maestro_manifest_file};
use std::io;

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

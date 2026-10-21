//! Raw package branding selection with deferred resource-list decoding.
pub(super) use maestro_json::is_raw_truthy as truthy;
use serde_json::value::RawValue;
use std::{collections::BTreeMap, io, path::Path};
/// Named fields retained without consuming their resource lists.
pub(super) struct Manifest {
    /// Last selected value for each decoded key.
    fields: BTreeMap<String, Box<RawValue>>,
}
impl Manifest {
    /// Borrows a named field without decoding its contents.
    pub(super) fn field(&self, name: &str) -> Option<&RawValue> {
        self.fields.get(name).map(AsRef::as_ref)
    }
    /// Consumes one complete string list; false values contribute no entries.
    pub(super) fn entries(&self, name: &str) -> io::Result<Vec<String>> {
        let Some(raw) = self.field(name).filter(|raw| truthy(raw)) else {
            return Ok(Vec::new());
        };
        serde_json::from_str(raw.get())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
/// Reads the selected branding member, treating unreadable manifests as absent.
pub(super) fn read_maestro_manifest_file(path: &str) -> Option<Manifest> {
    let content = maestro_resources::read_text(Path::new(path)).ok()?;
    let mut package: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&content).ok()?;
    let raw = package.remove("maestro").filter(|raw| truthy(raw))?;
    let fields = if raw.get().starts_with('{') {
        serde_json::from_str(raw.get()).ok()?
    } else {
        BTreeMap::new()
    };
    Some(Manifest { fields })
}

/// Expands positive operands before collecting their resource kinds.
pub(super) fn collect_manifest_entries(
    entries: &[String],
    root: &str,
    kind: super::resource_discovery::ResourceType,
) -> io::Result<Vec<String>> {
    use super::resource_discovery::{collect_files_from_paths, resolve_entry_path};
    let mut paths = Vec::new();
    for entry in entries
        .iter()
        .filter(|entry| !entry.starts_with(['!', '+', '-']))
    {
        if entry.contains(['*', '?']) {
            paths.extend(super::glob::glob_sync(entry, root)?);
        } else {
            paths.push(resolve_entry_path(root, entry)?);
        }
    }
    Ok(collect_files_from_paths(paths, kind))
}

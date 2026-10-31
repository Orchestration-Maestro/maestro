//! The package manifest and the extension entry points a directory declares.
use maestro_json::is_raw_truthy as truthy;
use maestro_path::join;
use serde_json::value::RawValue;
use std::{collections::BTreeMap, io, path::Path};

/// Extension module suffixes admitted by file discovery.
const EXTENSION_SUFFIXES: [&str; 3] = [".ts", ".js", ".wasm"];
/// Directory index entry points, in preference order.
const EXTENSION_INDEX_NAMES: [&str; 3] = ["index.ts", "index.js", "index.wasm"];

/// Named fields retained without consuming their resource lists.
pub struct Manifest {
    /// Last selected value for each decoded key.
    fields: BTreeMap<String, Box<RawValue>>,
}
impl Manifest {
    /// Borrows a named field without decoding its contents.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&RawValue> {
        self.fields.get(name).map(AsRef::as_ref)
    }
    /// Consumes one complete string list; false values contribute no entries.
    ///
    /// # Errors
    /// Returns `InvalidData` when a present truthy value is not a list of strings.
    pub fn entries(&self, name: &str) -> io::Result<Vec<String>> {
        let Some(raw) = self.field(name).filter(|raw| truthy(raw)) else {
            return Ok(Vec::new());
        };
        serde_json::from_str(raw.get())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
/// Reads the selected branding member, treating unreadable manifests as absent.
#[must_use]
pub fn read_maestro_manifest_file(path: &str) -> Option<Manifest> {
    let content = crate::read_text(Path::new(path)).ok()?;
    let mut package: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&content).ok()?;
    let raw = package.remove("maestro").filter(|raw| truthy(raw))?;
    let fields = if raw.get().starts_with('{') {
        serde_json::from_str(raw.get()).ok()?
    } else {
        BTreeMap::new()
    };
    Some(Manifest { fields })
}
/// Whether a file name has an admitted extension module suffix; matching is case-sensitive.
#[must_use]
pub fn is_extension_file(name: &str) -> bool {
    EXTENSION_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}
/// Selects the existing manifest entries in list order, else the first existing index.
///
/// A list that holds a non-string item contributes no entries when `ignore_malformed` is set.
///
/// # Errors
/// Returns `InvalidData` when the manifest's extension list holds a non-string item and
/// `ignore_malformed` is not set.
pub fn resolve_extension_entries(
    dir: &str,
    ignore_malformed: bool,
) -> io::Result<Option<Vec<String>>> {
    let manifest_path = join(&[dir, "package.json"]);
    if Path::new(&manifest_path).exists()
        && let Some(manifest) = read_maestro_manifest_file(&manifest_path)
        && manifest
            .field("extensions")
            .is_some_and(|raw| raw.get().starts_with('['))
    {
        let mut found = Vec::new();
        let listed = manifest.entries("extensions");
        let listed = if ignore_malformed {
            listed.unwrap_or_default()
        } else {
            listed?
        };
        for entry in listed {
            let path = crate::paths::resolve(&[dir, &entry])?;
            if Path::new(&path).exists() {
                found.push(path);
            }
        }
        if !found.is_empty() {
            return Ok(Some(found));
        }
    }
    Ok(EXTENSION_INDEX_NAMES
        .into_iter()
        .map(|name| join(&[dir, name]))
        .find(|path| Path::new(path).exists())
        .map(|path| vec![path]))
}

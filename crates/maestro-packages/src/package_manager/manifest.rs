//! Raw package branding selection for extension entry discovery.
use maestro_resources::skills::ResourceOperations;
use serde_json::Value;
use std::path::Path;
/// Reads the selected branding member, treating unreadable manifests as absent.
pub(super) fn read_maestro_manifest_file(
    path: &str,
    operations: &dyn ResourceOperations,
) -> Option<Value> {
    let content = operations.read_file(Path::new(path)).ok()?;
    let mut package: Value = serde_json::from_str(&content).ok()?;
    package
        .as_object_mut()?
        .remove("maestro")
        .filter(|value| !value.is_null())
}

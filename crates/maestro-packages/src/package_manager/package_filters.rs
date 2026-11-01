//! Package defaults and selected object resource filters.
use super::{
    DefaultPackageManager, PackageOperations,
    manifest::{collect_manifest_entries, read_maestro_manifest_file},
    resolution::Accumulator,
    resource_discovery::{ResourceType, collect_resource_files},
    resource_patterns::{apply_patterns, select_patterns},
};
use maestro_path::join;
use maestro_resources::PathMetadata;
use serde_json::{Map, Value};
use std::io;
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Collects each selected resource kind before reading its successor's filter.
    pub(super) fn collect_package(
        path: &str,
        metadata: &PathMetadata,
        filter: Option<&Map<String, Value>>,
        result: &mut Accumulator,
    ) -> io::Result<bool> {
        let Some(filter) = filter else {
            return Self::collect_conventional(path, metadata, result);
        };
        for kind in ResourceType::ALL {
            Self::collect_filtered_kind((path, metadata), kind, filter.get(kind.name()), result)?;
        }
        Ok(true)
    }
    /// Emits admitted candidates with defaults or one selected user filter.
    fn collect_filtered_kind(
        (path, metadata): (&str, &PathMetadata),
        kind: ResourceType,
        filter: Option<&Value>,
        result: &mut Accumulator,
    ) -> io::Result<()> {
        let files = candidate_files(path, kind, filter.is_some())?;
        let patterns = filter.map(decode_patterns).transpose()?;
        let enabled = select_patterns(&files, patterns.as_deref().unwrap_or(&[]), path)?;
        let empty = patterns.as_ref().is_some_and(Vec::is_empty);
        for file in &files {
            result.insert_enabled(
                kind,
                file.clone(),
                metadata,
                !empty && enabled.contains(file),
            );
        }
        Ok(())
    }
}
/// Collects manifest-admitted candidates with the caller's required emission order.
fn candidate_files(path: &str, kind: ResourceType, filtered: bool) -> io::Result<Vec<String>> {
    let manifest = read_maestro_manifest_file(&join(&[path, "package.json"]));
    let entries = manifest
        .as_ref()
        .filter(|m| m.field(kind.name()).is_some_and(super::manifest::truthy))
        .and_then(|m| m.entries(kind.name()).ok());
    let present = entries.is_some();
    let entries = entries.unwrap_or_default();
    if !present || (filtered && entries.is_empty()) {
        return collect_resource_files(&join(&[path, kind.name()]), kind);
    }
    let files = collect_manifest_entries(&entries, path, kind)?;
    let admitted = apply_patterns(&files, &entries, path)?;
    Ok(if filtered {
        admitted.ordered.iter().map(|s| (*s).to_owned()).collect()
    } else {
        files
            .iter()
            .filter(|s| admitted.contains(s))
            .cloned()
            .collect()
    })
}
/// Decodes only a selected package resource filter.
fn decode_patterns(value: &Value) -> io::Result<Vec<&str>> {
    let patterns = value.as_array().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "package filter is not an array")
    })?;
    patterns
        .iter()
        .map(|v| {
            v.as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "package pattern is not a string",
                )
            })
        })
        .collect()
}

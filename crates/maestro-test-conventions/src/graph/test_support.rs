//! Cargo-resolved boundaries for opt-in test helpers.
use cargo_metadata::{DependencyKind, Metadata, Package};

use super::metadata::members;

/// Validate activation independently of dependency spelling or target predicates.
pub(super) fn check(metadata: &Metadata) -> Result<(), String> {
    for package in members(metadata) {
        for dependency in &package.dependencies {
            if dependency.kind != DependencyKind::Development
                && dependency
                    .features
                    .iter()
                    .any(|feature| feature == "test-support")
                && (dependency.kind != DependencyKind::Normal
                    || !package.name.starts_with("maestro-test-"))
            {
                return Err(forbidden(package));
            }
            if dependency.name == package.name.as_str()
                && (dependency.kind != DependencyKind::Development
                    || dependency.features != ["test-support"])
            {
                return Err(format!(
                    "{}: self dependencies may only activate test-support for tests",
                    package.name
                ));
            }
        }
        check_features(package, metadata)?;
    }
    Ok(())
}

/// Keep forwarding confined to one opt-in hop, outside default/runtime features.
fn check_features(package: &Package, metadata: &Metadata) -> Result<(), String> {
    if package.name.starts_with("maestro-test-") {
        return Ok(());
    }
    for (feature, values) in &package.features {
        for value in values {
            if value == "test-support" {
                return Err(forbidden(package));
            }
            let Some((dependency, "test-support")) = value.split_once('/') else {
                continue;
            };
            let dependency = dependency.trim_end_matches('?');
            let target = package.dependencies.iter().find(|entry| {
                entry.rename.as_deref().unwrap_or(&entry.name) == dependency
                    && entry.kind == DependencyKind::Normal
            });
            let target = target.and_then(|entry| {
                members(metadata).find(|target| target.name.as_str() == entry.name)
            });
            if feature != "test-support" || target.is_none_or(has_forwarding) {
                return Err(forbidden(package));
            }
        }
    }
    Ok(())
}

/// Identify a second hop through another product's opt-in surface.
fn has_forwarding(package: &Package) -> bool {
    package
        .features
        .get("test-support")
        .is_some_and(|values| values.iter().any(|value| value.ends_with("/test-support")))
}

/// Report the same boundary for direct, aliased and forwarded activation.
fn forbidden(package: &Package) -> String {
    format!(
        "{}: test-support may only be enabled by development dependencies or dedicated test libraries",
        package.name
    )
}

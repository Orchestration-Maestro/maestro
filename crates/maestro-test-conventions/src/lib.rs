//! Checks the workspace's crate boundaries against its reviewed crate list.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

mod comments;
mod graph;

/// Checks names, membership and internal dependency boundaries using Cargo metadata.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = metadata_command(root, "--no-deps")?;
    let members = array(&metadata, "workspace_members")?;
    let list_path = root.join("workspace-crates.json");
    let list: Value = serde_json::from_slice(
        &std::fs::read(&list_path).map_err(|error| format!("{}: {error}", list_path.display()))?,
    )
    .map_err(|error| format!("{}: {error}", list_path.display()))?;
    let list = list.as_object().ok_or_else(|| {
        "workspace-crates.json must be an object mapping crate names to layers".to_string()
    })?;
    for (name, layer) in list {
        if !matches!(layer.as_str(), Some("core" | "dedicated")) {
            return Err(format!(
                "invalid layer for {name}: expected core or dedicated"
            ));
        }
    }
    for package in array(&metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let name = string(package, "name")?;
        if !valid_name(name) {
            return Err(format!("invalid workspace crate name: {name}"));
        }
        if list.get(name).is_none() {
            return Err(format!(
                "workspace crate is not listed in workspace-crates.json: {name}"
            ));
        }
    }
    graph::inventory(&metadata, list)?;
    let mut edges = graph::declared(&metadata)?;
    graph::validate(&metadata, &edges)?;
    let resolved = metadata_command(root, "--all-features")?;
    edges.extend(graph::resolved(&resolved)?);
    graph::validate(&metadata, &edges)?;
    comments::check(&metadata)
}

fn metadata_command(root: &Path, mode: &str) -> Result<Value, String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--format-version", "1", mode, "--offline"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("invalid cargo metadata: expected array {key}"))
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("invalid cargo metadata: expected string {key}"))
}

fn valid_name(name: &str) -> bool {
    if name == "maestro" {
        return true;
    }
    let Some(suffix) = name.strip_prefix("maestro-") else {
        return false;
    };
    let segments: Vec<_> = suffix.split('-').collect();
    (1..=2).contains(&segments.len())
        && segments.iter().all(|segment| {
            segment
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod graph_tests;

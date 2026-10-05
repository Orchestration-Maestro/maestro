//! Checks the workspace's crate boundaries against its reviewed crate list.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Checks names, membership and internal dependency boundaries using Cargo metadata.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = metadata(root)?;
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
    let graph = dependency_graph(&metadata)?;
    if let Some(dependencies) = graph.get("maestro-models")
        && let Some(dependency) = dependencies.first()
    {
        return Err(format!(
            "maestro-models must not depend on workspace crate {dependency}"
        ));
    }
    for (name, dependencies) in &graph {
        if list[name] != "core" {
            continue;
        }
        for dependency in dependencies {
            if list[dependency] == "dedicated" {
                return Err(format!(
                    "core crate {name} must not depend on dedicated crate {dependency}"
                ));
            }
        }
    }
    let mut visited = BTreeSet::new();
    for name in graph.keys() {
        check_cycles(name, &graph, &mut Vec::new(), &mut visited)?;
    }
    Ok(())
}

// Use manifest paths rather than dependency names: Cargo dependencies can be renamed.
// --no-deps lets us inspect cycles ourselves, before Cargo's resolver rejects them.
fn dependency_graph(metadata: &Value) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let members = array(metadata, "workspace_members")?;
    let packages: Vec<_> = array(metadata, "packages")?
        .iter()
        .filter(|package| members.contains(&package["id"]))
        .collect();
    let mut names = BTreeMap::new();
    for package in &packages {
        let manifest = Path::new(string(package, "manifest_path")?);
        let directory = manifest
            .parent()
            .ok_or("manifest has no parent directory")?;
        names.insert(canonical(directory)?, string(package, "name")?.to_owned());
    }
    let mut graph = BTreeMap::new();
    for package in packages {
        let mut edges = BTreeSet::new();
        // All dependency kinds, optional features and target-specific entries count.
        for dependency in array(package, "dependencies")? {
            let path = dependency
                .get("path")
                .map(|path| {
                    let path = path
                        .as_str()
                        .ok_or("invalid cargo metadata: dependency path")?;
                    canonical(Path::new(path))
                })
                .transpose()?;
            let member = path.as_ref().and_then(|path| names.get(path));
            let dependency_name = string(dependency, "name")?;
            // Registry/git references may be patched to members without exposing a path
            // in --no-deps metadata. Require explicit member paths instead of guessing.
            if names.values().any(|name| name == dependency_name)
                && member.map(String::as_str) != Some(dependency_name)
            {
                return Err(format!(
                    "dependency {} -> {dependency_name} must use a path to that workspace member",
                    string(package, "name")?
                ));
            }
            if let Some(name) = member {
                edges.insert(name.clone());
            }
        }
        graph.insert(string(package, "name")?.to_owned(), edges);
    }
    Ok(graph)
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))
}

fn check_cycles<'a>(
    name: &'a str,
    graph: &'a BTreeMap<String, BTreeSet<String>>,
    visiting: &mut Vec<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    if let Some(start) = visiting.iter().position(|entry| *entry == name) {
        let mut cycle = visiting[start..].to_vec();
        cycle.push(name);
        return Err(format!(
            "workspace dependency cycle: {}",
            cycle.join(" -> ")
        ));
    }
    if visited.contains(name) {
        return Ok(());
    }
    visiting.push(name);
    for dependency in &graph[name] {
        check_cycles(dependency, graph, visiting, visited)?;
    }
    visiting.pop();
    visited.insert(name);
    Ok(())
}

fn metadata(root: &Path) -> Result<Value, String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ])
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

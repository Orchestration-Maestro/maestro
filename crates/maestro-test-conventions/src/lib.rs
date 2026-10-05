//! Checks the workspace's crate boundaries against its reviewed crate list.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

mod comments;
mod graph;

/// Checks names, membership and internal dependency boundaries using Cargo metadata.
///
/// Declared edges cover all features and targets. The resolved pass uses default
/// features for the compiler host so it reads only crates a normal build fetched.
pub fn check_workspace(root: &Path) -> Result<(), String> {
    let metadata = metadata_command(root, &["--no-deps"])?;
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
    let host = compiler_host()?;
    let resolved = metadata_command(root, &["--filter-platform", &host])?;
    edges.extend(graph::resolved(&resolved)?);
    graph::validate(&metadata, &edges)?;
    comments::check(&metadata)
}

fn metadata_command(root: &Path, options: &[&str]) -> Result<Value, String> {
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--offline"])
        .args(options)
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

fn compiler_host() -> Result<String, String> {
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("-vV")
        .output()
        .map_err(|error| format!("rustc -vV failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "rustc -vV failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_compiler_host(&String::from_utf8_lossy(&output.stdout))
}

fn parse_compiler_host(version: &str) -> Result<String, String> {
    version
        .lines()
        .find_map(|line| line.strip_prefix("host:"))
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV failed: missing or empty host line".into())
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

#[cfg(test)]
mod tests {
    use super::parse_compiler_host;

    #[test]
    fn compiler_host_requires_a_nonempty_host_line() {
        assert_eq!(
            parse_compiler_host("rustc 1.96.0\nhost: x86_64-unknown-linux-gnu\nrelease: 1.96.0\n"),
            Ok("x86_64-unknown-linux-gnu".into())
        );
        for version in ["release: 1.96.0\n", "host:\n", "host:   \n"] {
            assert_eq!(
                parse_compiler_host(version),
                Err("rustc -vV failed: missing or empty host line".into())
            );
        }
    }
}

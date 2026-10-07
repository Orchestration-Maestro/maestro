use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::Metadata;
use serde::Deserialize;

pub(crate) mod metadata;
mod policy;
use metadata::{canonical, check_kind, members};
use policy::rule;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum CrateClass {
    Core,
    Dedicated,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Class {
    Known(CrateClass),
    Invalid(serde::de::IgnoredAny),
}
impl Class {
    fn name(&self, name: &str) -> Result<&str, String> {
        match self {
            Self::Known(CrateClass::Core) => Ok("core"),
            Self::Known(CrateClass::Dedicated) => Ok("dedicated"),
            Self::Invalid(_) => Err(format!(
                "invalid layer for {name}: expected core or dedicated"
            )),
        }
    }
}

pub(crate) fn check(root: &Path) -> Result<Metadata, String> {
    let declared = metadata::load(root, None)?;
    metadata::identities(&declared)?;
    inventory(root, &declared)?;
    let mut edges = declared_edges(&declared)?;
    validate(&declared, &edges)?;
    let host = metadata::compiler_host()?;
    let resolved = metadata::load(root, Some(&host))?;
    edges.extend(metadata::resolved(&resolved, &declared)?);
    validate(&declared, &edges)?;
    complete(&declared, &edges)?;
    Ok(declared)
}

fn inventory(root: &Path, metadata: &Metadata) -> Result<(), String> {
    let path = root.join("workspace-crates.json");
    let contents = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let list: BTreeMap<String, Class> = serde_json::from_slice(&contents).map_err(|_| {
        "workspace-crates.json must be an object mapping crate names to layers".to_string()
    })?;
    for (name, class) in &list {
        class.name(name)?;
    }
    for package in members(metadata) {
        let name = package.name.as_str();
        if !valid_name(name) {
            return Err(format!("invalid workspace crate name: {name}"));
        }
        let class = list.get(name).ok_or_else(|| {
            format!("workspace crate is not listed in workspace-crates.json: {name}")
        })?;
        let policy = rule(name)
            .ok_or_else(|| format!("workspace crate is outside scoped policy: {name}"))?;
        if class.name(name)? != policy.class {
            return Err(format!(
                "invalid scoped layer for {name}: expected {}",
                policy.class
            ));
        }
    }
    Ok(())
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Normal,
    Build,
    Development,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Edge {
    from: String,
    to: String,
    kind: Kind,
}

fn declared_edges(metadata: &Metadata) -> Result<BTreeSet<Edge>, String> {
    let names: BTreeMap<_, _> = members(metadata)
        .map(|package| {
            canonical(
                package
                    .manifest_path
                    .parent()
                    .ok_or("manifest has no parent")?
                    .as_std_path(),
            )
            .map(|path| (path, package.name.as_str()))
        })
        .collect::<Result<_, _>>()?;
    if names.len() != metadata.workspace_members.len() {
        return Err("invalid cargo metadata: duplicate member directory".into());
    }
    let mut edges = BTreeSet::new();
    for package in members(metadata) {
        for dependency in &package.dependencies {
            let kind = check_kind(dependency.kind)?;
            let path = dependency
                .path
                .as_ref()
                .map(|path| canonical(path.as_std_path()))
                .transpose()?;
            let member = path.as_ref().and_then(|path| names.get(path)).copied();
            let name = dependency.name.as_str();
            check_library_dependency(package.name.as_str(), name)?;
            if names.values().any(|member| *member == name) && member != Some(name) {
                return Err(format!(
                    "dependency {} -> {name} must use a path to that workspace member",
                    package.name
                ));
            }
            if let Some(to) = member {
                edges.insert(Edge {
                    from: package.name.to_string(),
                    to: to.into(),
                    kind,
                });
            }
        }
    }
    Ok(edges)
}

fn validate(metadata: &Metadata, edges: &BTreeSet<Edge>) -> Result<(), String> {
    let mut production: BTreeMap<_, BTreeSet<String>> = members(metadata)
        .map(|package| (package.name.to_string(), BTreeSet::new()))
        .collect();
    let mut test = production.clone();
    for edge in edges {
        if !test.contains_key(&edge.to) {
            return Err(format!(
                "invalid cargo metadata: unknown graph target {}",
                edge.to
            ));
        }
        test.get_mut(&edge.from)
            .ok_or("invalid cargo metadata: unknown graph source")?
            .insert(edge.to.clone());
        if edge.kind != Kind::Development {
            production
                .get_mut(&edge.from)
                .ok_or("invalid cargo metadata: unknown graph source")?
                .insert(edge.to.clone());
        }
    }
    for (label, graph) in [("workspace", &production), ("test", &test)] {
        let mut visited = BTreeSet::new();
        for name in graph.keys() {
            check_cycles(name, graph, &mut Vec::new(), &mut visited, label)?;
        }
    }
    for edge in edges {
        validate_edge(edge)?;
    }
    Ok(())
}

fn validate_edge(edge: &Edge) -> Result<(), String> {
    let from = rule(&edge.from).ok_or("unknown scoped source")?;
    let to = rule(&edge.to).ok_or("unknown scoped target")?;
    if from.dependencies.is_empty() {
        return Err(format!(
            "{} must not depend on workspace crate {}",
            edge.from, edge.to
        ));
    }
    if from.class == "core" && to.class == "dedicated" {
        return Err(format!(
            "core crate {} must not depend on dedicated crate {}",
            edge.from, edge.to
        ));
    }
    if edge.kind == Kind::Development {
        return Err(format!(
            "internal dev dependency requires declared dependency-free test support: {} -> {}",
            edge.from, edge.to
        ));
    }
    if !from.dependencies.contains(&edge.to.as_str()) {
        return Err(format!(
            "forbidden production dependency: {} -> {}",
            edge.from, edge.to
        ));
    }
    Ok(())
}

fn complete(metadata: &Metadata, edges: &BTreeSet<Edge>) -> Result<(), String> {
    for package in members(metadata) {
        let name = package.name.as_str();
        if !policy::exact(name) {
            continue;
        }
        for required in rule(name).ok_or("unknown scoped source")?.dependencies {
            if !edges.iter().any(|edge| {
                edge.from == name && edge.to == *required && edge.kind != Kind::Development
            }) {
                return Err(format!(
                    "{name}: missing required direct dependency {required}"
                ));
            }
        }
    }
    Ok(())
}

fn check_cycles<'a>(
    name: &'a str,
    graph: &'a BTreeMap<String, BTreeSet<String>>,
    visiting: &mut Vec<&'a str>,
    visited: &mut BTreeSet<&'a str>,
    label: &str,
) -> Result<(), String> {
    if let Some(start) = visiting.iter().position(|entry| *entry == name) {
        let mut cycle = visiting[start..].to_vec();
        cycle.push(name);
        return Err(format!("{label} dependency cycle: {}", cycle.join(" -> ")));
    }
    if visited.contains(name) {
        return Ok(());
    }
    visiting.push(name);
    for dependency in &graph[name] {
        check_cycles(dependency, graph, visiting, visited, label)?;
    }
    visiting.pop();
    visited.insert(name);
    Ok(())
}

fn check_library_dependency(owner: &str, dependency_name: &str) -> Result<(), String> {
    if dependency_name == "wasmtime-wasi-http"
        || (dependency_name == "wasmtime" || dependency_name.starts_with("wasmtime-"))
            && owner != "maestro-extensions-wasmtime"
    {
        return Err(format!(
            "{owner}: forbidden runtime library dependency {dependency_name}"
        ));
    }
    if owner == "maestro-tui" && matches!(dependency_name, "ratatui" | "syntect" | "two-face") {
        return Err(format!(
            "{owner}: forbidden toolkit library dependency {dependency_name}"
        ));
    }
    Ok(())
}

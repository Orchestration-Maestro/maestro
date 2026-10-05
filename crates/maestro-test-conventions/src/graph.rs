use crate::{array, string};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

struct Rule {
    class: &'static str,
    dependencies: &'static [&'static str],
}
const DOMAIN: &[&str] = &[
    "maestro-models",
    "maestro-storage",
    "maestro-packages",
    "maestro-agent",
    "maestro-credentials",
    "maestro-tools",
    "maestro-session",
    "maestro-settings",
    "maestro-resources",
    "maestro-extensions",
];
const CORE: &[&str] = &[
    "maestro-models",
    "maestro-storage",
    "maestro-packages",
    "maestro-agent",
    "maestro-credentials",
    "maestro-tools",
    "maestro-session",
    "maestro-settings",
    "maestro-resources",
    "maestro-extensions",
    "maestro-app",
    "maestro-cli",
];
fn rule(name: &str) -> Option<Rule> {
    let dependencies: &'static [&'static str] = match name {
        "maestro-models" | "maestro-storage" | "maestro-packages" | "maestro-test-conventions" => {
            &[]
        }
        "maestro-agent" | "maestro-credentials" => &["maestro-models"],
        "maestro-tools" => &["maestro-agent", "maestro-models"],
        "maestro-session" => &["maestro-agent", "maestro-models", "maestro-storage"],
        "maestro-settings" => &["maestro-models", "maestro-agent", "maestro-packages"],
        "maestro-resources" => &["maestro-models", "maestro-settings", "maestro-packages"],
        "maestro-extensions" => &[
            "maestro-models",
            "maestro-agent",
            "maestro-session",
            "maestro-storage",
        ],
        "maestro-app" => DOMAIN,
        "maestro-cli" => &["maestro-app"],
        "maestro" => CORE,
        _ => return None,
    };
    Some(Rule {
        class: if matches!(name, "maestro" | "maestro-test-conventions") {
            "dedicated"
        } else {
            "core"
        },
        dependencies,
    })
}

pub(crate) fn inventory(
    metadata: &Value,
    list: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let members = array(metadata, "workspace_members")?;
    for package in array(metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let name = string(package, "name")?;
        let policy = rule(name)
            .ok_or_else(|| format!("workspace crate is outside scoped policy: {name}"))?;
        if list[name] != policy.class {
            return Err(format!(
                "invalid scoped layer for {name}: expected {}",
                policy.class
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Normal,
    Build,
    Dev,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Edge {
    from: String,
    to: String,
    kind: Kind,
}
fn kind(value: &Value) -> Result<Kind, String> {
    match value.as_str() {
        None if value.is_null() => Ok(Kind::Normal),
        Some("build") => Ok(Kind::Build),
        Some("dev") => Ok(Kind::Dev),
        _ => Err("invalid cargo metadata: dependency kind".into()),
    }
}

const TEST_SUPPORT_ALLOWLIST: &[&str] = &[];

pub(crate) fn validate(metadata: &Value, edges: &BTreeSet<Edge>) -> Result<(), String> {
    validate_with_support(metadata, edges, TEST_SUPPORT_ALLOWLIST)
}

pub(crate) fn validate_with_support(
    metadata: &Value,
    edges: &BTreeSet<Edge>,
    support: &[&str],
) -> Result<(), String> {
    let members = array(metadata, "workspace_members")?;
    let mut production = BTreeMap::new();
    let mut test = BTreeMap::new();
    for package in array(metadata, "packages")? {
        if members.contains(&package["id"]) {
            production.insert(string(package, "name")?.to_owned(), BTreeSet::new());
            test.insert(string(package, "name")?.to_owned(), BTreeSet::new());
        }
    }
    for edge in edges {
        test.get_mut(&edge.from)
            .ok_or("unknown graph source")?
            .insert(edge.to.clone());
        if edge.kind != Kind::Dev {
            production
                .get_mut(&edge.from)
                .ok_or("unknown graph source")?
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
        let policy = |name: &str| {
            rule(name).or_else(|| {
                support.contains(&name).then_some(Rule {
                    class: "core",
                    dependencies: &[],
                })
            })
        };
        let from = policy(&edge.from).ok_or("unknown scoped source")?;
        let to = policy(&edge.to).ok_or("unknown scoped target")?;
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
        if edge.kind == Kind::Dev {
            let target = array(metadata, "packages")?
                .iter()
                .find(|package| package["name"] == edge.to && members.contains(&package["id"]));
            let dependency_free = match target {
                Some(package) => {
                    array(package, "dependencies")?.is_empty()
                        && !edges.iter().any(|other| other.from == edge.to)
                }
                None => false,
            };
            if !support.contains(&edge.to.as_str()) || !dependency_free {
                return Err(format!(
                    "internal dev dependency requires declared dependency-free test support: {} -> {}",
                    edge.from, edge.to
                ));
            }
        } else if !from.dependencies.contains(&edge.to.as_str()) {
            return Err(format!(
                "forbidden production dependency: {} -> {}",
                edge.from, edge.to
            ));
        }
    }
    Ok(())
}
// Use manifest paths rather than dependency names: Cargo dependencies can be renamed.
// --no-deps lets us inspect cycles ourselves, before Cargo's resolver rejects them.
pub(crate) fn declared(metadata: &Value) -> Result<BTreeSet<Edge>, String> {
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
    let mut edges = BTreeSet::new();
    for package in packages {
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
                edges.insert(Edge {
                    from: string(package, "name")?.into(),
                    to: name.clone(),
                    kind: kind(&dependency["kind"])?,
                });
            }
        }
    }
    Ok(edges)
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

pub(crate) fn resolved(metadata: &Value) -> Result<BTreeSet<Edge>, String> {
    let members = array(metadata, "workspace_members")?;
    let mut packages = BTreeMap::new();
    for package in array(metadata, "packages")? {
        packages.insert(string(package, "id")?, string(package, "name")?);
    }
    for member in members {
        if !packages.contains_key(
            member
                .as_str()
                .ok_or("invalid cargo metadata: member identity")?,
        ) {
            return Err("invalid cargo metadata: missing member package".into());
        }
    }
    let nodes = array(&metadata["resolve"], "nodes")?;
    let mut node_ids = BTreeSet::new();
    for node in nodes {
        let id = string(node, "id")?;
        if !packages.contains_key(id) || !node_ids.insert(id) {
            return Err("invalid cargo metadata: unknown or duplicate resolved node".into());
        }
    }
    if packages.keys().any(|id| !node_ids.contains(id)) {
        return Err("invalid cargo metadata: missing resolved node".into());
    }
    let mut edges = BTreeSet::new();
    for node in nodes {
        let id = string(node, "id")?;
        let from = packages
            .get(id)
            .ok_or("invalid cargo metadata: unknown resolved node")?;
        for dependency in array(node, "deps")? {
            let target = string(dependency, "pkg")?;
            let to = packages
                .get(target)
                .ok_or("invalid cargo metadata: unknown resolved dependency")?;
            string(dependency, "name")?;
            let kinds = array(dependency, "dep_kinds")?;
            if kinds.is_empty() {
                return Err("invalid cargo metadata: empty dependency kinds".into());
            }
            for dependency_kind in kinds {
                let kind = kind(
                    dependency_kind
                        .get("kind")
                        .ok_or("invalid cargo metadata: missing dependency kind")?,
                )?;
                let condition = dependency_kind
                    .get("target")
                    .ok_or("invalid cargo metadata: missing dependency target")?;
                if !condition.is_null() && !condition.is_string() {
                    return Err("invalid cargo metadata: dependency target".into());
                }
                if members.iter().any(|member| member == id)
                    && members.iter().any(|member| member == target)
                {
                    edges.insert(Edge {
                        from: (*from).into(),
                        to: (*to).into(),
                        kind,
                    });
                }
            }
        }
    }
    Ok(edges)
}

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::Metadata;
use serde::Deserialize;

/// Cargo metadata acquisition and identity validation.
pub(crate) mod metadata;
/// Scoped crate ownership and direct-dependency rules.
mod policy;
use metadata::{canonical, check_kind, members};
use policy::{TEST_FIXTURES, UTILITY, rule, test_support_callers, uses_utility};
pub(crate) use policy::{TEST_TRACING, UNIFIER};

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
/// Allowed workspace crate classifications.
enum CrateClass {
    /// A reusable capability crate.
    Core,
    /// A binary, checker, harness or repository-tooling crate.
    Dedicated,
}
#[derive(Deserialize)]
#[serde(untagged)]
/// Decoded classification with a marker for discarded invalid values.
enum Class {
    /// A recognized crate classification.
    Known(CrateClass),
    /// An invalid-classification marker used for a crate-specific diagnostic.
    Invalid(serde::de::IgnoredAny),
}
impl Class {
    /// Return the accepted classification spelling or a crate-specific diagnostic.
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

/// The only target table that may declare the feature unifier.
const NATIVE: &str = "cfg(not(target_arch = \"wasm32\"))";

/// Validate declared and host-resolved workspace graphs.
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
    complete(&declared, &edges, &resolved)?;
    Ok(declared)
}

/// Require every Cargo member to match the named crate inventory.
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

/// Recognize the composition root or an accepted Maestro crate name.
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
/// Dependency category used to separate production and test graphs.
enum Kind {
    /// Ordinary library dependency.
    Normal,
    /// Build-time dependency.
    Build,
    /// Dependency available only to development targets.
    Development,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Directed dependency between named workspace crates.
pub(super) struct Edge {
    /// Name of the depending crate.
    from: String,
    /// Name of the dependency crate.
    to: String,
    /// Cargo dependency category for this edge.
    kind: Kind,
}

/// Resolve declared path dependencies into internal graph edges.
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
            check_library_dependency(package.name.as_str(), name, &kind)?;
            check_native_declaration(package.name.as_str(), member, dependency, &kind)?;
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

/// Enforces the target and kind of dependencies restricted to native declarations.
fn check_native_declaration(
    owner: &str,
    member: Option<&str>,
    dependency: &cargo_metadata::Dependency,
    kind: &Kind,
) -> Result<(), String> {
    if *kind != Kind::Development
        && owner == "maestro-app"
        && member == Some("maestro-lock")
        && dependency
            .target
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            != Some(NATIVE)
    {
        return Err("maestro-app: maestro-lock must be a native dependency".into());
    }
    if member == Some(UNIFIER) && !native_declared(dependency, kind, &Kind::Normal) {
        return Err(format!(
            "{owner}: {UNIFIER} must be a native, normal, non-optional dependency"
        ));
    }
    if let Some(member) =
        member.filter(|name| *name != TEST_FIXTURES && test_support_callers(name).is_some())
        && !native_declared(dependency, kind, &Kind::Development)
    {
        return Err(format!(
            "{owner}: {member} must be a native, non-optional development dependency"
        ));
    }
    if *kind == Kind::Development
        && let Some(member) =
            member.filter(|member| policy::NATIVE_DEVELOPMENT.contains(&(owner, member)))
        && !native_declared(dependency, kind, &Kind::Development)
    {
        return Err(format!(
            "{owner}: {member} test support must be a native, non-optional development dependency"
        ));
    }
    Ok(())
}

/// Report whether a declaration is a non-optional dependency of `wanted` kind on native targets.
fn native_declared(dependency: &cargo_metadata::Dependency, kind: &Kind, wanted: &Kind) -> bool {
    kind == wanted
        && !dependency.optional
        && dependency
            .target
            .as_ref()
            .map(ToString::to_string)
            .as_deref()
            == Some(NATIVE)
}

/// Check graph identities, cycles and each dependency's ownership policy.
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

/// Require a direct edge to obey its source and target crate rules.
fn validate_edge(edge: &Edge) -> Result<(), String> {
    let from = rule(&edge.from).ok_or("unknown scoped source")?;
    let to = rule(&edge.to).ok_or("unknown scoped target")?;
    if edge.to == UNIFIER {
        return Ok(());
    }
    if let Some(callers) = test_support_callers(&edge.to) {
        return if edge.kind == Kind::Development && callers.contains(&edge.from.as_str()) {
            Ok(())
        } else {
            Err(format!(
                "{} is reachable only as a {}development dependency of its declared callers: {} -> {}",
                edge.to,
                if edge.to == TEST_FIXTURES {
                    ""
                } else {
                    "native "
                },
                edge.from,
                edge.to
            ))
        };
    }
    if edge.to == UTILITY && edge.kind != Kind::Development && uses_utility(&edge.from) {
        return Ok(());
    }
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
        if policy::permitted_development(&edge.from, &edge.to) {
            return Ok(());
        }
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

/// Require the executable's mandatory wiring to exist as native runtime dependencies.
fn complete_runtime(metadata: &Metadata, resolved: &Metadata) -> Result<(), String> {
    let Some(root) = members(metadata).find(|package| package.name == "maestro") else {
        return Ok(());
    };
    let native = resolved
        .resolve
        .as_ref()
        .and_then(|resolve| resolve.nodes.iter().find(|node| node.id == root.id));
    for required in policy::required("maestro") {
        let target = members(metadata).find(|package| package.name == *required);
        let active = target
            .zip(native)
            .is_some_and(|(target, node)| runtime_required(root, node, target));
        if !active {
            return Err(format!(
                "maestro: missing required direct dependency {required}"
            ));
        }
    }
    Ok(())
}

/// Match a non-optional normal declaration to the same active native dependency kind.
fn runtime_required(
    root: &cargo_metadata::Package,
    native: &cargo_metadata::Node,
    target: &cargo_metadata::Package,
) -> bool {
    root.dependencies.iter().any(|dependency| {
        dependency.name == target.name.as_str()
            && dependency.kind == cargo_metadata::DependencyKind::Normal
            && !dependency.optional
            && native
                .deps
                .iter()
                .filter(|edge| edge.pkg == target.id)
                .flat_map(|edge| &edge.dep_kinds)
                .any(|kind| kind.kind == dependency.kind && kind.target == dependency.target)
    })
}

/// Require each crate's currently required direct dependencies.
fn complete(
    metadata: &Metadata,
    edges: &BTreeSet<Edge>,
    resolved: &Metadata,
) -> Result<(), String> {
    complete_runtime(metadata, resolved)?;
    for package in members(metadata).filter(|package| package.name != "maestro") {
        let name = package.name.as_str();
        for required in policy::required(name) {
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

/// Detect a cycle using the current traversal path and completed nodes.
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

/// Reject runtime or toolkit libraries outside their owning adapter.
fn check_library_dependency(owner: &str, dependency_name: &str, kind: &Kind) -> Result<(), String> {
    let test_host = owner == crate::generated::GUEST
        && *kind == Kind::Development
        && matches!(dependency_name, "wasmtime" | "wasmtime-wasi");
    if dependency_name == "wasmtime-wasi-http"
        || (dependency_name == "wasmtime" || dependency_name.starts_with("wasmtime-"))
            && owner != "maestro-extensions-wasmtime"
            && !test_host
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

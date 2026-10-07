use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package, PackageId};

use super::{Edge, Kind};

pub(super) fn load(root: &Path, host: Option<&str>) -> Result<Metadata, String> {
    let mut command = MetadataCommand::new();
    command
        .current_dir(root)
        .other_options(vec!["--offline".into()]);
    if let Some(cargo) = std::env::var_os("CARGO") {
        command.cargo_path(cargo);
    }
    if let Some(host) = host {
        command.other_options(vec![
            "--offline".into(),
            "--filter-platform".into(),
            host.into(),
        ]);
    } else {
        command.no_deps();
    }
    let output = command
        .cargo_command()
        .output()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))?;
    MetadataCommand::parse(text).map_err(|error| format!("invalid cargo metadata: {error}"))
}

pub(super) fn compiler_host() -> Result<String, String> {
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
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host:"))
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV failed: missing or empty host line".into())
}

pub(crate) fn members(metadata: &Metadata) -> impl Iterator<Item = &Package> {
    metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
}

pub(super) fn identities(metadata: &Metadata) -> Result<(), String> {
    let mut members = BTreeSet::new();
    for id in &metadata.workspace_members {
        if id.repr.is_empty() || !members.insert(id) {
            return Err("invalid cargo metadata: duplicate or empty member identity".into());
        }
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for package in &metadata.packages {
        if package.id.repr.is_empty() || package.name.is_empty() || !ids.insert(&package.id) {
            return Err("invalid cargo metadata: duplicate or empty package identity".into());
        }
        if members.contains(&package.id) && !names.insert(&package.name) {
            return Err("invalid cargo metadata: duplicate member package name".into());
        }
    }
    if !members.is_subset(&ids) {
        return Err("invalid cargo metadata: missing member package".into());
    }
    Ok(())
}

pub(super) fn canonical(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))
}

pub(super) fn check_kind(kind: DependencyKind) -> Result<Kind, String> {
    match kind {
        DependencyKind::Normal => Ok(Kind::Normal),
        DependencyKind::Build => Ok(Kind::Build),
        DependencyKind::Development => Ok(Kind::Development),
        _ => Err("invalid cargo metadata: dependency kind".into()),
    }
}

pub(super) fn resolved(metadata: &Metadata, declared: &Metadata) -> Result<BTreeSet<Edge>, String> {
    identities(metadata)?;
    let packages: BTreeMap<_, _> = metadata.packages.iter().map(|p| (&p.id, p)).collect();
    if metadata.workspace_members != declared.workspace_members
        || members(declared).any(|p| {
            packages
                .get(&p.id)
                .is_none_or(|other| other.name != p.name || other.manifest_path != p.manifest_path)
        })
    {
        return Err("invalid cargo metadata: inconsistent member identity".into());
    }
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or("invalid cargo metadata: missing resolve")?;
    let mut nodes = BTreeSet::new();
    for node in &resolve.nodes {
        if !packages.contains_key(&node.id) || !nodes.insert(&node.id) {
            return Err("invalid cargo metadata: unknown or duplicate resolved node".into());
        }
    }
    if packages.keys().any(|id| !nodes.contains(id)) {
        return Err("invalid cargo metadata: missing resolved node".into());
    }
    let mut edges = BTreeSet::new();
    for node in &resolve.nodes {
        resolved_edges(node, &packages, &metadata.workspace_members, &mut edges)?;
    }
    Ok(edges)
}

fn resolved_edges(
    node: &cargo_metadata::Node,
    packages: &BTreeMap<&PackageId, &Package>,
    members: &[PackageId],
    edges: &mut BTreeSet<Edge>,
) -> Result<(), String> {
    let from = packages
        .get(&node.id)
        .ok_or("invalid cargo metadata: unknown graph source")?;
    for dependency in &node.deps {
        let to = packages
            .get(&dependency.pkg)
            .ok_or("invalid cargo metadata: unknown resolved dependency")?;
        if dependency.dep_kinds.is_empty() {
            return Err("invalid cargo metadata: empty dependency kinds".into());
        }
        for kind in &dependency.dep_kinds {
            let kind = check_kind(kind.kind)?;
            if members.contains(&node.id) && members.contains(&dependency.pkg) {
                edges.insert(Edge {
                    from: from.name.to_string(),
                    to: to.name.to_string(),
                    kind,
                });
            }
        }
    }
    Ok(())
}

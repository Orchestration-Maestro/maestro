//! The path-dependency graph of the workspace members, read from Cargo metadata.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use cargo_metadata::{Dependency, Metadata, Package};
use maestro_path::normalize_native;

/// Workspace members in metadata package order.
pub(crate) fn members(metadata: &Metadata) -> Vec<&Package> {
    let workspace: HashSet<&str> = metadata
        .workspace_members
        .iter()
        .map(|id| id.repr.as_str())
        .collect();
    metadata
        .packages
        .iter()
        .filter(|package| workspace.contains(package.id.repr.as_str()))
        .collect()
}

/// Index each member by its manifest directory.
pub(crate) fn member_roots(members: &[&Package]) -> HashMap<PathBuf, usize> {
    members
        .iter()
        .enumerate()
        .filter_map(|(member, package)| {
            let root = package.manifest_path.parent()?;
            Some((normalize_native(root.as_std_path()), member))
        })
        .collect()
}

/// For each member, the path dependencies that `keep` accepts.
pub(crate) fn dependency_edges_where(
    members: &[&Package],
    roots: &HashMap<PathBuf, usize>,
    keep: impl Fn(&Dependency) -> bool,
) -> Vec<Vec<usize>> {
    members
        .iter()
        .map(|package| {
            package
                .dependencies
                .iter()
                .filter(|dependency| keep(dependency))
                .filter_map(|dependency| dependency.path.as_deref())
                .filter_map(|path| roots.get(&normalize_native(path.as_std_path())).copied())
                .collect()
        })
        .collect()
}

/// Walk the edges once, visiting each reached member once.
pub(crate) fn closure(edges: &[Vec<usize>], start: HashSet<usize>) -> HashSet<usize> {
    let mut selected = start;
    let mut pending: Vec<usize> = selected.iter().copied().collect();
    while let Some(owner) = pending.pop() {
        for &next in &edges[owner] {
            if selected.insert(next) {
                pending.push(next);
            }
        }
    }
    selected
}

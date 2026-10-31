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

/// For each member, the members it declares a path dependency on, of any kind.
pub(crate) fn dependency_edges(
    members: &[&Package],
    roots: &HashMap<PathBuf, usize>,
) -> Vec<Vec<usize>> {
    dependency_edges_where(members, roots, |_| true)
}

/// Like `dependency_edges`, keeping only the dependencies `keep` accepts.
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

/// For each member, the members that declare a path dependency on it.
pub(crate) fn reverse_edges(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (caller, targets) in edges.iter().enumerate() {
        for &target in targets {
            reverse[target].push(caller);
        }
    }
    reverse
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

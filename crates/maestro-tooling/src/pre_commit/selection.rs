//! Select the workspace members a staged change can affect.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use cargo_metadata::{Metadata, Package};

#[cfg(test)]
mod tests;

/// Crates whose verification a staged change requires.
#[derive(Debug, Eq, PartialEq)]
pub(super) enum Scope {
    /// Every workspace member, named in metadata order.
    Workspace(Vec<String>),
    /// The required members, named in metadata order.
    Packages(Vec<String>),
}

impl Scope {
    /// Names of the selected members in metadata order.
    pub(super) fn names(&self) -> &[String] {
        match self {
            Self::Workspace(names) | Self::Packages(names) => names,
        }
    }

    /// Whether the selection compiles the delivered browser entry.
    pub(super) fn reaches_browser(&self) -> bool {
        matches!(self, Self::Workspace(_))
            || self
                .names()
                .iter()
                .any(|name| matches!(name.as_str(), "maestro-models" | "maestro-web"))
    }
}

/// Select the changed members and every transitive reverse dependent.
pub(super) fn select(metadata: &Metadata, paths: &[OsString]) -> Scope {
    let members = metadata.workspace_packages();
    let roots = member_roots(&members);
    let Some(changed) = changed_owners(&roots, metadata.workspace_root.as_std_path(), paths) else {
        return Scope::Workspace(names(&members));
    };
    let selected = closure(&reverse_edges(&members, &roots), changed);
    Scope::Packages(
        members
            .iter()
            .enumerate()
            .filter(|(member, _)| selected.contains(member))
            .map(|(_, package)| package.name.to_string())
            .collect(),
    )
}

/// Names of every workspace member in metadata order.
fn names(members: &[&Package]) -> Vec<String> {
    members
        .iter()
        .map(|package| package.name.to_string())
        .collect()
}

/// Index each member by its manifest directory.
fn member_roots(members: &[&Package]) -> HashMap<PathBuf, usize> {
    members
        .iter()
        .enumerate()
        .filter_map(|(member, package)| {
            let root = package.manifest_path.parent()?;
            Some((normalize(root.as_std_path()), member))
        })
        .collect()
}

/// Classify the staged paths, or `None` when a change reaches beyond one member.
fn changed_owners(
    roots: &HashMap<PathBuf, usize>,
    workspace_root: &Path,
    paths: &[OsString],
) -> Option<HashSet<usize>> {
    if paths.is_empty() {
        return None;
    }
    paths
        .iter()
        .map(|path| owner(roots, workspace_root, Path::new(path)))
        .collect()
}

/// Find the deepest member root holding a staged path.
fn owner(roots: &HashMap<PathBuf, usize>, workspace_root: &Path, path: &Path) -> Option<usize> {
    if path.file_name() == Some(OsStr::new("Cargo.toml")) || is_conventions(path) {
        return None;
    }
    normalize(&workspace_root.join(path))
        .ancestors()
        .find_map(|ancestor| roots.get(ancestor).copied())
}

/// Whether a staged path changes the workspace-wide conventions checker.
fn is_conventions(path: &Path) -> bool {
    path.starts_with("crates/maestro-test-conventions")
}

/// Index every member that declares a path dependency on another member.
fn reverse_edges(members: &[&Package], roots: &HashMap<PathBuf, usize>) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); members.len()];
    for (caller, package) in members.iter().enumerate() {
        for dependency in &package.dependencies {
            let Some(path) = dependency.path.as_deref() else {
                continue;
            };
            if let Some(&target) = roots.get(&normalize(path.as_std_path())) {
                reverse[target].push(caller);
            }
        }
    }
    reverse
}

/// Walk the reverse edges once, visiting each reached member once.
fn closure(reverse: &[Vec<usize>], changed: HashSet<usize>) -> HashSet<usize> {
    let mut selected = changed;
    let mut pending: Vec<usize> = selected.iter().copied().collect();
    while let Some(owner) = pending.pop() {
        for &caller in &reverse[owner] {
            if selected.insert(caller) {
                pending.push(caller);
            }
        }
    }
    selected
}

/// Reduce `.` and `..` components without reading the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut parts: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(parts.last(), Some(Component::Normal(_))) => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.iter().collect()
}

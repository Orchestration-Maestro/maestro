//! Select the workspace members a staged change can affect.

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, Package};
use maestro_path::normalize_native;

use crate::member_graph::{closure, dependency_edges, member_roots, members, reverse_edges};

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
    let members = members(metadata);
    let roots = member_roots(&members);
    let Some(changed) = changed_owners(&roots, metadata.workspace_root.as_std_path(), paths) else {
        return Scope::Workspace(names(&members));
    };
    let selected = closure(&reverse_edges(&dependency_edges(&members, &roots)), changed);
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
    if path.file_name() == Some(OsStr::new("Cargo.toml")) || super::is_conventions(path) {
        return None;
    }
    normalize_native(&workspace_root.join(path))
        .ancestors()
        .find_map(|ancestor| roots.get(ancestor).copied())
}

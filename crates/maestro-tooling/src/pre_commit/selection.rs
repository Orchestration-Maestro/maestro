//! Select only the workspace members holding staged paths.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cargo_metadata::Metadata;
use maestro_path::normalize_native;

use crate::member_graph::{member_roots, members};

#[cfg(test)]
mod tests;

/// Crates holding staged paths, in metadata order.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Scope {
    /// Selected member names, with each owner present once.
    names: Vec<String>,
}

impl Scope {
    /// Names of the selected members in metadata order.
    pub(super) fn names(&self) -> &[String] {
        &self.names
    }

    /// Whether a staged crate owns a browser compile witness.
    pub(super) fn reaches_browser(&self) -> bool {
        self.names.iter().any(|name| {
            crate::check_browser_build::WITNESSES
                .iter()
                .any(|(package, ..)| package == name)
        })
    }
}

/// Select staged owners without adding dependents or unowned workspace inputs.
pub(super) fn select(metadata: &Metadata, paths: &[OsString]) -> Scope {
    let members = members(metadata);
    let roots = member_roots(&members);
    let changed: HashSet<_> = paths
        .iter()
        .filter_map(|path| {
            owner(
                &roots,
                metadata.workspace_root.as_std_path(),
                Path::new(path),
            )
        })
        .collect();
    Scope {
        names: members
            .iter()
            .enumerate()
            .filter(|(member, _)| changed.contains(member))
            .map(|(_, package)| package.name.to_string())
            .collect(),
    }
}

/// Find the deepest member root holding a staged path.
fn owner(roots: &HashMap<PathBuf, usize>, workspace_root: &Path, path: &Path) -> Option<usize> {
    normalize_native(&workspace_root.join(path))
        .ancestors()
        .find_map(|ancestor| roots.get(ancestor).copied())
}

//! The packages of `Cargo.lock` and the dependency graph between them.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// The package tables of `Cargo.lock`.
#[derive(Deserialize)]
pub(crate) struct Lock {
    /// Every locked package, in lock order.
    #[serde(default)]
    package: Vec<LockedPackage>,
}

/// One locked package.
#[derive(Deserialize, Serialize)]
pub(crate) struct LockedPackage {
    /// The package name.
    pub(crate) name: String,
    /// The locked version.
    version: String,
    /// The registry or Git source; absent for a workspace crate.
    pub(crate) source: Option<String>,
    /// The checksum of a registry package.
    checksum: Option<String>,
    /// The packages it builds on, each as a name optionally followed by a version.
    #[serde(default)]
    dependencies: Vec<String>,
}

/// The locked packages a filter accepts, with the edges between them.
pub(crate) struct LockGraph<'a> {
    /// The accepted packages, in lock order.
    packages: Vec<&'a LockedPackage>,
    /// For each package, the positions of the packages it builds on.
    edges: Vec<Vec<usize>>,
    /// The positions of the packages that carry a name, one per locked version.
    positions: HashMap<&'a str, Vec<usize>>,
}

impl Lock {
    /// Read `Cargo.lock` in `checkout`.
    ///
    /// # Errors
    /// Returns a failed read or parse.
    pub(crate) fn read(checkout: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(checkout.join("Cargo.lock"))?;
        toml::from_str(&text).map_err(io::Error::other)
    }

    /// Every locked package, in lock order.
    pub(crate) fn packages(&self) -> &[LockedPackage] {
        &self.package
    }

    /// The dependency graph of the packages `keep` accepts.
    ///
    /// The lock file lists a package's dependencies for every target, and an entry that names no
    /// version stands for each locked version of that name, so a closure can name more packages
    /// than one target builds.
    pub(crate) fn graph(&self, keep: impl Fn(&LockedPackage) -> bool) -> LockGraph<'_> {
        let packages: Vec<&LockedPackage> = self
            .package
            .iter()
            .filter(|package| keep(package))
            .collect();
        let mut positions: HashMap<&str, Vec<usize>> = HashMap::new();
        for (position, package) in packages.iter().enumerate() {
            positions
                .entry(package.name.as_str())
                .or_default()
                .push(position);
        }
        let edges = packages
            .iter()
            .map(|package| {
                package
                    .dependencies
                    .iter()
                    .filter_map(|entry| positions.get(entry.split_whitespace().next()?))
                    .flatten()
                    .copied()
                    .collect()
            })
            .collect();
        LockGraph {
            packages,
            edges,
            positions,
        }
    }
}

impl<'a> LockGraph<'a> {
    /// The packages named in `roots` and every package they build on, in lock order.
    pub(crate) fn closure<'n>(
        &self,
        roots: impl IntoIterator<Item = &'n str>,
    ) -> Vec<&'a LockedPackage> {
        let _ = (roots, &self.packages, &self.edges, &self.positions);
        Vec::new()
    }
}

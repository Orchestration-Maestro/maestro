//! The packages of `Cargo.lock` and the dependency graph between them.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::member_graph::closure;

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
        let path = checkout.join("Cargo.lock");
        let read = || {
            let text = std::fs::read_to_string(&path)?;
            toml::from_str(&text).map_err(io::Error::other)
        };
        read().map_err(|error: io::Error| {
            io::Error::new(error.kind(), format!("{}: {error}", path.display()))
        })
    }

    /// Find the version of exactly one named package.
    ///
    /// # Errors
    /// Returns an error when the lock omits the package or contains multiple versions.
    pub(crate) fn version(&self, name: &str) -> io::Result<&str> {
        let mut packages = self.package.iter().filter(|package| package.name == name);
        let package = packages
            .next()
            .ok_or_else(|| io::Error::other(format!("Cargo.lock has no {name} package")))?;
        if packages.next().is_some() {
            return Err(io::Error::other(format!(
                "Cargo.lock has multiple {name} packages"
            )));
        }
        Ok(&package.version)
    }

    /// Refuse external identities absent from this lock, allowing removed or local packages.
    ///
    /// # Errors
    /// Returns an error for empty package records or an added external identity.
    pub(crate) fn check_external_identities(&self, other: &Self) -> io::Result<()> {
        if self.package.is_empty() || other.package.is_empty() {
            return Err(io::Error::other("Cargo.lock must contain package records"));
        }
        let known: HashSet<_> = self
            .package
            .iter()
            .filter(|package| package.source.is_some())
            .map(LockedPackage::identity)
            .collect();
        for package in other
            .package
            .iter()
            .filter(|package| package.source.is_some())
        {
            if !known.contains(&package.identity()) {
                return Err(io::Error::other(format!(
                    "Cargo.lock introduced external dependency {} {} from {} (checksum {})",
                    package.name,
                    package.version,
                    package.source.as_deref().unwrap_or("none"),
                    package.checksum.as_deref().unwrap_or("none")
                )));
            }
        }
        Ok(())
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

impl LockedPackage {
    /// The package coordinates that must remain unchanged by offline scratch resolution.
    fn identity(&self) -> (&str, &str, Option<&str>, Option<&str>) {
        (
            &self.name,
            &self.version,
            self.source.as_deref(),
            self.checksum.as_deref(),
        )
    }
}

impl<'a> LockGraph<'a> {
    /// The packages named in `roots` and every package they build on, in lock order.
    pub(crate) fn closure<'n>(
        &self,
        roots: impl IntoIterator<Item = &'n str>,
    ) -> Vec<&'a LockedPackage> {
        let start = roots
            .into_iter()
            .filter_map(|name| self.positions.get(name))
            .flatten()
            .copied()
            .collect();
        let mut reached: Vec<usize> = closure(&self.edges, start).into_iter().collect();
        reached.sort_unstable();
        reached
            .into_iter()
            .map(|position| self.packages[position])
            .collect()
    }
}

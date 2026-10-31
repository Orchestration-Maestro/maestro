//! The inputs each artifact of the test bundle is prepared from, read from the workspace.
//!
//! The components build for WebAssembly from one package and its examples, the setup example and
//! the development tool for the host from two packages, and the browser graph from two others.
//! Each artifact hashes the closure of its own packages, so a change to another artifact's
//! crates leaves it valid.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use cargo_metadata::{Dependency, DependencyKind, Metadata, Package};

use super::inputs::{Inputs, locked_closure};
use super::{
    BROWSER_PACKAGES, COMPONENT_PACKAGE, EXAMPLE_FOLDERS, EXAMPLE_INPUTS, LOCKED_INPUTS,
    SHARED_INPUTS, TOOL_PACKAGES,
};
use crate::example_files::{FIXED_EXAMPLES, FIXED_VARIABLES, visit};
use crate::member_graph::{closure, dependency_edges_where, member_roots, members};

/// The workspace-level gate that marks an edge as native only.
const NATIVE_ONLY: &str = "cfg(not(target_arch = \"wasm32\"))";

/// The pathspecs each artifact hashes.
pub(super) struct Plan {
    /// What every component shares.
    pub(super) shared: BTreeSet<String>,
    /// The registry packages the components depend on directly.
    pub(super) dependencies: BTreeSet<String>,
    /// What the setup example and the development tool read.
    pub(super) tool: BTreeSet<String>,
    /// What the browser graph reads.
    pub(super) browser: BTreeSet<String>,
}

/// The inputs of the artifacts that do not belong to one component.
pub(super) struct Artifacts {
    /// What every component shares.
    pub(super) shared: Inputs,
    /// What the setup example and the development tool read.
    pub(super) tool: Inputs,
    /// What the browser graph reads.
    pub(super) browser: Inputs,
}

impl Artifacts {
    /// Digest the inputs of each artifact in `checkout`.
    ///
    /// # Errors
    /// Returns failed metadata, a missing built package, or a failed listing or read.
    pub(super) fn read(cargo: &Path, checkout: &Path) -> io::Result<Self> {
        let plan = plan(&crate::locked_metadata(cargo, checkout)?)?;
        let locked = locked_closure(checkout, &plan.dependencies)?;
        Ok(Self {
            shared: Inputs::new(checkout, plan.shared, locked)?,
            tool: Inputs::new(checkout, plan.tool, BTreeSet::new())?,
            browser: Inputs::new(checkout, plan.browser, BTreeSet::new())?,
        })
    }

    /// Whether `checkout` still holds what every artifact was digested from.
    ///
    /// # Errors
    /// Returns a failed listing or read.
    pub(super) fn is_current(&self, checkout: &Path) -> io::Result<bool> {
        Ok(self.shared.is_current(checkout)?
            && self.tool.is_current(checkout)?
            && self.browser.is_current(checkout)?)
    }
}

/// Whether the component build uses a dependency.
///
/// shortcut: only the workspace's native-only gate is recognised, and any other platform gate
/// counts as used, which at worst rebuilds the components for a change they ignore.
fn used_by_wasm(dependency: &Dependency) -> bool {
    dependency.kind == DependencyKind::Build
        || dependency
            .target
            .as_ref()
            .is_none_or(|platform| platform.to_string() != NATIVE_ONLY)
}

/// The workspace packages `built` names and those they depend on through the edges `keep`
/// accepts.
fn closure_packages<'a>(
    metadata: &'a Metadata,
    built: &[&str],
    keep: impl Fn(&Dependency) -> bool,
) -> io::Result<Vec<&'a Package>> {
    let members = members(metadata);
    let roots = member_roots(&members);
    let start = built
        .iter()
        .map(|name| {
            members
                .iter()
                .position(|package| package.name == *name)
                .ok_or_else(|| io::Error::other(format!("{name} is not a workspace package")))
        })
        .collect::<io::Result<_>>()?;
    Ok(
        closure(&dependency_edges_where(&members, &roots, keep), start)
            .into_iter()
            .map(|member| members[member])
            .collect(),
    )
}

/// The repository-relative directories of `packages`.
fn directories(metadata: &Metadata, packages: &[&Package]) -> BTreeSet<String> {
    packages
        .iter()
        .filter_map(|package| {
            let directory = package.manifest_path.parent()?;
            let relative = directory.strip_prefix(&metadata.workspace_root).ok()?;
            Some(
                relative
                    .components()
                    .map(|part| part.as_str())
                    .collect::<Vec<_>>()
                    .join("/"),
            )
        })
        .collect()
}

/// The directories of the named workspace packages and of the packages they depend on.
pub(super) fn closure_directories(
    metadata: &Metadata,
    built: &[&str],
) -> io::Result<BTreeSet<String>> {
    Ok(directories(
        metadata,
        &closure_packages(metadata, built, |_| true)?,
    ))
}

/// The pathspecs of the crates of `built` and what `fixed` names.
fn crate_paths(
    metadata: &Metadata,
    built: &[&str],
    fixed: impl Iterator<Item = &'static str>,
) -> io::Result<BTreeSet<String>> {
    Ok(closure_directories(metadata, built)?
        .into_iter()
        .chain(fixed.map(str::to_owned))
        .collect())
}

/// What each artifact hashes, from the workspace metadata.
pub(super) fn plan(metadata: &Metadata) -> io::Result<Plan> {
    let components = closure_packages(metadata, &[COMPONENT_PACKAGE], used_by_wasm)?;
    let dependencies = components
        .iter()
        .flat_map(|package| &package.dependencies)
        .filter(|dependency| dependency.path.is_none() && used_by_wasm(dependency))
        .map(|dependency| dependency.name.clone())
        .collect();
    // The examples are hashed one by one, beside the shared inputs.
    let mut shared = directories(metadata, &components);
    shared.extend(SHARED_INPUTS.map(str::to_owned));
    shared.insert(format!(":(exclude){}", EXAMPLE_FOLDERS[0]));
    let fixed = SHARED_INPUTS.into_iter().chain(LOCKED_INPUTS);
    Ok(Plan {
        shared,
        dependencies,
        tool: crate_paths(
            metadata,
            &TOOL_PACKAGES,
            fixed.clone().chain(EXAMPLE_INPUTS),
        )?,
        browser: crate_paths(metadata, &BROWSER_PACKAGES, fixed)?,
    })
}

/// The example target each component variable exports.
///
/// # Errors
/// Returns a failed discovery of the marked components.
pub(super) fn component_names(checkout: &Path) -> io::Result<BTreeMap<String, String>> {
    let mut names: BTreeMap<String, String> = FIXED_VARIABLES
        .into_iter()
        .zip(FIXED_EXAMPLES)
        .map(|(variable, name)| (variable.to_owned(), name.to_owned()))
        .collect();
    let folders: Vec<_> = EXAMPLE_FOLDERS
        .iter()
        .map(|folder| checkout.join(folder))
        .filter(|folder| folder.is_dir())
        .collect();
    visit(&folders, |name, variable| {
        names.insert(variable.to_owned(), name.to_owned());
        Ok(())
    })?;
    Ok(names)
}

/// The inputs of the example source of the component target `name`, if it has one: its file, its
/// module directory or its directory with an entry point.
///
/// # Errors
/// Returns a failed listing or read.
pub(super) fn own_inputs(checkout: &Path, name: Option<&str>) -> io::Result<Inputs> {
    let paths = name
        .into_iter()
        .flat_map(|name| {
            EXAMPLE_FOLDERS.into_iter().flat_map(move |folder| {
                [
                    format!("{folder}/{name}.rs"),
                    format!("{folder}/{name}"),
                    format!("{folder}/{}", name.replace('_', "-")),
                ]
            })
        })
        .collect();
    Inputs::new(checkout, paths, BTreeSet::new())
}

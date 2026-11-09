//! The workspace a development command serves, located from the directory it starts in.
//!
//! A development command is installed once and started in the workspace it prepares, so nothing
//! here reads the directory the command was compiled in. The crates the workspace holds are the
//! members Cargo resolves: a nested fixture workspace and a manifest the root manifest leaves out
//! are not its crates. A package it only depends on is reached through Cargo's resolved
//! metadata, whose checkout sits outside the workspace.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use cargo_metadata::Metadata;

use crate::member_graph::members;

/// The marker that opens the generated example region in a crate's manifest.
pub const REGION_START: &str = "# example-index:start";
/// The marker that closes that region.
pub const REGION_END: &str = "# example-index:end";
/// The shipped extension folder, beside the workspace manifest.
const SHIPPED: &str = "examples/extensions";
/// The standalone guide fixture, under the crate that holds one.
pub(crate) const GUIDE: &str = "tests/fixtures/hello-extension";
/// The fixture components of the component crate, relative to that crate.
pub(crate) const FIXTURES: &str = "tests/support";

/// The workspace root: the nearest directory at or above `start` whose manifest declares one.
///
/// # Errors
/// Returns an error when no directory at or above `start` holds a workspace manifest.
pub fn root(start: &Path) -> io::Result<PathBuf> {
    for directory in start.ancestors() {
        if fs::read_to_string(directory.join("Cargo.toml"))
            .is_ok_and(|text| declares_workspace(&text))
        {
            return Ok(directory.to_path_buf());
        }
    }
    Err(io::Error::other(format!(
        "{} is not inside a workspace",
        start.display()
    )))
}

/// Whether a manifest declares the workspace its directory roots.
fn declares_workspace(text: &str) -> bool {
    text.parse::<toml::Table>()
        .is_ok_and(|table| table.contains_key("workspace"))
}

/// The member crate whose manifest holds the generated example region.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ComponentCrate {
    /// The crate directory.
    pub(crate) directory: PathBuf,
    /// The crate's package name, which selects its examples in a Cargo build.
    pub(crate) package: String,
}

/// A workspace: its root, the Cargo that resolves it, and that resolution.
pub(crate) struct Workspace {
    /// The workspace root directory.
    root: PathBuf,
    /// The Cargo executable that resolves the workspace.
    cargo: PathBuf,
    /// The workspace resolution: its members and their manifests.
    metadata: Metadata,
}

impl Workspace {
    /// Read the members of the workspace at `root` with `cargo`.
    ///
    /// # Errors
    /// Returns a failed Cargo execution or an unreadable resolution.
    pub(crate) fn read(cargo: &Path, root: &Path) -> io::Result<Self> {
        let metadata = crate::locked_metadata(cargo, root)?;
        Ok(Self {
            root: metadata.workspace_root.clone().into_std_path_buf(),
            cargo: cargo.to_path_buf(),
            metadata,
        })
    }

    /// The root directory the workspace was read at.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The Cargo executable that resolves the workspace.
    pub(crate) fn cargo(&self) -> &Path {
        &self.cargo
    }

    /// The resolved members and their manifests.
    pub(crate) fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// The names among `names` that this workspace holds, in their original order.
    ///
    /// A package the workspace only depends on is built by Cargo outside the checkout, so it
    /// contributes no crate directory to fingerprint.
    pub(crate) fn holds<'a>(&self, names: &[&'a str]) -> Vec<&'a str> {
        names
            .iter()
            .copied()
            .filter(|name| self.member(name).is_some())
            .collect()
    }

    /// The directory of the member crate whose package is `name`.
    pub(crate) fn member(&self, name: &str) -> Option<PathBuf> {
        members(&self.metadata)
            .into_iter()
            .find(|package| package.name == name)
            .and_then(|package| package.manifest_path.parent())
            .map(|directory| directory.as_std_path().to_path_buf())
    }

    /// The directory of the package `name`, which this workspace holds or depends on.
    ///
    /// # Errors
    /// Returns a failed resolution of the dependency packages.
    pub(crate) fn package(&self, name: &str) -> io::Result<Option<PathBuf>> {
        if let Some(directory) = self.member(name) {
            return Ok(Some(directory));
        }
        let metadata = crate::locked_dependencies(&self.cargo, &self.root)?;
        Ok(metadata
            .packages
            .iter()
            .find(|package| package.name == name)
            .and_then(|package| package.manifest_path.parent())
            .map(|directory| directory.as_std_path().to_path_buf()))
    }

    /// The crate whose manifest holds the generated example region.
    ///
    /// # Errors
    /// Returns an unreadable manifest, or an error naming the crates when none, or more than
    /// one, holds the region.
    pub(crate) fn component_crate(&self) -> io::Result<ComponentCrate> {
        self.find_component_crate()?.ok_or_else(|| {
            io::Error::other(format!(
                "no member crate of {} holds {REGION_START}",
                self.root.display()
            ))
        })
    }

    /// The crate whose manifest holds the generated example region, when one does.
    ///
    /// # Errors
    /// Returns an unreadable manifest, or an error naming the crates when more than one holds
    /// the region.
    pub(crate) fn find_component_crate(&self) -> io::Result<Option<ComponentCrate>> {
        let mut marked = Vec::new();
        for member in members(&self.metadata) {
            let manifest = member.manifest_path.as_std_path();
            let text = fs::read_to_string(manifest)
                .map_err(|error| io::Error::other(format!("{}: {error}", manifest.display())))?;
            if text.contains(REGION_START) && text.contains(REGION_END) {
                marked.push(ComponentCrate {
                    directory: directory(manifest),
                    package: member.name.to_string(),
                });
            }
        }
        if marked.len() > 1 {
            let directories: Vec<PathBuf> = marked
                .into_iter()
                .map(|component| component.directory)
                .collect();
            return Err(io::Error::other(format!(
                "more than one member crate of {} holds {REGION_START}: {}",
                self.root.display(),
                self.names(&directories)?
            )));
        }
        Ok(marked.pop())
    }

    /// The member crate that holds the standalone guide fixture.
    pub(crate) fn guide(&self) -> Option<PathBuf> {
        members(&self.metadata).into_iter().find_map(|member| {
            let crate_root = member.manifest_path.parent()?.as_std_path().to_path_buf();
            crate_root.join(GUIDE).is_dir().then_some(crate_root)
        })
    }

    /// The shipped extension folder, when this workspace has one.
    pub(crate) fn shipped(&self) -> Option<PathBuf> {
        let folder = self.root.join(SHIPPED);
        folder.is_dir().then_some(folder)
    }

    /// The workspace-relative name of `path`, with `/` separators.
    ///
    /// # Errors
    /// Returns a path outside the workspace or one that is not UTF-8.
    pub(crate) fn relative(&self, path: &Path) -> io::Result<String> {
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            io::Error::other(format!("{} is outside the workspace", path.display()))
        })?;
        let parts: Vec<&str> = relative
            .components()
            .map(|part| match part {
                Component::Normal(name) => name.to_str(),
                _ => None,
            })
            .collect::<Option<_>>()
            .ok_or_else(|| io::Error::other(format!("{} is not a plain path", path.display())))?;
        Ok(parts.join("/"))
    }

    /// The parent reference from `directory` back to the workspace root.
    pub(crate) fn climb(&self, directory: &Path) -> io::Result<String> {
        let depth = directory
            .strip_prefix(&self.root)
            .map_err(|_| {
                io::Error::other(format!("{} is outside the workspace", directory.display()))
            })?
            .components()
            .count();
        Ok("../".repeat(depth))
    }

    /// The names of the crates at `directories`, for an error that names them.
    fn names(&self, directories: &[PathBuf]) -> io::Result<String> {
        directories
            .iter()
            .map(|directory| self.relative(directory))
            .collect::<io::Result<Vec<_>>>()
            .map(|names| names.join(", "))
    }
}

/// The directory a manifest sits in.
fn directory(manifest: &Path) -> PathBuf {
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    use maestro_test_directory::Scratch;

    /// Write a manifest and the source file its package needs.
    fn crate_at(root: &Path, relative: &str, manifest: &str) {
        let directory = root.join(relative);
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::write(directory.join("Cargo.toml"), manifest).unwrap();
        std::fs::write(directory.join("src/lib.rs"), "").unwrap();
    }

    /// The manifest of a crate whose region the generator owns.
    fn marked(name: &str) -> String {
        format!("[package]\nname = \"{name}\"\n{REGION_START}\n{REGION_END}\n")
    }

    /// A workspace holding one marked crate and an unmarked one.
    fn fixture() -> Scratch {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        crate_at(root, "crates/component", &marked("example-components"));
        crate_at(
            root,
            "crates/plain",
            "[package]\nname = \"plain\"\nversion = \"0.1.0\"\n",
        );
        scratch
    }

    /// Read the workspace of `root` with the Cargo that built the tests.
    fn read(root: &Path) -> Workspace {
        Workspace::read(Path::new(env!("CARGO")), root).unwrap()
    }

    /// A command started below the workspace finds the root the manifests sit under.
    #[test]
    fn root_is_the_nearest_directory_whose_manifest_declares_a_workspace() {
        let scratch = fixture();
        let workspace = scratch.path().canonicalize().unwrap();
        crate_at(
            &workspace,
            "crates/component/examples",
            "[package]\nname = \"nested\"\n",
        );
        assert_eq!(
            super::root(&workspace.join("crates/component/src")).unwrap(),
            workspace
        );
        assert!(super::root(Path::new("/")).is_err());
    }

    /// Exactly one marked manifest names the component crate and its package, so a rename cannot
    /// pick another; without one the workspace has no component crate.
    #[test]
    fn the_component_crate_is_the_one_marked_manifest() {
        let scratch = fixture();
        let workspace = read(scratch.path());
        assert_eq!(
            workspace.component_crate().unwrap(),
            ComponentCrate {
                directory: scratch.path().join("crates/component"),
                package: "example-components".to_owned(),
            }
        );
        crate_at(scratch.path(), "crates/second", &marked("second"));
        let error = read(scratch.path())
            .component_crate()
            .unwrap_err()
            .to_string();
        assert!(error.contains("crates/component, crates/second"), "{error}");
        let unmarked = "[package]\nname = \"second\"\n";
        fs::write(scratch.path().join("crates/second/Cargo.toml"), unmarked).unwrap();
        fs::write(
            scratch.path().join("crates/component/Cargo.toml"),
            unmarked.replace("second", "component"),
        )
        .unwrap();
        let workspace = read(scratch.path());
        assert_eq!(workspace.find_component_crate().unwrap(), None);
        let error = workspace.component_crate().unwrap_err().to_string();
        assert!(error.contains("no member crate"), "{error}");
    }

    /// A manifest below a member crate is not a crate of the workspace, so it cannot declare the
    /// region or answer for a package.
    #[test]
    fn a_nested_fixture_manifest_is_not_a_workspace_crate() {
        let scratch = fixture();
        let root = scratch.path();
        crate_at(root, "crates/plain/tests/fixtures/nested", &marked("plain"));
        let workspace = read(root);
        assert_eq!(
            workspace.member("plain").unwrap(),
            root.join("crates/plain")
        );
        assert_eq!(
            workspace.component_crate().unwrap().directory,
            root.join("crates/component")
        );
    }

    /// A package outside the workspace is absent, which is what skips the steps tied to it.
    #[test]
    fn only_the_manifests_the_workspace_holds_name_a_package() {
        let scratch = fixture();
        let workspace = read(scratch.path());
        assert_eq!(
            workspace.member("plain").unwrap(),
            scratch.path().join("crates/plain")
        );
        assert_eq!(workspace.member("maestro-models"), None);
        assert_eq!(workspace.holds(&["plain", "maestro-models"]), ["plain"]);
    }

    /// The guide fixture and the shipped folder are the workspace's own, when it has them.
    #[test]
    fn the_guide_fixture_and_the_shipped_folder_are_optional() {
        let scratch = fixture();
        let root = scratch.path();
        let mut workspace = read(root);
        assert_eq!(workspace.guide(), None);
        assert_eq!(workspace.shipped(), None);
        std::fs::create_dir_all(root.join("crates/guide").join(GUIDE)).unwrap();
        crate_at(root, "crates/guide", "[package]\nname = \"guide\"\n");
        std::fs::create_dir_all(root.join(SHIPPED)).unwrap();
        workspace = read(root);
        assert_eq!(workspace.guide().unwrap(), root.join("crates/guide"));
        assert_eq!(workspace.shipped().unwrap(), root.join(SHIPPED));
    }

    /// A shipped example's manifest path climbs from the component crate to the root.
    #[test]
    fn a_shipped_example_is_reached_by_climbing_to_the_root() {
        let scratch = fixture();
        let root = scratch.path();
        let workspace = read(root);
        let component = workspace.component_crate().unwrap().directory;
        assert_eq!(workspace.climb(&component).unwrap(), "../../");
        assert_eq!(
            format!(
                "{}examples/extensions/hello.rs",
                workspace.climb(&component).unwrap()
            ),
            "../../examples/extensions/hello.rs"
        );
        assert_eq!(workspace.relative(&component).unwrap(), "crates/component");
    }
}

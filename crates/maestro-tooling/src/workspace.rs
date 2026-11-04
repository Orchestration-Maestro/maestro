//! The workspace a development command serves, located from the directory it starts in.
//!
//! A development command is installed once and started in the workspace it prepares, so
//! nothing here reads the directory the command was compiled in. The manifests are read
//! directly: a consuming workspace builds on Maestro through git dependencies, whose checkouts
//! sit outside it, so only the manifests the workspace itself holds name its own crates.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// The marker that opens the generated example region in a crate's manifest.
pub const REGION_START: &str = "# example-index:start";
/// The marker that closes that region.
pub const REGION_END: &str = "# example-index:end";
/// The shipped extension folder, beside the workspace manifest.
const SHIPPED: &str = "examples/extensions";
/// The standalone guide fixture, under the crate that holds one.
pub(crate) const GUIDE: &str = "tests/fixtures/hello-extension";

/// Directories a workspace scan never enters.
const SKIPPED: [&str; 3] = ["target", "node_modules", "build"];

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
    tables(text).is_some_and(|table| table.contains_key("workspace"))
}

/// The tables of a manifest, when it parses.
fn tables(text: &str) -> Option<toml::Table> {
    text.parse().ok()
}

/// A workspace and the manifests its own directories hold.
pub(crate) struct Workspace {
    /// The workspace root directory.
    root: PathBuf,
    /// The manifest of every crate the workspace holds, in path order.
    manifests: Vec<PathBuf>,
}

impl Workspace {
    /// Read the manifests of the workspace at `root`.
    ///
    /// # Errors
    /// Returns a failed directory listing.
    pub(crate) fn read(root: &Path) -> io::Result<Self> {
        let mut manifests = Vec::new();
        collect(root, &mut manifests)?;
        manifests.sort();
        Ok(Self {
            root: root.to_path_buf(),
            manifests,
        })
    }

    /// The root directory the manifests were read under.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The crate whose manifest holds the generated example region.
    ///
    /// # Errors
    /// Returns an error naming the crates when none, or more than one, holds the region.
    pub(crate) fn component_crate(&self) -> io::Result<PathBuf> {
        let mut marked = Vec::new();
        for manifest in &self.manifests {
            let text = text(manifest)?;
            if text.contains(REGION_START) && text.contains(REGION_END) {
                marked.push(directory(manifest));
            }
        }
        match marked.as_slice() {
            [component] => Ok(component.clone()),
            [] => Err(io::Error::other(format!(
                "no manifest under {} holds {REGION_START}",
                self.root.display()
            ))),
            _ => Err(io::Error::other(format!(
                "more than one manifest under {} holds {REGION_START}: {}",
                self.root.display(),
                self.names(&marked)?
            ))),
        }
    }

    /// The directory of the crate whose package is `name`, when this workspace holds it.
    pub(crate) fn package(&self, name: &str) -> Option<PathBuf> {
        self.manifests.iter().find_map(|manifest| {
            let text = text(manifest).ok()?;
            let table = tables(&text)?;
            let package = table.get("package")?.get("name")?.as_str()?;
            (package == name).then(|| directory(manifest))
        })
    }

    /// The crate that holds the standalone guide fixture.
    pub(crate) fn guide(&self) -> Option<PathBuf> {
        self.manifests.iter().find_map(|manifest| {
            let crate_root = directory(manifest);
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

/// The text of one manifest.
fn text(manifest: &Path) -> io::Result<String> {
    fs::read_to_string(manifest)
        .map_err(|error| io::Error::other(format!("{}: {error}", manifest.display())))
}

/// The directory a manifest sits in.
fn directory(manifest: &Path) -> PathBuf {
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// Gather the manifests under `directory`, skipping build output and hidden directories.
fn collect(directory: &Path, manifests: &mut Vec<PathBuf>) -> io::Result<()> {
    let manifest = directory.join("Cargo.toml");
    if manifest.is_file() {
        manifests.push(manifest);
    }
    let entries = fs::read_dir(directory)
        .map_err(|error| io::Error::other(format!("{}: {error}", directory.display())))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let hidden = name.to_string_lossy().starts_with('.');
        let nested = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && !hidden
            && !SKIPPED.iter().any(|skipped| name == *skipped)
        {
            collect(&nested, manifests)
                .map_err(|error| io::Error::other(format!("{}: {error}", nested.display())))?;
        }
    }
    Ok(())
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
        crate_at(root, "crates/component", &marked("component"));
        crate_at(
            root,
            "crates/plain",
            "[package]\nname = \"plain\"\nversion = \"0.1.0\"\n",
        );
        scratch
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

    /// Exactly one marked manifest names the component crate, so a rename cannot pick another.
    #[test]
    fn the_component_crate_is_the_one_marked_manifest() {
        let scratch = fixture();
        let workspace = Workspace::read(scratch.path()).unwrap();
        assert_eq!(
            workspace.component_crate().unwrap(),
            scratch.path().join("crates/component")
        );
        crate_at(scratch.path(), "crates/second", &marked("second"));
        let error = Workspace::read(scratch.path())
            .unwrap()
            .component_crate()
            .unwrap_err()
            .to_string();
        assert!(error.contains("crates/component, crates/second"), "{error}");
        fs::remove_file(scratch.path().join("crates/second/Cargo.toml")).unwrap();
        fs::remove_file(scratch.path().join("crates/component/Cargo.toml")).unwrap();
        let error = Workspace::read(scratch.path())
            .unwrap()
            .component_crate()
            .unwrap_err()
            .to_string();
        assert!(error.contains("no manifest"), "{error}");
    }

    /// A package outside the workspace is absent, which is what skips the steps tied to it.
    #[test]
    fn only_the_manifests_the_workspace_holds_name_a_package() {
        let scratch = fixture();
        let workspace = Workspace::read(scratch.path()).unwrap();
        assert_eq!(
            workspace.package("plain").unwrap(),
            scratch.path().join("crates/plain")
        );
        assert_eq!(workspace.package("maestro-models"), None);
    }

    /// The guide fixture and the shipped folder are the workspace's own, when it has them.
    #[test]
    fn the_guide_fixture_and_the_shipped_folder_are_optional() {
        let scratch = fixture();
        let root = scratch.path();
        let mut workspace = Workspace::read(root).unwrap();
        assert_eq!(workspace.guide(), None);
        assert_eq!(workspace.shipped(), None);
        std::fs::create_dir_all(root.join("crates/guide").join(GUIDE)).unwrap();
        crate_at(root, "crates/guide", "[package]\nname = \"guide\"\n");
        std::fs::create_dir_all(root.join(SHIPPED)).unwrap();
        workspace = Workspace::read(root).unwrap();
        assert_eq!(workspace.guide().unwrap(), root.join("crates/guide"));
        assert_eq!(workspace.shipped().unwrap(), root.join(SHIPPED));
    }

    /// A shipped example's manifest path climbs from the component crate to the root.
    #[test]
    fn a_shipped_example_is_reached_by_climbing_to_the_root() {
        let scratch = fixture();
        let root = scratch.path();
        let workspace = Workspace::read(root).unwrap();
        let component = workspace.component_crate().unwrap();
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

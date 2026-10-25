//! Installation evidence classification.

/// How an installation ships its assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageLayout {
    /// A source workspace run from its own checkout.
    Package,
    /// Assets beside a standalone executable.
    Standalone,
}
/// Owned installation location observations.
#[derive(Debug)]
pub struct Installation {
    /// The directory the package-directory walk starts from.
    pub module_dir: String,
    /// The path of the running executable.
    pub executable: String,
    /// How this installation ships its assets.
    pub layout: PackageLayout,
}
#[cfg(not(target_arch = "wasm32"))]
impl Installation {
    /// The running process and its executable, with the probed build layout.
    ///
    /// # Errors
    /// Returns the error of locating the running executable.
    pub fn current() -> std::io::Result<Self> {
        let (module_dir, layout) = probe_layout();
        Self::locate(module_dir, layout, std::env::current_exe)
    }

    /// Complete a probed walk start and layout with the path `executable` locates.
    pub(super) fn locate(
        module_dir: String,
        layout: PackageLayout,
        executable: impl FnOnce() -> std::io::Result<std::path::PathBuf>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            module_dir,
            executable: executable()?.to_string_lossy().into_owned(),
            layout,
        })
    }
}
/// The package-directory walk start and asset layout of this build, observed without
/// the running executable.
///
/// A build whose compile-time crate directory still sits below its source workspace
/// runs from that checkout, so the walk starts at the workspace root, which holds the
/// shipped documentation; any other build is a standalone executable whose assets sit
/// beside it.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn probe_layout() -> (String, PackageLayout) {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    match workspace_root(crate_dir) {
        Some(root) => (root, PackageLayout::Package),
        None => (crate_dir.to_owned(), PackageLayout::Standalone),
    }
}
/// The nearest directory from `start` upward whose `Cargo.toml` declares `[workspace]`.
#[cfg(not(target_arch = "wasm32"))]
fn workspace_root(start: &str) -> Option<String> {
    // shortcut: matches only a `[workspace]` header line; parse the manifest if a dotted
    // `workspace.` key ever declares the workspace instead.
    let declares_workspace = |dir: &std::path::Path| {
        std::fs::read_to_string(dir.join("Cargo.toml"))
            .is_ok_and(|manifest| manifest.lines().any(|line| line.trim() == "[workspace]"))
    };
    std::path::Path::new(start)
        .ancestors()
        .find(|dir| declares_workspace(dir))
        .map(|dir| dir.to_string_lossy().into_owned())
}
/// The package manager suggested by installation paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallMethod {
    /// A standalone binary installation.
    BunBinary,
    /// An npm installation.
    Npm,
    /// A pnpm installation.
    Pnpm,
    /// A Yarn installation.
    Yarn,
    /// A Bun global installation.
    Bun,
    /// No known manager marker.
    Unknown,
}
/// Select the installation method without executing a package manager.
#[must_use]
pub fn detect_install_method(installation: &Installation) -> InstallMethod {
    if installation.layout == PackageLayout::Standalone {
        return InstallMethod::BunBinary;
    }
    let path = format!("{}\0{}", installation.module_dir, installation.executable)
        .to_ascii_lowercase()
        .replace('\\', "/");
    if path.contains("/pnpm/") || path.contains("/.pnpm/") {
        InstallMethod::Pnpm
    } else if path.contains("/yarn/") || path.contains("/.yarn/") {
        InstallMethod::Yarn
    } else if path.contains("/install/global/node_modules/") {
        InstallMethod::Bun
    } else if path.contains("/npm/") || path.contains("/node_modules/") {
        InstallMethod::Npm
    } else {
        InstallMethod::Unknown
    }
}

//! Installation evidence classification.

/// How an installation ships its assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageLayout {
    /// A package tree with source or distribution directories.
    Package,
    /// Assets beside a standalone executable.
    Standalone,
}
/// Owned installation location observations.
#[derive(Debug)]
pub struct Installation {
    /// The directory containing the configuration module.
    pub module_dir: String,
    /// The executable path observed by the caller.
    pub executable: String,
    /// The supplied package layout.
    pub layout: PackageLayout,
}
#[cfg(not(target_arch = "wasm32"))]
impl Installation {
    /// The running process: its executable and this crate's compile-time directory.
    ///
    /// A build that finds a package manifest walking up from that directory runs from
    /// its source, so its assets come from that package tree; any other build is a
    /// standalone executable whose assets sit beside it.
    ///
    /// # Errors
    /// Returns the error of locating the running executable.
    pub fn current() -> std::io::Result<Self> {
        let module_dir = env!("CARGO_MANIFEST_DIR").to_owned();
        let exists = |path: &str| std::path::Path::new(path).exists();
        let layout = if super::paths::package_root(&module_dir, exists).is_some() {
            PackageLayout::Package
        } else {
            PackageLayout::Standalone
        };
        Ok(Self {
            module_dir,
            executable: std::env::current_exe()?.to_string_lossy().into_owned(),
            layout,
        })
    }
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

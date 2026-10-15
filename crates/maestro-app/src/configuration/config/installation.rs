//! Installation evidence classification.

/// The layout supplied by the application composition root.
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

//! Configuration path queries.

use super::installation::{Installation, PackageLayout};
use super::{APP_NAME, CONFIG_DIR_NAME};
use maestro_path::{dirname, join};
use std::io;

/// Expand only exact `~` and a leading `~/`, preserving the remaining text.
///
/// # Errors
/// Returns the supplied home lookup error when expansion needs it.
pub fn expand_tilde_path(
    path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    if path == "~" {
        return home();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home()? + "/" + rest);
    }
    Ok(path.to_owned())
}

/// Select an override, standalone directory, or nearest package marker, observing the
/// installation only when no override applies.
///
/// # Errors
/// Returns the home lookup error when a tilde override needs it, or the installation
/// lookup error otherwise.
pub fn get_package_dir(
    installation: impl FnOnce() -> io::Result<Installation>,
    override_dir: Option<&str>,
    home: impl FnOnce() -> io::Result<String>,
    exists: impl Fn(&str) -> bool,
) -> io::Result<String> {
    if let Some(path) = override_dir.filter(|s| !s.is_empty()) {
        return expand_tilde_path(path, home);
    }
    let installation = installation()?;
    if installation.layout == PackageLayout::Standalone {
        return Ok(dirname(&installation.executable));
    }
    let mut dir = std::borrow::Cow::Borrowed(installation.module_dir.as_str());
    loop {
        let parent = dirname(&dir);
        if parent == *dir {
            return Ok(installation.module_dir);
        }
        if exists(&join(&[&dir, "package.json"])) || exists(&join(&[&dir, "Cargo.toml"])) {
            return Ok(dir.into_owned());
        }
        dir = parent.into();
    }
}

/// Select a nonempty override or the default home configuration root.
///
/// # Errors
/// Returns the home lookup error when the chosen path requires home.
pub fn get_agent_dir(
    override_dir: Option<&str>,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    if let Some(path) = override_dir.filter(|s| !s.is_empty()) {
        return expand_tilde_path(path, home);
    }
    Ok(join(&[&home()?, CONFIG_DIR_NAME, "agent"]))
}
/// Select one shared source, distribution or standalone asset layout.
fn asset_dir(
    package_dir: &str,
    layout: PackageLayout,
    packaged: &[&str],
    standalone: &str,
    exists: impl Fn(&str) -> bool,
) -> String {
    if layout == PackageLayout::Standalone {
        return join(&[package_dir, standalone]);
    }
    let src_or_dist = if exists(&join(&[package_dir, "src"])) {
        "src"
    } else {
        "dist"
    };
    let mut parts = vec![package_dir, src_or_dist];
    parts.extend_from_slice(packaged);
    join(&parts)
}
/// Get the built-in theme directory: the theme crate's assets in a source workspace, or
/// the `theme` directory of a standalone package directory.
#[must_use]
pub fn get_themes_dir(package_dir: &str, layout: PackageLayout) -> String {
    match layout {
        PackageLayout::Package => {
            join(&[package_dir, "crates", "maestro-theme", "assets", "theme"])
        }
        PackageLayout::Standalone => join(&[package_dir, "theme"]),
    }
}
/// Get the export template directory.
pub fn get_export_template_dir(
    package_dir: &str,
    layout: PackageLayout,
    exists: impl Fn(&str) -> bool,
) -> String {
    asset_dir(
        package_dir,
        layout,
        &["core", "export-html"],
        "export-html",
        exists,
    )
}
/// Get the built-in interactive assets directory.
pub fn get_interactive_assets_dir(
    package_dir: &str,
    layout: PackageLayout,
    exists: impl Fn(&str) -> bool,
) -> String {
    asset_dir(
        package_dir,
        layout,
        &["modes", "interactive", "assets"],
        "assets",
        exists,
    )
}
/// Join an asset name onto the supplied assets directory.
#[must_use]
pub fn get_bundled_interactive_asset_path(assets_dir: &str, name: &str) -> String {
    join(&[assets_dir, name])
}
/// Resolve a packaged text path, reading the process directories only when it is relative.
///
/// # Errors
/// Returns the working-directory error unchanged when the path needs it.
fn package_path(package_dir: &str, name: &str) -> io::Result<String> {
    let path = join(&[package_dir, name]);
    maestro_resources::paths::resolve(&[&path])
}
/// Resolve the packaged README path.
///
/// # Errors
/// Returns the working-directory error unchanged when the path needs it.
pub fn get_readme_path(package_dir: &str) -> io::Result<String> {
    package_path(package_dir, "README.md")
}
/// Resolve the packaged documentation directory.
///
/// # Errors
/// Returns the working-directory error unchanged when the path needs it.
pub fn get_docs_path(package_dir: &str) -> io::Result<String> {
    package_path(package_dir, "docs")
}
/// Resolve the packaged examples directory.
///
/// # Errors
/// Returns the working-directory error unchanged when the path needs it.
pub fn get_examples_path(package_dir: &str) -> io::Result<String> {
    package_path(package_dir, "examples")
}
/// Resolve the packaged changelog path.
///
/// # Errors
/// Returns the working-directory error unchanged when the path needs it.
pub fn get_changelog_path(package_dir: &str) -> io::Result<String> {
    package_path(package_dir, "CHANGELOG.md")
}
/// Join the custom themes onto the supplied agent root.
#[must_use]
pub fn get_custom_themes_dir(agent_dir: &str) -> String {
    join(&[agent_dir, "themes"])
}
/// Join the models file onto the supplied agent root.
#[must_use]
pub fn get_models_path(agent_dir: &str) -> String {
    join(&[agent_dir, "models.json"])
}
/// Join the authentication file onto the supplied agent root.
#[must_use]
pub fn get_auth_path(agent_dir: &str) -> String {
    join(&[agent_dir, "auth.json"])
}
/// Join the settings file onto the supplied agent root.
#[must_use]
pub fn get_settings_path(agent_dir: &str) -> String {
    join(&[agent_dir, "settings.json"])
}
/// Join the tools directory onto the supplied agent root.
#[must_use]
pub fn get_tools_dir(agent_dir: &str) -> String {
    join(&[agent_dir, "tools"])
}
/// Join the managed binaries directory onto the supplied agent root.
#[must_use]
pub fn get_bin_dir(agent_dir: &str) -> String {
    join(&[agent_dir, "bin"])
}
/// Join the prompt templates directory onto the supplied agent root.
#[must_use]
pub fn get_prompts_dir(agent_dir: &str) -> String {
    join(&[agent_dir, "prompts"])
}
/// Join the sessions directory onto the supplied agent root.
#[must_use]
pub fn get_sessions_dir(agent_dir: &str) -> String {
    join(&[agent_dir, "sessions"])
}
/// Join the debug log file onto the supplied agent root.
#[must_use]
pub fn get_debug_log_path(agent_dir: &str) -> String {
    join(&[agent_dir, &format!("{APP_NAME}-debug.log")])
}

/// Resolve the shipped workspace skill beneath the package directory.
///
/// # Errors
/// Returns path-resolution failures.
pub fn get_workspace_skill_path(package_dir: &str) -> io::Result<String> {
    package_path(package_dir, "docs/skills/workspace/SKILL.md")
}

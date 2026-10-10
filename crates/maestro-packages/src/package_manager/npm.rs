//! npm install and uninstall in user and project scope.
use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations};
use maestro_path::join;
use std::io;

/// The manifest written into a new project npm root, without a final newline.
const PACKAGE_JSON: &str = "{\n  \"name\": \"maestro-extensions\",\n  \"private\": true\n}";
/// The ignore file written into a new managed root.
pub(super) const GITIGNORE: &str = "*\n!.gitignore\n";

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Installs a spec globally, or into the project root after creating its missing files.
    pub(super) async fn install_npm(
        &self,
        spec: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        if scope == InstalledSourceScope::User {
            return self.run_npm(&["install", "-g", spec], None).await;
        }
        let root = join(&[&self.base(scope), "npm"]);
        self.ensure_file(&root, ".gitignore", GITIGNORE)?;
        self.ensure_file(&root, "package.json", PACKAGE_JSON)?;
        self.run_npm(&["install", spec, "--prefix", &root], None)
            .await
    }
    /// Uninstalls a package by name; a project root that does not exist has nothing to remove.
    pub(super) async fn uninstall_npm(
        &self,
        name: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        if scope == InstalledSourceScope::User {
            return self.run_npm(&["uninstall", "-g", name], None).await;
        }
        let root = join(&[&self.base(scope), "npm"]);
        if !self.operations.exists(&root) {
            return Ok(());
        }
        self.run_npm(&["uninstall", name, "--prefix", &root], None)
            .await
    }
    /// Writes a file into a directory unless one is already there, creating the directory first.
    pub(super) fn ensure_file(&self, dir: &str, name: &str, text: &str) -> io::Result<()> {
        if !self.operations.exists(dir) {
            self.operations.create_dir_all(dir)?;
        }
        let path = join(&[dir, name]);
        if self.operations.exists(&path) {
            return Ok(());
        }
        self.operations.write_file(&path, text)
    }
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Compares selected installed and latest strings, suppressing probe failures.
    pub(super) async fn npm_update_available(&self, name: &str, path: &str) -> bool {
        if self.offline() {
            return false;
        }
        let manifest = join(&[path, "package.json"]);
        if !self.operations.exists(&manifest) {
            return false;
        }
        let installed = self
            .operations
            .read_file(&manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .and_then(|value| {
                value
                    .as_object()?
                    .get("version")?
                    .as_str()
                    .map(str::to_owned)
            })
            .filter(|version| !version.is_empty());
        let Some(installed) = installed else {
            return false;
        };
        self.latest_npm_version(name)
            .await
            .is_ok_and(|latest| latest != installed)
    }
    /// Captures the live command's latest version without coercing decoded values.
    pub(super) async fn latest_npm_version(&self, name: &str) -> io::Result<String> {
        let (command, mut args) = self.npm_command()?;
        args.extend(["view", name, "version", "--json"].map(str::to_owned));
        let stdout = self
            .operations
            .run_command_capture(
                &command,
                &args,
                super::CommandCaptureOptions {
                    cwd: Some(&self.options.cwd),
                    timeout: Some(std::time::Duration::from_millis(10000)),
                    env: &[],
                },
            )
            .await?;
        let raw = crate::trim(&stdout);
        if raw.is_empty() {
            return Err(io::Error::other("Empty response from npm view"));
        }
        serde_json::from_str(raw).map_err(io::Error::other)
    }
}

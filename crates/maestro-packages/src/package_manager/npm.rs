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

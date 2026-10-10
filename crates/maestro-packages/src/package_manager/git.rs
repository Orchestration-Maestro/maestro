//! Git clone, checkout and dependency install, and removal with bounded parent cleanup.
use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations, npm::GITIGNORE};
use crate::GitSource;
use maestro_path::{Cwd, SEP, dirname, is_absolute, join, relative};
use std::io;

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// The managed Git root and the repository's directory below it.
    fn git_locations(&self, git: &GitSource, scope: InstalledSourceScope) -> (String, String) {
        let root = join(&[&self.base(scope), "git"]);
        let target = join(&[&root, &git.host, &git.path]);
        (root, target)
    }
    /// Clones into the scope's Git root unless the target exists, then checks out and installs.
    pub(super) async fn install_git(
        &self,
        git: &GitSource,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let (root, target) = self.git_locations(git, scope);
        if self.operations.exists(&target) {
            return Ok(());
        }
        self.ensure_file(&root, ".gitignore", GITIGNORE)?;
        self.operations.create_dir_all(&dirname(&target))?;
        let clone = ["clone".to_owned(), git.repo.clone(), target.clone()];
        self.run_command("git", &clone, None).await?;
        if let Some(reference) = &git.r#ref {
            let checkout = ["checkout".to_owned(), reference.clone()];
            self.run_command("git", &checkout, Some(&target)).await?;
        }
        if self.operations.exists(&join(&[&target, "package.json"])) {
            let args: &[&str] = if self.npm_command_configured()? {
                &["install"]
            } else {
                &["install", "--omit=dev"]
            };
            self.run_npm(args, Some(&target)).await?;
        }
        Ok(())
    }
    /// Removes the repository directory, then the empty parents below the Git root.
    pub(super) fn remove_git(
        &self,
        git: &GitSource,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let (root, target) = self.git_locations(git, scope);
        if !self.operations.exists(&target) {
            return Ok(());
        }
        self.operations.remove_path(&target)?;
        self.prune_empty_parents(&target, &root)
    }
    /// Removes an empty directory; a failed removal is reported as not removed.
    fn removed_if_empty(&self, dir: &str) -> io::Result<bool> {
        Ok(self.operations.directory_is_empty(dir)? && self.operations.remove_path(dir).is_ok())
    }
    /// Removes empty ancestors of the target that lie strictly inside the root.
    ///
    /// Root and ancestors are resolved with the working directory captured at most once, only when required
    /// (per-drive directory lookups are separate), and compared by path components, so a
    /// relative base prunes and a similarly named sibling directory does not. A failed
    /// removal ends the cleanup without failing the operation.
    fn prune_empty_parents(&self, target: &str, root: &str) -> io::Result<()> {
        let mut paths = self
            .resolve_together(&[&[root], &[&dirname(target)]])?
            .into_iter();
        let (Some(root), Some(mut current)) = (paths.next(), paths.next()) else {
            return Ok(());
        };
        while is_inside(&root, &current) {
            if self.operations.exists(&current) && !self.removed_if_empty(&current)? {
                break;
            }
            current = dirname(&current);
        }
        Ok(())
    }
}

/// Whether the resolved `path` is a proper descendant of the resolved `root`.
fn is_inside(root: &str, path: &str) -> bool {
    let path = relative(
        root,
        path,
        &Cwd {
            current: "",
            drive_directories: &[],
        },
    );
    !path.is_empty() && !is_absolute(&path) && path.split(SEP).next() != Some("..")
}

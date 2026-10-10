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
        self.install_git_dependencies(&target).await
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

/// The local ref and targeted fetch operands selected before refreshing.
struct GitUpdateTarget {
    /// The ref used for post-fetch comparison and reset.
    reference: &'static str,
    /// The refspec updated by the fetch.
    refspec: String,
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Installs dependencies only when the checkout has a manifest.
    async fn install_git_dependencies(&self, target: &str) -> io::Result<()> {
        if self.operations.exists(&join(&[target, "package.json"])) {
            let args: &[&str] = if self.npm_command_configured()? {
                &["install"]
            } else {
                &["install", "--omit=dev"]
            };
            self.run_npm(args, Some(target)).await?;
        }
        Ok(())
    }
    /// Retains the fallible pre-fetch probe because its failure selects fallback.
    async fn get_local_git_update_target(&self, path: &str) -> io::Result<GitUpdateTarget> {
        let upstream = async {
            let text = self
                .git_capture(path, &["rev-parse", "--abbrev-ref", "@{upstream}"], &[])
                .await?;
            let branch = crate::trim(&text)
                .strip_prefix("origin/")
                .filter(|branch| !branch.is_empty())
                .ok_or_else(|| io::Error::other("Unsupported upstream remote"))?;
            self.git_capture(path, &["rev-parse", "@{upstream}"], &[])
                .await?;
            Ok::<_, io::Error>(GitUpdateTarget {
                reference: "@{upstream}",
                refspec: format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"),
            })
        }
        .await;
        if let Ok(target) = upstream {
            return Ok(target);
        }
        let args = ["remote", "set-head", "origin", "-a"].map(str::to_owned);
        let _ = self.run_command("git", &args, Some(path)).await;
        self.git_capture(path, &["rev-parse", "origin/HEAD"], &[])
            .await?;
        let text = self
            .git_capture(path, &["symbolic-ref", "refs/remotes/origin/HEAD"], &[])
            .await
            .unwrap_or_default();
        let trimmed = crate::trim(&text);
        let branch = trimmed
            .strip_prefix("refs/remotes/origin/")
            .unwrap_or(trimmed);
        Ok(GitUpdateTarget {
            reference: "origin/HEAD",
            refspec: if branch.is_empty() {
                "+HEAD:refs/remotes/origin/HEAD".into()
            } else {
                format!("+refs/heads/{branch}:refs/remotes/origin/{branch}")
            },
        })
    }
    /// Fetches the selected ref and replaces changed contents without merging.
    pub(super) async fn update_git(
        &self,
        git: &GitSource,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let (_, path) = self.git_locations(git, scope);
        if !self.operations.exists(&path) {
            return self.install_git(git, scope).await;
        }
        let target = self.get_local_git_update_target(&path).await?;
        let args = ["fetch", "--prune", "--no-tags", "origin", &target.refspec].map(str::to_owned);
        self.run_command("git", &args, Some(&path)).await?;
        let local = self.git_capture(&path, &["rev-parse", "HEAD"], &[]).await?;
        let refreshed = self
            .git_capture(&path, &["rev-parse", target.reference], &[])
            .await?;
        if crate::trim(&local) == crate::trim(&refreshed) {
            return Ok(());
        }
        self.run_command(
            "git",
            &["reset", "--hard", target.reference].map(str::to_owned),
            Some(&path),
        )
        .await?;
        self.run_command("git", &["clean", "-fdx"].map(str::to_owned), Some(&path))
            .await?;
        self.install_git_dependencies(&path).await
    }
}

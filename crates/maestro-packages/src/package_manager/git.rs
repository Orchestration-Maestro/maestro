//! Git clone, checkout and dependency install, and removal with bounded parent cleanup.
use super::acquisition::SourceAcquisition;
use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations, npm::GITIGNORE};
use crate::GitSource;
use maestro_path::{Cwd, SEP, dirname, is_absolute, join, relative};
use maestro_resources::{SourceScope, paths::CONFIG_DIR_NAME};
use std::{io, path::Path};

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// The optional managed Git root with the selected repository directory.
    fn git_locations(&self, git: &GitSource, scope: SourceScope) -> (Option<String>, String) {
        let root = match scope {
            SourceScope::User => Some(join(&[&self.options.agent_dir, "git"])),
            SourceScope::Project => Some(join(&[&self.options.cwd, CONFIG_DIR_NAME, "git"])),
            SourceScope::Temporary => None,
        };
        let target = self.git_path(git, scope);
        (root, target)
    }
    /// Clones a missing checkout, then selects its ref and installs dependencies; an existing
    /// checkout is brought up to date with its requested ref or upstream branch instead.
    pub(super) async fn install_git(
        &self,
        git: &GitSource,
        scope: SourceScope,
    ) -> io::Result<SourceAcquisition> {
        let (_, target) = self.git_locations(git, scope);
        if Path::new(&target).exists() {
            return self.repin_git(git, &target).await;
        }
        self.prepare_git_root(git, scope)?;
        self.install_missing_git(git, &target).await?;
        Ok(SourceAcquisition::Prepared)
    }
    /// Initializes the managed root before preparing an installed checkout.
    fn prepare_git_root(&self, git: &GitSource, scope: SourceScope) -> io::Result<String> {
        let (root, target) = self.git_locations(git, scope);
        if let Some(root) = root {
            super::npm::ensure_file(&root, ".gitignore", GITIGNORE)?;
        }
        Ok(target)
    }
    /// Prepares a fresh checkout before publishing it at the installed path.
    async fn install_missing_git(&self, git: &GitSource, target: &str) -> io::Result<()> {
        std::fs::create_dir_all(dirname(target))?;
        let mut stage = GitStaging::new(&dirname(target))?;
        let result = async {
            self.prepare_git_checkout(git, stage.path(), false).await?;
            self.install_git_dependencies(stage.path()).await?;
            stage.publish(target)
        }
        .await;
        stage.finish(result)
    }
    /// Clones and selects a checkout, disabling hooks for candidate inspection.
    pub(super) async fn prepare_git_checkout(
        &self,
        git: &GitSource,
        target: &str,
        inspection: bool,
    ) -> io::Result<()> {
        let options = if inspection {
            vec!["-c".to_owned(), "core.hooksPath=".to_owned()]
        } else {
            Vec::new()
        };
        let mut clone = options.clone();
        clone.extend(["clone".to_owned(), git.repo.clone(), target.to_owned()]);
        self.run_command("git", &clone, None).await?;
        if let Some(reference) = &git.r#ref {
            let reference = if inspection {
                self.local_git_pin(git, target)
                    .await?
                    .ok_or_else(|| io::Error::other("Candidate pin is unavailable"))?
            } else {
                reference.clone()
            };
            let mut checkout = options;
            checkout.extend(["checkout".to_owned(), reference]);
            self.run_command("git", &checkout, Some(target)).await?;
        }
        Ok(())
    }
    /// Prepares ordinary installation before replacing any existing managed checkout.
    pub(super) async fn reinstall_git(
        &self,
        git: &GitSource,
        scope: SourceScope,
    ) -> io::Result<()> {
        let target = self.prepare_git_root(git, scope)?;
        let target = target.as_str();
        std::fs::create_dir_all(dirname(target))?;
        let mut stage = GitStaging::new(&dirname(target))?;
        let fresh = join(&[stage.path(), "checkout"]);
        self.install_missing_git(git, &fresh).await?;
        self.check_pin_repository(&fresh).await?;
        let commit = self
            .local_git_pin(git, &fresh)
            .await?
            .ok_or_else(|| io::Error::other("Installed pin is unavailable"))?;
        self.restore_git_commit(&fresh, &commit).await?;
        let previous = join(&[stage.path(), "previous"]);
        let existed = Path::new(target).exists();
        if existed {
            std::fs::rename(target, &previous)?;
        }
        if let Err(error) = std::fs::rename(&fresh, target) {
            if existed && let Err(recovery) = std::fs::rename(&previous, target) {
                stage.owned = false;
                return Err(with_recovery(
                    error,
                    Err(io::Error::other(format!(
                        "{recovery}; checkout retained at {previous}"
                    ))),
                ));
            }
            return Err(error);
        }
        stage.discard()
    }
    /// Resets and cleans a known commit, recovering selection after command failures.
    pub(super) async fn restore_git_commit(&self, path: &str, expected: &str) -> io::Result<()> {
        let previous = self.get_git_head(path).await?;
        let result = async {
            self.run_command(
                "git",
                &["reset", "--hard", expected].map(str::to_owned),
                Some(path),
            )
            .await?;
            self.run_command("git", &["clean", "-ffdx"].map(str::to_owned), Some(path))
                .await?;
            if !self.tracked_pin_paths(path, expected).await?.is_empty()
                || !self
                    .git_capture(path, &["ls-files", "--others", "-z"], &[])
                    .await?
                    .is_empty()
            {
                return Err(io::Error::other("Pinned checkout still contains drift"));
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(with_recovery(
                error,
                self.restore_git_head(path, &previous).await,
            )),
        }
    }
    /// Best-effort refresh, with collection left outside the progress catch.
    pub(super) async fn refresh_temporary_git(&self, git: &GitSource, source: &str) {
        if self.offline() {
            return;
        }
        let _ = self
            .with_progress(
                super::ProgressAction::Pull,
                source,
                format!("Refreshing {source}..."),
                self.update_git(git, SourceScope::Temporary),
            )
            .await;
    }
    /// Removes the repository directory, then the empty parents below the Git root.
    pub(super) fn remove_git(
        &self,
        git: &GitSource,
        scope: InstalledSourceScope,
    ) -> io::Result<()> {
        let (root, target) = self.git_locations(git, scope.resource_scope());
        if !Path::new(&target).exists() {
            return Ok(());
        }
        remove_path(&target)?;
        root.map_or(Ok(()), |root| prune_empty_parents(&target, &root))
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
    /// The `git fetch` arguments that update it.
    fetch_args: Vec<String>,
}
impl GitUpdateTarget {
    /// Fetches only `refspec` without tags, so `reference` is the ref that fetch updates.
    fn fetching(reference: &'static str, refspec: String) -> Self {
        Self {
            reference,
            fetch_args: ["fetch", "--prune", "--no-tags", "origin"]
                .into_iter()
                .map(str::to_owned)
                .chain([refspec])
                .collect(),
        }
    }

    /// Fetches only the explicit `reference` from the remote; the fetched commit is `FETCH_HEAD`.
    fn requested(reference: &str) -> Self {
        Self {
            reference: "FETCH_HEAD^{commit}",
            fetch_args: ["fetch", "origin", reference].map(str::to_owned).into(),
        }
    }
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Installs dependencies only when the checkout has a manifest.
    async fn install_git_dependencies(&self, target: &str) -> io::Result<()> {
        if Path::new(&join(&[target, "package.json"])).exists() {
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
            Ok::<_, io::Error>(GitUpdateTarget::fetching(
                "@{upstream}",
                format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"),
            ))
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
        Ok(GitUpdateTarget::fetching(
            "origin/HEAD",
            if branch.is_empty() {
                "+HEAD:refs/remotes/origin/HEAD".into()
            } else {
                format!("+refs/heads/{branch}:refs/remotes/origin/{branch}")
            },
        ))
    }
    /// Fetches the selected ref and replaces changed contents without merging.
    pub(super) async fn update_git(&self, git: &GitSource, scope: SourceScope) -> io::Result<()> {
        let (_, path) = self.git_locations(git, scope);
        if !Path::new(&path).exists() {
            return self.install_git(git, scope).await.map(drop);
        }
        let target = self.get_local_git_update_target(&path).await?;
        self.refresh_git(&path, &target).await.map(drop)
    }
    /// Fetches the target and replaces the checkout when its head changed.
    async fn refresh_git(
        &self,
        path: &str,
        target: &GitUpdateTarget,
    ) -> io::Result<SourceAcquisition> {
        self.run_command("git", &target.fetch_args, Some(path))
            .await?;
        let local = self.git_capture(path, &["rev-parse", "HEAD"], &[]).await?;
        let refreshed = self
            .git_capture(path, &["rev-parse", target.reference], &[])
            .await?;
        if crate::trim(&local) == crate::trim(&refreshed) {
            return Ok(SourceAcquisition::CurrentGit);
        }
        self.change_git_revision(
            path,
            &["reset", "--hard", target.reference].map(str::to_owned),
        )
        .await
    }
}

/// Original commit and attached/detached selection, retained across preparation.
struct GitHead {
    /// The previously selected commit.
    oid: String,
    /// Original symbolic HEAD, absent for detached checkouts.
    symbolic: Option<String>,
}
impl GitHead {
    /// Restores the original selection, forcing a detached checkout.
    fn selection_args(&self) -> Vec<String> {
        if let Some(symbolic) = &self.symbolic {
            return vec!["symbolic-ref".into(), "HEAD".into(), symbolic.clone()];
        }
        ["checkout", "--detach", "--force", &self.oid]
            .map(str::to_owned)
            .into()
    }
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Captures selection before any checkout mutation.
    async fn get_git_head(&self, path: &str) -> io::Result<GitHead> {
        let oid = self.git_capture(path, &["rev-parse", "HEAD"], &[]).await?;
        let symbolic = self
            .git_capture(path, &["rev-parse", "--symbolic-full-name", "HEAD"], &[])
            .await?;
        let symbolic = crate::trim(&symbolic);
        Ok(GitHead {
            oid: crate::trim(&oid).to_owned(),
            symbolic: (symbolic != "HEAD").then(|| symbolic.to_owned()),
        })
    }

    /// Restores selection first, without rewinding a different requested branch.
    async fn restore_git_head(&self, path: &str, previous: &GitHead) -> io::Result<()> {
        self.run_command("git", &previous.selection_args(), Some(path))
            .await?;
        self.run_command(
            "git",
            &["reset", "--hard", &previous.oid].map(str::to_owned),
            Some(path),
        )
        .await
    }
    /// Brings an existing checkout up to date with the requested ref, or with its
    /// upstream branch when no ref is requested.
    async fn repin_git(&self, git: &GitSource, path: &str) -> io::Result<SourceAcquisition> {
        let target = match &git.r#ref {
            Some(reference) => GitUpdateTarget::requested(reference),
            None => self.get_local_git_update_target(path).await?,
        };
        self.refresh_git(path, &target).await
    }
    /// Changes revision and prepares dependencies, recovering HEAD after any mutation failure.
    async fn change_git_revision(
        &self,
        path: &str,
        args: &[String],
    ) -> io::Result<SourceAcquisition> {
        let previous = self.get_git_head(path).await?;
        let result = async {
            self.run_command("git", args, Some(path)).await?;
            self.run_command("git", &["clean", "-fdx"].map(str::to_owned), Some(path))
                .await?;
            self.install_git_dependencies(path).await?;
            Ok(SourceAcquisition::Prepared)
        }
        .await;
        match result {
            Ok(outcome) => Ok(outcome),
            Err(error) => Err(with_recovery(
                error,
                self.restore_git_head(path, &previous).await,
            )),
        }
    }
}

/// A staging directory removed unless successfully published.
pub(super) struct GitStaging {
    /// Exclusively allocated path.
    path: String,
    /// Whether cleanup still belongs to this guard.
    owned: bool,
}
impl GitStaging {
    /// Allocates before a clone can write into the directory.
    pub(super) fn new(parent: &str) -> io::Result<Self> {
        Ok(Self {
            path: create_temp_dir(parent)?,
            owned: true,
        })
    }
    /// Borrows the owned path while preparation is running.
    pub(super) fn path(&self) -> &str {
        &self.path
    }
    /// Publishes the prepared path, disarming deletion only on success.
    fn publish(&mut self, target: &str) -> io::Result<()> {
        std::fs::rename(&self.path, target)?;
        self.owned = false;
        Ok(())
    }
    /// Settles an operation with explicit cleanup, retaining both errors on failure.
    pub(super) fn finish<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Ok(value) => {
                self.discard()?;
                Ok(value)
            }
            Err(error) => Err(with_recovery(error, self.discard())),
        }
    }
    /// Reports cleanup errors without deleting any path not owned here.
    fn discard(&mut self) -> io::Result<()> {
        if self.owned {
            remove_path(&self.path)?;
            self.owned = false;
        }
        Ok(())
    }
}
impl Drop for GitStaging {
    fn drop(&mut self) {
        let _ = self.discard();
    }
}

/// The initiating failure with an unsuccessful cleanup or recovery attached.
#[derive(Debug)]
struct GitRecoveryError {
    /// The original error, retained as the cause.
    original: io::Error,
    /// The recovery operation's error.
    recovery: io::Error,
}
impl std::fmt::Display for GitRecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}; recovery failed: {}", self.original, self.recovery)
    }
}
impl std::error::Error for GitRecoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.original)
    }
}
/// Retains the initiating error unchanged unless recovery also failed.
fn with_recovery(original: io::Error, recovery: io::Result<()>) -> io::Error {
    match recovery {
        Ok(()) => original,
        Err(recovery) => io::Error::new(original.kind(), GitRecoveryError { original, recovery }),
    }
}

/// Exclusively allocates an owned directory below `parent`; existing names are skipped.
fn create_temp_dir(parent: &str) -> io::Result<String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    loop {
        let name = format!(
            ".maestro-stage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = Path::new(parent).join(name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path.to_string_lossy().into_owned()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

/// Removes an empty directory; a failed removal is reported as not removed.
fn removed_if_empty(dir: &str) -> io::Result<bool> {
    Ok(directory_is_empty(dir)? && remove_path(dir).is_ok())
}

/// Whether a directory has no entries; an enumeration failure after an earlier entry is
/// still an error.
fn directory_is_empty(path: &str) -> io::Result<bool> {
    let mut empty = true;
    for entry in std::fs::read_dir(path)? {
        entry?;
        empty = false;
    }
    Ok(empty)
}

/// Removes a file, link or directory tree; a missing path succeeds.
fn remove_path(path: &str) -> io::Result<()> {
    let path = Path::new(path);
    let removed = match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path).or_else(|error| {
            if cfg!(windows) {
                std::fs::remove_dir(path)
            } else {
                Err(error)
            }
        }),
        Err(error) => Err(error),
    };
    removed.or_else(|error| match error.kind() {
        io::ErrorKind::NotFound => Ok(()),
        _ => Err(error),
    })
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;

/// Removes empty ancestors of the target that lie strictly inside the root.
///
/// Root and ancestors are resolved with the working directory captured at most once, only when required
/// (per-drive directory lookups are separate), and compared by path components, so a
/// relative base prunes and a similarly named sibling directory does not. A failed
/// removal ends the cleanup without failing the operation.
fn prune_empty_parents(target: &str, root: &str) -> io::Result<()> {
    let mut paths =
        maestro_resources::paths::resolve_groups(&[&[root], &[&dirname(target)]])?.into_iter();
    let (Some(root), Some(mut current)) = (paths.next(), paths.next()) else {
        return Ok(());
    };
    while is_inside(&root, &current) {
        if Path::new(&current).exists() && !removed_if_empty(&current)? {
            break;
        }
        current = dirname(&current);
    }
    Ok(())
}

//! Observations of managed pinned Git contents.
pub use maestro_package::GitPinStatus;

use super::{DefaultPackageManager, InstalledSourceScope, PackageOperations, sources::Source};
use crate::GitSource;
use std::{io, path::Path};

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Selects only explicit remote Git pins at managed installed locations.
    fn pin_target(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> io::Result<(GitSource, String)> {
        let Source::Git(git) = super::sources::parse(source) else {
            return Err(io::Error::other("Integrity requires a pinned Git source"));
        };
        if !git.pinned {
            return Err(io::Error::other("Integrity requires a pinned Git source"));
        }
        let path = self.git_path(&git, scope.resource_scope());
        let root = maestro_path::join(&[&self.base(scope), "git"]);
        check_managed_path(Path::new(&root), Path::new(&path))?;
        Ok((git, path))
    }
    /// Refuses linked worktrees and redirected repository metadata before mutation.
    pub(super) async fn check_pin_repository(&self, path: &str) -> io::Result<()> {
        let target = std::fs::canonicalize(path)?;
        let metadata = target.join(".git");
        if !std::fs::symlink_metadata(&metadata)?.is_dir() {
            return Err(io::Error::other("Integrity requires managed Git metadata"));
        }
        for (option, expected) in [
            ("--show-toplevel", target),
            ("--absolute-git-dir", metadata.clone()),
            ("--git-common-dir", metadata),
        ] {
            let observed = self.git_capture(path, &["rev-parse", option], &[]).await?;
            let observed = Path::new(path).join(crate::trim(&observed));
            if std::fs::canonicalize(observed)? != expected {
                return Err(io::Error::other("Git checkout escapes managed contents"));
            }
        }
        Ok(())
    }
    /// Resolves a pin locally without fetching or treating it as a command option.
    pub(super) async fn local_git_pin(
        &self,
        git: &GitSource,
        path: &str,
    ) -> io::Result<Option<String>> {
        let reference = format!("{}^{{commit}}", git.r#ref.as_deref().unwrap_or_default());
        let output = self
            .git_capture(
                path,
                &["rev-parse", "--revs-only", "--end-of-options", &reference],
                &[],
            )
            .await?;
        let commit = crate::trim(&output);
        Ok((!commit.is_empty()).then(|| commit.to_owned()))
    }
    /// Observes the exact local commit and NUL-delimited tracked and untracked paths.
    pub(super) async fn inspect_pin(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<GitPinStatus> {
        let (git, path) = self.pin_target(source, scope.unwrap_or(InstalledSourceScope::User))?;
        if !Path::new(&path).exists() {
            return Ok(GitPinStatus::default());
        }
        self.check_pin_repository(&path).await?;
        let expected = self.local_git_pin(&git, &path).await?.unwrap_or_default();
        let head = self.git_capture(&path, &["rev-parse", "HEAD"], &[]).await?;
        let mut paths = self
            .git_capture(&path, &["ls-files", "--others", "-z"], &[])
            .await?;
        let expected = crate::trim(&expected);
        let comparison = if expected.is_empty() {
            crate::trim(&head)
        } else {
            expected
        };
        paths.push_str(&self.tracked_pin_paths(&path, comparison).await?);
        let mut changed_paths: Vec<_> = paths
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect();
        changed_paths.sort();
        changed_paths.dedup();
        Ok(GitPinStatus {
            expected_commit: (!expected.is_empty()).then(|| expected.to_owned()),
            head_commit: Some(crate::trim(&head).to_owned()),
            changed_paths,
        })
    }
    /// Compares the real index and an unsuppressed index against the selected commit.
    pub(super) async fn tracked_pin_paths(&self, path: &str, commit: &str) -> io::Result<String> {
        let stage = super::git::GitStaging::new(&maestro_path::dirname(path))?;
        let index = std::fs::canonicalize(stage.path())?.join("index");
        let index = index.to_string_lossy();
        let env = [("GIT_INDEX_FILE", index.as_ref())];
        self.git_capture(path, &["read-tree", "--no-sparse-checkout", commit], &env)
            .await?;
        let mut paths = self
            .git_capture(
                path,
                &[
                    "diff",
                    "--cached",
                    "--no-renames",
                    "--ignore-submodules=none",
                    "--name-only",
                    "-z",
                    commit,
                    "--",
                ],
                &[],
            )
            .await?;
        paths.push_str(
            &self
                .git_capture(
                    path,
                    &[
                        "diff",
                        "--no-ext-diff",
                        "--no-renames",
                        "--ignore-submodules=none",
                        "--name-only",
                        "-z",
                        commit,
                        "--",
                    ],
                    &env,
                )
                .await?,
        );
        Ok(paths)
    }
    /// Restores the exact commit even when HEAD has not moved.
    pub(super) async fn repair_pin(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> io::Result<()> {
        let scope = scope.unwrap_or(InstalledSourceScope::User);
        let (git, path) = self.pin_target(source, scope)?;
        let status = self.inspect_pin(source, Some(scope)).await?;
        if status.is_clean() {
            return Ok(());
        }
        if let Some(expected) = status.expected_commit {
            match self.restore_git_commit(&path, &expected).await {
                Ok(()) => return Ok(()),
                Err(error) if self.offline() => return Err(error),
                Err(_) => {}
            }
        }
        if self.offline() {
            return Err(io::Error::other(
                "Pinned checkout could not be restored offline",
            ));
        }
        self.reinstall_git(&git, scope.resource_scope()).await
    }
}

/// Rejects link redirection anywhere below the supplied configuration base.
fn check_managed_path(root: &Path, target: &Path) -> io::Result<()> {
    let relative = target.strip_prefix(root).map_err(io::Error::other)?;
    let mut current = root.to_path_buf();
    for part in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(part) = part {
            current.push(part);
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(io::Error::other("Managed Git path contains a link"));
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

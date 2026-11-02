//! Read-only release discovery and isolated candidate contents.

use super::{
    DefaultPackageManager, PackageOperations,
    git::GitStaging,
    sources::{self, Source},
};
use maestro_path::dirname;
use maestro_resources::SourceScope;
use std::{
    collections::BTreeMap,
    io,
    path::{Component, Path},
};

/// One Git tag with its peeled commit identity.
#[derive(Debug, PartialEq, Eq)]
pub struct GitRelease {
    /// Tag name without the refs prefix.
    pub name: String,
    /// Commit named by the tag, not an annotated tag object.
    pub commit: String,
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Lists remote tags without acquiring or updating any checkout.
    pub(super) async fn git_releases(&self, source: &str) -> io::Result<Vec<GitRelease>> {
        let git = inspection_source(source)?;
        self.require_inspection_online()?;
        let output = self
            .git_capture(
                &self.options.cwd,
                &["ls-remote", "--tags", &git.repo],
                &[("GIT_TERMINAL_PROMPT", "0")],
            )
            .await?;
        let mut tags = BTreeMap::new();
        for line in output.lines() {
            let (commit, reference) = line.split_once('\t').ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid Git tag record")
            })?;
            let Some(name) = reference.strip_prefix("refs/tags/") else {
                continue;
            };
            let peeled = name.strip_suffix("^{}");
            let name = peeled.unwrap_or(name);
            if peeled.is_some() || !tags.contains_key(name) {
                tags.insert(name.to_owned(), commit.to_owned());
            }
        }
        Ok(tags
            .into_iter()
            .map(|(name, commit)| GitRelease { name, commit })
            .collect())
    }

    /// Refuses inspection rather than silently returning stale or missing data.
    fn require_inspection_online(&self) -> io::Result<()> {
        if self.offline() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "Package inspection is unavailable offline",
            ));
        }
        Ok(())
    }
}

/// Uses the shared source grammar while rejecting non-Git inspection requests.
fn inspection_source(source: &str) -> io::Result<crate::GitSource> {
    match sources::parse(source) {
        Source::Git(git) => Ok(git),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Package inspection requires a Git source",
        )),
    }
}

impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Reads a fresh candidate and removes it before returning any bytes.
    pub(super) async fn inspect_candidate_files(
        &self,
        source: &str,
        paths: &[String],
    ) -> io::Result<Vec<Option<Vec<u8>>>> {
        let git = inspection_source(source)?;
        if !git.pinned {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Candidate inspection requires a Git pin",
            ));
        }
        for path in paths {
            if Path::new(path)
                .components()
                .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Candidate paths must stay relative to the release",
                ));
            }
        }
        self.require_inspection_online()?;
        let parent = dirname(&self.git_path(&git, SourceScope::Temporary));
        std::fs::create_dir_all(&parent)?;
        let mut stage = GitStaging::new(&parent)?;
        let result = async {
            self.prepare_git_checkout(&git, stage.path(), true).await?;
            let root = std::fs::canonicalize(stage.path())?;
            paths
                .iter()
                .map(|path| read_candidate_file(&root, path))
                .collect()
        }
        .await;
        stage.finish(result)
    }
}

/// Follows in-release links but never reads a target outside the staged candidate.
fn read_candidate_file(root: &Path, path: &str) -> io::Result<Option<Vec<u8>>> {
    let target = match std::fs::canonicalize(root.join(path)) {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !target.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Candidate file resolves outside the release",
        ));
    }
    std::fs::read(target).map(Some)
}

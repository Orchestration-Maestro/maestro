//! Read-only release discovery and isolated candidate contents.

/// One Git tag with its peeled commit identity.
#[derive(Debug, PartialEq, Eq)]
pub struct GitRelease {
    /// Tag name without the refs prefix.
    pub name: String,
    /// Commit named by the tag, not an annotated tag object.
    pub commit: String,
}

use super::{
    DefaultPackageManager, PackageOperations,
    sources::{self, Source},
};
use std::{collections::BTreeMap, io};

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

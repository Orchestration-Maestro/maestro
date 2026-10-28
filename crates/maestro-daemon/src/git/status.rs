//! Status snapshots and the fingerprints that decide when a cached one is rebuilt.

use super::discovery::{
    GitWorktreeInfo, automatic_workspace_label, canonicalize_best_effort_path,
    fallback_label_from_cwd, git_ref_storage_is_reftable, git_rev_parse_verify,
    git_space_metadata_from_info, git_symbolic_head_full, git_trimmed_stdout, git_worktree_info,
    read_text, trim,
};
use maestro_workspace::workspace::{
    GitSpaceMetadata, GitStatusRefreshDemand, WorkspaceGitStatusSnapshot,
    git::config::{ConfigEnvironment, ConfigRead, read_branch_config},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// How long a directory outside any repository is kept before it is searched again.
const NON_REPOSITORY_RETRY: Duration = Duration::from_secs(30);

/// One cached observation: a repository's fingerprint and snapshot, or a directory outside any.
#[derive(Debug, Clone)]
pub struct GitStatusCacheEntry {
    /// What the repository looked like, absent for a directory outside any repository.
    pub fingerprint: Option<GitStatusFingerprint>,
    /// Until when a result without a repository is kept instead of searched again.
    pub retry_after: Option<Instant>,
    /// The snapshot built from the observation.
    pub snapshot: WorkspaceGitStatusSnapshot,
}

impl GitStatusCacheEntry {
    /// Whether this is a still-fresh result for a directory outside any repository.
    fn is_unexpired_miss(&self) -> bool {
        self.fingerprint.is_none() && self.retry_after.is_some_and(|due| due > Instant::now())
    }

    /// Whether the repository still shows what this entry's snapshot was built from.
    fn matches(&self, fingerprint: &GitStatusFingerprint) -> bool {
        self.fingerprint.as_ref().is_some_and(|cached| {
            cached.head == fingerprint.head && cached.upstream == fingerprint.upstream
        })
    }
}

/// The reference and commit identities whose change makes a cached snapshot stale.
#[derive(Debug, Clone)]
pub struct GitStatusFingerprint {
    /// What the checkout has out.
    pub head: GitHeadIdentity,
    /// The branch's upstream, when tracking is configured and was demanded.
    pub upstream: Option<GitUpstreamIdentity>,
    /// The tracking configuration this was read from, reused while its files are unchanged.
    tracking: Option<TrackingRead>,
}

impl GitStatusFingerprint {
    /// Name of the checked-out branch.
    fn branch_name(&self) -> Option<&str> {
        match &self.head {
            GitHeadIdentity::Branch { short_name, .. } => Some(short_name),
            GitHeadIdentity::Detached { .. } => None,
        }
    }

    /// Commit the checkout is at.
    fn head_oid(&self) -> Option<&str> {
        match &self.head {
            GitHeadIdentity::Branch { oid, .. } => oid.as_deref(),
            GitHeadIdentity::Detached { oid } => Some(oid),
        }
    }
}

/// What `HEAD` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHeadIdentity {
    /// A local branch.
    Branch {
        /// Full reference name.
        full_ref: String,
        /// Name without the `refs/heads/` prefix.
        short_name: String,
        /// Commit the branch points at, absent for an unborn branch.
        oid: Option<String>,
    },
    /// A commit checked out directly.
    Detached {
        /// The commit.
        oid: String,
    },
}

/// The tracking reference of a branch and the commit it points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUpstreamIdentity {
    /// Remote named by the branch configuration.
    pub remote: String,
    /// Reference the branch merges from.
    pub merge_ref: String,
    /// Local reference the fetch refspecs map it to.
    pub full_ref: String,
    /// Commit that reference points at, absent when it does not exist.
    pub oid: Option<String>,
}

/// A configuration read for one branch.
#[derive(Debug, Clone)]
struct TrackingRead {
    /// The branch whose section was read.
    branch: String,
    /// What the read found and the files it depended on.
    read: ConfigRead,
}

/// Cache identity of the checkout containing `cwd`.
#[must_use]
pub fn git_status_cache_key(cwd: &Path) -> Option<PathBuf> {
    git_worktree_info(cwd).map(|info| canonicalize_best_effort_path(&info.repo_root))
}

/// Cache identity of a checkout whose metadata is already known.
#[must_use]
pub fn git_status_cache_key_for_space(space: &GitSpaceMetadata) -> PathBuf {
    canonicalize_best_effort_path(&space.repo_root)
}

/// Observe `cwd` for the demanded fields, reusing `cached` while its fingerprint still holds.
///
/// The second value is the entry to cache; it is absent when the repository could not be read.
#[must_use]
pub fn git_status_snapshot_for_cwd_with_demand(
    cwd: &Path,
    cached: Option<&GitStatusCacheEntry>,
    demand: GitStatusRefreshDemand,
    environment: &ConfigEnvironment,
) -> (WorkspaceGitStatusSnapshot, Option<GitStatusCacheEntry>) {
    if let Some(entry) = cached.filter(|entry| entry.is_unexpired_miss()) {
        return (entry.snapshot.clone(), Some(entry.clone()));
    }
    let home = environment.home.as_deref();
    let Some(info) = git_worktree_info(cwd) else {
        return outside_repository(cwd, home);
    };
    let repository = Repository {
        reftable: git_ref_storage_is_reftable(&info.git_common_dir),
        info: &info,
    };
    let fingerprint = repository.fingerprint(demand, cached, environment);
    let ahead_behind = fingerprint
        .as_ref()
        .filter(|_| demand.ahead_behind)
        .and_then(
            |fingerprint| match cached.filter(|entry| entry.matches(fingerprint)) {
                Some(entry) => entry.snapshot.ahead_behind,
                None => repository.ahead_behind(fingerprint),
            },
        );
    let snapshot = WorkspaceGitStatusSnapshot {
        auto_label: automatic_workspace_label(cwd, &info.repo_root, home),
        branch: fingerprint
            .as_ref()
            .and_then(GitStatusFingerprint::branch_name)
            .filter(|_| !demand.is_empty())
            .map(str::to_owned),
        ahead_behind,
        space: Some(git_space_metadata_from_info(&info)),
    };
    (
        snapshot.clone(),
        fingerprint.map(|fingerprint| GitStatusCacheEntry {
            fingerprint: Some(fingerprint),
            retry_after: None,
            snapshot,
        }),
    )
}

/// The result for a directory outside any repository, kept until its retry time.
fn outside_repository(
    cwd: &Path,
    home: Option<&Path>,
) -> (WorkspaceGitStatusSnapshot, Option<GitStatusCacheEntry>) {
    let snapshot = WorkspaceGitStatusSnapshot {
        auto_label: fallback_label_from_cwd(cwd, home),
        branch: None,
        ahead_behind: None,
        space: None,
    };
    (
        snapshot.clone(),
        Some(GitStatusCacheEntry {
            fingerprint: None,
            retry_after: Some(Instant::now() + NON_REPOSITORY_RETRY),
            snapshot,
        }),
    )
}

/// A discovered checkout and how its references are stored.
struct Repository<'a> {
    /// Paths of the checkout.
    info: &'a GitWorktreeInfo,
    /// Whether references live in reftable storage, which only Git can read.
    reftable: bool,
}

impl Repository<'_> {
    /// Read the head and, when ahead/behind is demanded, the upstream of the checked-out branch.
    fn fingerprint(
        &self,
        demand: GitStatusRefreshDemand,
        cached: Option<&GitStatusCacheEntry>,
        environment: &ConfigEnvironment,
    ) -> Option<GitStatusFingerprint> {
        let head = self.read_head()?;
        let branch = match &head {
            GitHeadIdentity::Branch { short_name, .. } if demand.ahead_behind => short_name.clone(),
            _ => {
                return Some(GitStatusFingerprint {
                    head,
                    upstream: None,
                    tracking: None,
                });
            }
        };
        let previous = cached
            .and_then(|entry| entry.fingerprint.as_ref())
            .and_then(|fingerprint| fingerprint.tracking.as_ref())
            .filter(|tracking| tracking.branch == branch && tracking.read.is_current());
        let tracking = previous.cloned().unwrap_or_else(|| TrackingRead {
            read: read_branch_config(self.info, &branch, environment),
            branch,
        });
        let upstream = self.upstream(&tracking);
        Some(GitStatusFingerprint {
            head,
            upstream,
            tracking: Some(tracking),
        })
    }

    /// Resolve the tracking reference a configuration read found for the branch.
    fn upstream(&self, tracking: &TrackingRead) -> Option<GitUpstreamIdentity> {
        let branch = tracking.read.branch.as_ref()?;
        let full_ref = branch.upstream_full_ref()?;
        Some(GitUpstreamIdentity {
            oid: self.ref_oid(&full_ref),
            remote: branch.remote.clone(),
            merge_ref: branch.merge_ref.clone(),
            full_ref,
        })
    }

    /// Read what `HEAD` names.
    fn read_head(&self) -> Option<GitHeadIdentity> {
        if self.reftable {
            return self.read_head_through_git();
        }
        let head = read_text(&self.info.git_dir.join("HEAD")).ok()?;
        let head = trim(&head);
        if let Some(full_ref) = head.strip_prefix("ref: ") {
            return self.branch_identity(full_ref);
        }
        (!head.is_empty()).then(|| GitHeadIdentity::Detached {
            oid: head.to_owned(),
        })
    }

    /// Read `HEAD` through Git, which is the only reader of reftable storage.
    fn read_head_through_git(&self) -> Option<GitHeadIdentity> {
        let root = &self.info.repo_root;
        match git_symbolic_head_full(root) {
            Some(full_ref) => self.branch_identity(&full_ref),
            None => git_rev_parse_verify(root, "HEAD").map(|oid| GitHeadIdentity::Detached { oid }),
        }
    }

    /// Identify a local branch by its full reference; other references are not branches.
    fn branch_identity(&self, full_ref: &str) -> Option<GitHeadIdentity> {
        Some(GitHeadIdentity::Branch {
            short_name: full_ref.strip_prefix("refs/heads/")?.to_owned(),
            oid: self.ref_oid(full_ref),
            full_ref: full_ref.to_owned(),
        })
    }

    /// The commit a reference points at: a loose file first, then the packed references.
    fn ref_oid(&self, full_ref: &str) -> Option<String> {
        if self.reftable {
            return git_rev_parse_verify(&self.info.repo_root, full_ref);
        }
        let common = &self.info.git_common_dir;
        let loose = read_text(&common.join(full_ref)).ok();
        if let Some(oid) = loose.as_deref().map(trim).filter(|oid| !oid.is_empty()) {
            return Some(oid.to_owned());
        }
        read_text(&common.join("packed-refs"))
            .ok()?
            .lines()
            .map(trim)
            .filter(|line| !line.starts_with(['#', '^']))
            .find_map(|line| {
                let (oid, name) = line.split_once(char::is_whitespace)?;
                (trim(name) == full_ref).then(|| oid.to_owned())
            })
    }

    /// Count the commits each side has that the other lacks.
    fn ahead_behind(&self, fingerprint: &GitStatusFingerprint) -> Option<(usize, usize)> {
        let head = fingerprint.head_oid()?;
        let upstream = fingerprint.upstream.as_ref()?.oid.as_deref()?;
        let range = format!("{head}...{upstream}");
        let counts = git_trimmed_stdout(
            &self.info.repo_root,
            &["rev-list", "--left-right", "--count", &range],
        )?;
        let mut counts = counts.split_whitespace();
        Some((counts.next()?.parse().ok()?, counts.next()?.parse().ok()?))
    }
}

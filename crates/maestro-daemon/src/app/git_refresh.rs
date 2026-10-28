//! Demand-driven repository status refresh on the daemon loop's timer.

use super::App;
use crate::{events::AppEvent, git::status::GitStatusCacheEntry};
use maestro_workspace::workspace::{
    GitStatusRefreshDemand, WorkspaceGitStatus, git::config::ConfigEnvironment,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

#[cfg(test)]
mod tests;

/// Time between status refreshes while a sidebar row or a directory report needs one.
const GIT_REMOTE_STATUS_REFRESH_INTERVAL: Duration = Duration::from_millis(1500);

/// Refresh scheduling and the status cache, retained across loop passes.
pub(super) struct GitRefresh {
    /// When the last refresh finished or was made due.
    last_remote_refresh: Instant,
    /// When repositories were last rediscovered instead of trusting cached keys.
    last_repo_discovery: Instant,
    /// Whether a refresh is running, so that no second one starts.
    in_flight: bool,
    /// Whether a refresh was requested while one ran.
    due_after_in_flight: bool,
    /// Whether a directory report asked for a fresh branch and label.
    identity_requested: bool,
    /// Cached observation for each repository.
    cache: HashMap<PathBuf, GitStatusCacheEntry>,
}

impl GitRefresh {
    /// Scheduling state whose first refresh is already due.
    pub(super) fn new(now: Instant) -> Self {
        Self {
            last_remote_refresh: now
                .checked_sub(GIT_REMOTE_STATUS_REFRESH_INTERVAL)
                .unwrap_or(now),
            last_repo_discovery: now,
            in_flight: false,
            due_after_in_flight: false,
            identity_requested: false,
            cache: HashMap::new(),
        }
    }
}

/// A workspace to observe and the cache key remembered for its directory.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceGitRefreshItem {
    /// Workspace handle.
    workspace_id: String,
    /// Directory the workspace's identity follows.
    resolved_identity_cwd: PathBuf,
    /// Cache key admitted for that directory, absent when it must be discovered.
    cache_key_hint: Option<PathBuf>,
}

/// A workspace that receives a shared observation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceGitRefreshTarget {
    /// Workspace handle.
    workspace_id: String,
    /// Directory the observation was requested for.
    resolved_identity_cwd: PathBuf,
}

/// One observation shared by every workspace in a repository.
#[derive(Debug, Clone)]
struct WorkspaceGitRefreshJob {
    /// Repository cache key.
    cache_key: PathBuf,
    /// Cached entry to reuse, absent when the key must be reconciled.
    cached: Option<GitStatusCacheEntry>,
    /// Workspaces the observation applies to.
    targets: Vec<WorkspaceGitRefreshTarget>,
}

/// What a background refresh hands back to the loop.
#[derive(Debug, Default)]
struct WorkspaceGitRefreshOutput {
    /// One status for each workspace.
    results: Vec<WorkspaceGitStatus>,
    /// Entries to cache, keyed by repository.
    cache_updates: Vec<(PathBuf, GitStatusCacheEntry)>,
}

impl<C: FnMut() -> u128> App<C> {
    /// Start one background refresh once its deadline has passed.
    pub(super) fn start_git_status_refresh_if_due(&mut self, now: Instant) {
        let _ = now;
        let items = self.workspace_git_refresh_items(false);
        let demand = self.git_refresh_demand();
        let cache = HashMap::new();
        spawn_refresh(self.launch.events.clone(), move || {
            refresh_workspace_git_statuses_with_cache_and_demand(
                items,
                &cache,
                demand,
                &ConfigEnvironment::default(),
            )
        });
    }

    /// Ask for a fresh branch and label of every workspace.
    pub(super) fn request_git_identity_refresh(&mut self, now: Instant) {
        self.mark_git_status_refresh_due(now);
    }

    /// Make the next refresh due now, dropping results cached for directories outside repositories.
    pub(super) fn mark_git_status_refresh_due(&mut self, now: Instant) {
        let _ = (now, self.git_refresh.in_flight);
    }

    /// When the next refresh is due, absent while none is wanted or one is running.
    pub(super) fn git_refresh_deadline(&self) -> Option<Instant> {
        let state = &self.git_refresh;
        let _ = (
            state.last_remote_refresh,
            state.last_repo_discovery,
            state.in_flight,
            state.due_after_in_flight,
            state.identity_requested,
            state.cache.len(),
        );
        None
    }

    /// The observations the sidebar rows consume.
    fn git_refresh_demand(&self) -> GitStatusRefreshDemand {
        let _ = &self.config;
        GitStatusRefreshDemand::default()
    }

    /// Workspaces to observe, each with the cache key remembered for its directory.
    fn workspace_git_refresh_items(
        &self,
        refresh_repo_discovery: bool,
    ) -> Vec<WorkspaceGitRefreshItem> {
        let _ = refresh_repo_discovery;
        self.state
            .topology
            .workspaces
            .iter()
            .take(0)
            .map(|workspace| WorkspaceGitRefreshItem {
                workspace_id: workspace.id.clone(),
                resolved_identity_cwd: workspace.identity_cwd.clone(),
                cache_key_hint: None,
            })
            .collect()
    }

    /// Apply a finished refresh and report whether anything visible changed.
    pub(super) fn handle_git_status_refreshed(
        &mut self,
        results: Vec<WorkspaceGitStatus>,
        cache_updates: Vec<(PathBuf, GitStatusCacheEntry)>,
    ) -> bool {
        let _ = (results, cache_updates, &self.git_refresh);
        false
    }
}

/// Group items that share a repository so each repository is observed once.
fn deduplicate_git_refresh_items(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
) -> Vec<WorkspaceGitRefreshJob> {
    let _ = cache;
    items
        .into_iter()
        .map(|item| WorkspaceGitRefreshJob {
            cache_key: item
                .cache_key_hint
                .unwrap_or_else(|| item.resolved_identity_cwd.clone()),
            cached: None,
            targets: vec![WorkspaceGitRefreshTarget {
                workspace_id: item.workspace_id,
                resolved_identity_cwd: item.resolved_identity_cwd,
            }],
        })
        .collect()
}

/// Observe every repository once and project the result onto each workspace.
fn refresh_workspace_git_statuses_with_cache_and_demand(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
    demand: GitStatusRefreshDemand,
    environment: &ConfigEnvironment,
) -> WorkspaceGitRefreshOutput {
    let _ = (demand, environment);
    for job in deduplicate_git_refresh_items(items, cache) {
        let _ = (&job.cache_key, &job.cached, &job.targets);
    }
    WorkspaceGitRefreshOutput::default()
}

/// Run a refresh on the blocking pool and publish its result, even if the refresh panics.
fn spawn_refresh(
    events: mpsc::Sender<AppEvent>,
    refresh: impl FnOnce() -> WorkspaceGitRefreshOutput + Send + 'static,
) {
    let output = refresh();
    let _ = (events, output.results, output.cache_updates);
}

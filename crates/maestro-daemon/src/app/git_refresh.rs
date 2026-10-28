//! Demand-driven repository status refresh on the daemon loop's timer.

use super::App;
use crate::{
    events::AppEvent,
    git::status::{
        GitStatusCacheEntry, git_status_cache_key, git_status_snapshot_for_cwd_with_demand,
    },
    settings::sidebar::SpaceSidebarToken,
};
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

/// Time between repository rediscoveries, which also reconcile every remembered cache key.
const GIT_REPO_DISCOVERY_REFRESH_INTERVAL: Duration = Duration::from_mins(5);

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
        let Some(deadline) = self.git_refresh_deadline() else {
            return;
        };
        if now < deadline {
            return;
        }
        let state = &self.git_refresh;
        let refresh_repo_discovery = state.identity_requested
            || now.saturating_duration_since(state.last_repo_discovery)
                >= GIT_REPO_DISCOVERY_REFRESH_INTERVAL;
        let items = self.workspace_git_refresh_items(refresh_repo_discovery);
        let mut demand = self.git_refresh_demand();
        let state = &mut self.git_refresh;
        demand.branch |= std::mem::take(&mut state.identity_requested);
        if items.is_empty() {
            state.last_remote_refresh = now;
            return;
        }
        if refresh_repo_discovery {
            state.last_repo_discovery = now;
        }
        state.in_flight = true;
        let cache = state.cache.clone();
        let environment = git_config_environment();
        spawn_refresh(self.launch.events.clone(), move || {
            refresh_workspace_git_statuses_with_cache_and_demand(
                items,
                &cache,
                demand,
                &environment,
            )
        });
    }

    /// Ask for a fresh branch and label of every workspace.
    pub(super) fn request_git_identity_refresh(&mut self, now: Instant) {
        self.git_refresh.identity_requested = true;
        self.mark_git_status_refresh_due(now);
    }

    /// Make the next refresh due now, dropping results cached for directories outside repositories.
    pub(super) fn mark_git_status_refresh_due(&mut self, now: Instant) {
        let state = &mut self.git_refresh;
        state.cache.retain(|_, entry| entry.fingerprint.is_some());
        if state.in_flight {
            state.due_after_in_flight = true;
            return;
        }
        state.last_remote_refresh = now
            .checked_sub(GIT_REMOTE_STATUS_REFRESH_INTERVAL)
            .unwrap_or(now);
        state.due_after_in_flight = false;
    }

    /// When the next refresh is due, absent while none is wanted or one is running.
    pub(super) fn git_refresh_deadline(&self) -> Option<Instant> {
        let state = &self.git_refresh;
        (!state.in_flight
            && !self.state.topology.workspaces.is_empty()
            && (state.identity_requested || !self.git_refresh_demand().is_empty()))
        .then_some(state.last_remote_refresh + GIT_REMOTE_STATUS_REFRESH_INTERVAL)
    }

    /// The observations the sidebar rows consume.
    fn git_refresh_demand(&self) -> GitStatusRefreshDemand {
        let mut demand = GitStatusRefreshDemand::default();
        for token in self.config.ui.sidebar.spaces.rows.iter().flatten() {
            match token.parts().0 {
                SpaceSidebarToken::Branch => demand.branch = true,
                SpaceSidebarToken::GitStatus => demand.ahead_behind = true,
                _ => {}
            }
        }
        demand
    }

    /// Workspaces to observe, each with the cache key remembered for its directory.
    fn workspace_git_refresh_items(
        &self,
        refresh_repo_discovery: bool,
    ) -> Vec<WorkspaceGitRefreshItem> {
        self.state
            .topology
            .workspaces
            .iter()
            .filter_map(|workspace| {
                let cwd = crate::workspace::resolved_identity_cwd_from(
                    workspace,
                    &self.state.terminals,
                    &self.terminal_runtimes,
                )?;
                let cache_key_hint = (!refresh_repo_discovery
                    && workspace.cached_identity_cwd == cwd)
                    .then(|| workspace.cached_git_status_key.clone());
                Some(WorkspaceGitRefreshItem {
                    workspace_id: workspace.id.clone(),
                    resolved_identity_cwd: cwd,
                    cache_key_hint,
                })
            })
            .collect()
    }

    /// Apply a finished refresh and report whether anything visible changed.
    pub(super) fn handle_git_status_refreshed(
        &mut self,
        results: Vec<WorkspaceGitStatus>,
        cache_updates: Vec<(PathBuf, GitStatusCacheEntry)>,
    ) -> bool {
        let now = Instant::now();
        let state = &mut self.git_refresh;
        state.in_flight = false;
        state.cache.extend(cache_updates);
        if state.due_after_in_flight {
            self.mark_git_status_refresh_due(now);
        } else {
            self.git_refresh.last_remote_refresh = now;
        }
        self.state
            .apply_workspace_git_statuses(&self.terminal_runtimes, results)
    }
}

/// Git's user-level configuration files for the process environment.
fn git_config_environment() -> ConfigEnvironment {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    ConfigEnvironment::from_variables(xdg.as_deref(), home.as_deref())
}

/// Group items that share a repository so each repository is observed once.
fn deduplicate_git_refresh_items(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
) -> Vec<WorkspaceGitRefreshJob> {
    let mut indexes = HashMap::<PathBuf, usize>::new();
    let mut jobs = Vec::<WorkspaceGitRefreshJob>::new();
    for item in items {
        let reconcile = item.cache_key_hint.is_none();
        let cache_key = item.cache_key_hint.unwrap_or_else(|| {
            git_status_cache_key(&item.resolved_identity_cwd)
                .unwrap_or_else(|| item.resolved_identity_cwd.clone())
        });
        let target = WorkspaceGitRefreshTarget {
            workspace_id: item.workspace_id,
            resolved_identity_cwd: item.resolved_identity_cwd,
        };
        if let Some(&index) = indexes.get(&cache_key) {
            jobs[index].cached = jobs[index].cached.take().filter(|_| !reconcile);
            jobs[index].targets.push(target);
            continue;
        }
        let cached = cache.get(&cache_key).filter(|_| !reconcile).cloned();
        indexes.insert(cache_key.clone(), jobs.len());
        jobs.push(WorkspaceGitRefreshJob {
            cache_key,
            cached,
            targets: vec![target],
        });
    }
    jobs
}

/// Observe every repository once and project the result onto each workspace.
fn refresh_workspace_git_statuses_with_cache_and_demand(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
    demand: GitStatusRefreshDemand,
    environment: &ConfigEnvironment,
) -> WorkspaceGitRefreshOutput {
    let mut output = WorkspaceGitRefreshOutput::default();
    for job in deduplicate_git_refresh_items(items, cache) {
        let (snapshot, entry) = git_status_snapshot_for_cwd_with_demand(
            &job.cache_key,
            job.cached.as_ref(),
            demand,
            environment,
        );
        output
            .cache_updates
            .extend(entry.map(|entry| (job.cache_key.clone(), entry)));
        output.results.extend(job.targets.into_iter().map(|target| {
            snapshot.clone().into_workspace_status(
                target.workspace_id,
                (target.resolved_identity_cwd, job.cache_key.clone()),
                demand,
                environment.home.as_deref(),
            )
        }));
    }
    output
}

/// Run a refresh on the blocking pool and publish its result, even if the refresh panics.
fn spawn_refresh(
    events: mpsc::Sender<AppEvent>,
    refresh: impl FnOnce() -> WorkspaceGitRefreshOutput + Send + 'static,
) {
    tokio::spawn(async move {
        let output = tokio::task::spawn_blocking(refresh)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "repository status refresh failed");
                WorkspaceGitRefreshOutput::default()
            });
        AppEvent::GitStatusRefreshed {
            results: output.results,
            cache_updates: output.cache_updates,
        }
        .publish(&events)
        .await;
    });
}

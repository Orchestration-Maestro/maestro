//! Refresh demand, deduplication, in-flight suppression and the loop's timer.

use super::*;
use crate::{
    api::event_hub::EventHub,
    app::construction::PaneLaunchInputs,
    events::AppEvent,
    git::status::git_status_snapshot_for_cwd_with_demand,
    render_signal::RenderSignal,
    settings::{model::Config, sidebar::SpaceSidebarToken},
    terminal_theme::TerminalTheme,
};
use maestro_process::pty::{PtySetupEntry, foreground::ProcessDetectionMode};
use maestro_workspace::workspace::{GitSpaceMetadata, WorkspaceGitStatusSnapshot};
use std::{
    path::Path,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::{Notify, mpsc};

/// Daemon owner driven by a deterministic public-identity clock.
type TestApp = App<fn() -> u128>;

/// Both observations demanded.
const ALL: GitStatusRefreshDemand = GitStatusRefreshDemand {
    branch: true,
    ahead_behind: true,
};

/// The refresh interval as the loop applies it.
const INTERVAL: Duration = super::GIT_REMOTE_STATUS_REFRESH_INTERVAL;

/// The instant a full refresh interval before `now`, when a refresh is due.
fn overdue(now: Instant) -> Instant {
    now.checked_sub(INTERVAL).unwrap()
}

/// A daemon that launches no pane process, with the sidebar rows of `config`.
fn test_app(config: &Config) -> TestApp {
    let (events, receiver) = mpsc::channel(8);
    let launch = PaneLaunchInputs {
        no_session: true,
        setup_entry: PtySetupEntry {
            executable: std::env::current_exe().unwrap(),
            args: Vec::new(),
        },
        socket: "/nonexistent/maestro-git-refresh.sock".into(),
        executable: None,
        manifests: Arc::new(RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
        events,
        render_notify: Arc::new(Notify::new()),
        render_dirty: Arc::new(RenderSignal::new()),
        theme: TerminalTheme::default(),
        appearance: None,
        process_detection_mode: ProcessDetectionMode::Native,
    };
    App::new(
        config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

/// A daemon whose workspace sidebar shows exactly `rows`.
fn app_with_rows(rows: Vec<Vec<SpaceSidebarToken>>) -> TestApp {
    let mut config = Config::default();
    config.ui.sidebar.spaces.rows = rows;
    test_app(&config)
}

/// Add a workspace rooted at `cwd` and return its handle.
fn add_workspace(app: &mut TestApp, cwd: &Path) -> String {
    app.state
        .topology
        .new_with_tab(cwd.to_path_buf(), None, |_, _| Ok(()))
        .unwrap();
    app.state.topology.workspaces.last().unwrap().id.clone()
}

/// An item for a directory with no remembered cache key.
fn item(workspace_id: &str, cwd: &Path, hint: Option<&Path>) -> WorkspaceGitRefreshItem {
    WorkspaceGitRefreshItem {
        workspace_id: workspace_id.into(),
        resolved_identity_cwd: cwd.to_path_buf(),
        cache_key_hint: hint.map(Path::to_path_buf),
    }
}

/// Observe the supplied items without user configuration.
fn refresh(
    items: Vec<WorkspaceGitRefreshItem>,
    cache: &HashMap<PathBuf, GitStatusCacheEntry>,
) -> WorkspaceGitRefreshOutput {
    refresh_workspace_git_statuses_with_cache_and_demand(
        items,
        cache,
        ALL,
        &ConfigEnvironment::default(),
    )
}

/// Run `git` in `directory`, which must succeed.
fn git(directory: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
}

#[test]
fn workspaces_in_one_repository_share_one_refresh_job() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let repo = directory.path().to_path_buf();
    let nested = repo.join("nested");
    let other = repo.join("other");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    git(&repo, &["init", "--quiet"]);

    let output = refresh(
        vec![item("one", &nested, None), item("two", &other, None)],
        &HashMap::new(),
    );

    assert_eq!(output.cache_updates.len(), 1);
    assert_eq!(
        output.cache_updates[0].0,
        std::fs::canonicalize(&repo).unwrap()
    );
    let workspaces: Vec<_> = output
        .results
        .iter()
        .map(|status| (status.workspace_id.as_str(), &status.resolved_identity_cwd))
        .collect();
    assert_eq!(workspaces, [("one", &nested), ("two", &other)]);
}

#[test]
fn shared_root_refresh_keeps_each_workspace_label() {
    let root = PathBuf::from("/");
    let cached = GitStatusCacheEntry {
        fingerprint: None,
        retry_after: Some(Instant::now() + Duration::from_secs(30)),
        snapshot: WorkspaceGitStatusSnapshot {
            auto_label: "/".into(),
            branch: Some("main".into()),
            ahead_behind: None,
            space: Some(GitSpaceMetadata {
                key: "/.git".into(),
                checkout_key: "/".into(),
                repo_name: "repo".into(),
                repo_root: root.clone(),
                is_linked_worktree: false,
            }),
        },
    };
    let items = ["alpha", "beta"]
        .map(|name| item(name, &root.join(name), Some(&root)))
        .to_vec();

    let output = refresh(items, &HashMap::from([(root, cached)]));

    assert_eq!(output.cache_updates.len(), 1);
    let labels: Vec<_> = output
        .results
        .iter()
        .map(|status| &*status.auto_label)
        .collect();
    assert_eq!(labels, ["alpha", "beta"]);
    assert!(
        output
            .results
            .iter()
            .all(|status| status.branch.as_deref() == Some("main"))
    );
}

#[test]
fn item_collection_leaves_an_uncached_directory_undiscovered() {
    let mut app = app_with_rows(Vec::new());
    add_workspace(&mut app, Path::new("/workspace"));
    let uncached = PathBuf::from("/uncached/directory");
    app.state.topology.workspaces[0]
        .identity_cwd
        .clone_from(&uncached);

    let items = app.workspace_git_refresh_items(false);

    assert_eq!(
        items,
        [item(&app.state.topology.workspaces[0].id, &uncached, None)]
    );
}

#[test]
fn item_collection_reuses_the_cached_key_for_the_same_directory() {
    let mut app = app_with_rows(Vec::new());
    add_workspace(&mut app, Path::new("/workspace"));
    let cwd = PathBuf::from("/repo/deep/nested");
    let workspace = &mut app.state.topology.workspaces[0];
    workspace.identity_cwd.clone_from(&cwd);
    workspace.cached_identity_cwd.clone_from(&cwd);
    workspace.cached_git_status_key = "/repo".into();

    let items = app.workspace_git_refresh_items(false);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].cache_key_hint.as_deref(), Some(Path::new("/repo")));
}

#[test]
fn periodic_discovery_ignores_cached_keys() {
    let mut app = app_with_rows(Vec::new());
    add_workspace(&mut app, Path::new("/workspace"));
    let cwd = PathBuf::from("/repo/deep/nested");
    let workspace = &mut app.state.topology.workspaces[0];
    workspace.identity_cwd.clone_from(&cwd);
    workspace.cached_identity_cwd.clone_from(&cwd);
    workspace.cached_git_status_key = "/repo".into();

    let items = app.workspace_git_refresh_items(true);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].cache_key_hint, None);
    let stale = GitStatusCacheEntry {
        fingerprint: None,
        retry_after: None,
        snapshot: WorkspaceGitStatusSnapshot {
            auto_label: "stale".into(),
            branch: None,
            ahead_behind: None,
            space: None,
        },
    };
    let cache = HashMap::from([(items[0].resolved_identity_cwd.clone(), stale)]);
    let jobs = deduplicate_git_refresh_items(items, &cache);
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].cached.is_none());
}

#[test]
fn directory_report_refreshes_once_without_git_tokens() {
    crate::tests::block_on(async {
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Workspace]]);
        add_workspace(&mut app, directory.path());
        let now = Instant::now();

        app.request_git_identity_refresh(now);

        assert!(app.git_refresh_deadline().is_some());
        app.start_git_status_refresh_if_due(now);
        assert!(app.git_refresh.in_flight);
        assert!(!app.git_refresh.identity_requested);
        let Some(AppEvent::GitStatusRefreshed { results, .. }) = app.event_rx.recv().await else {
            panic!("the refresh must publish its result");
        };
        assert_eq!(results.len(), 1);
    });
}

#[test]
fn due_refresh_does_not_start_without_a_sidebar_consumer() {
    crate::tests::block_on(async {
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let now = Instant::now();
        let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Workspace]]);
        add_workspace(&mut app, directory.path());
        app.git_refresh.last_remote_refresh = overdue(now);

        app.start_git_status_refresh_if_due(now);

        assert!(!app.git_refresh.in_flight);
        assert!(app.event_rx.try_recv().is_err());

        let mut consumer = app_with_rows(vec![vec![SpaceSidebarToken::Branch]]);
        add_workspace(&mut consumer, directory.path());
        consumer.git_refresh.last_remote_refresh = overdue(now);
        consumer.start_git_status_refresh_if_due(now);
        assert!(consumer.git_refresh.in_flight);
        assert!(consumer.event_rx.recv().await.is_some());
    });
}

#[test]
fn demand_follows_the_sidebar_rows() {
    let cases = [
        (
            SpaceSidebarToken::Workspace,
            GitStatusRefreshDemand::default(),
        ),
        (
            SpaceSidebarToken::Branch,
            GitStatusRefreshDemand {
                branch: true,
                ahead_behind: false,
            },
        ),
        (
            SpaceSidebarToken::GitStatus,
            GitStatusRefreshDemand {
                branch: false,
                ahead_behind: true,
            },
        ),
    ];
    for (token, expected) in cases {
        let mut app = app_with_rows(vec![vec![token.clone()]]);
        add_workspace(&mut app, Path::new("/workspace"));

        assert_eq!(app.git_refresh_demand(), expected, "token: {token:?}");
        assert_eq!(
            app.git_refresh_deadline().is_some(),
            !expected.is_empty(),
            "token: {token:?}"
        );
    }
}

#[test]
fn linked_worktrees_add_no_periodic_demand() {
    for custom_name in [None, Some("custom".to_owned())] {
        let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Workspace]]);
        add_workspace(&mut app, Path::new("/repo-worktree"));
        let workspace = &mut app.state.topology.workspaces[0];
        workspace.custom_name = custom_name;
        workspace.worktree_space = Some(maestro_workspace::workspace::WorktreeSpaceMembership {
            key: "repo".into(),
            label: "repo".into(),
            repo_root: "/repo".into(),
            checkout_path: "/repo-worktree".into(),
            is_linked_worktree: true,
        });

        assert_eq!(app.git_refresh_deadline(), None);

        app.config.ui.sidebar.spaces.rows = vec![vec![SpaceSidebarToken::Branch]];
        assert!(app.git_refresh_deadline().is_some());
    }
}

#[test]
fn loop_deadline_includes_the_due_refresh() {
    let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Branch]]);
    add_workspace(&mut app, Path::new("/workspace"));
    let now = Instant::now();
    app.git_refresh.last_remote_refresh = overdue(now);

    assert_eq!(app.next_headless_loop_deadline(now, false), Some(now));

    app.git_refresh.in_flight = true;
    assert_ne!(app.next_headless_loop_deadline(now, false), Some(now));
}

#[test]
fn scheduled_tasks_start_the_due_refresh() {
    crate::tests::block_on(async {
        let directory = maestro_test_directory::Scratch::new().unwrap();
        let paths =
            crate::paths::DaemonPaths::new(directory.path().to_str().unwrap(), None).unwrap();
        let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Branch]]);
        add_workspace(&mut app, directory.path());
        let now = Instant::now();
        app.git_refresh.last_remote_refresh = overdue(now);

        app.handle_scheduled_tasks(now, &paths).await;

        assert!(app.git_refresh.in_flight);
        assert!(app.event_rx.recv().await.is_some());
    });
}

#[test]
fn explicit_refresh_discards_cached_non_repository_results() {
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let repository = directory.path().join("repository");
    let outside = directory.path().join("outside");
    std::fs::create_dir_all(&repository).unwrap();
    git(&repository, &["init", "--quiet"]);
    let environment = ConfigEnvironment::default();
    let mut app = test_app(&Config::default());
    for cwd in [&repository, &outside] {
        let (_, entry) = git_status_snapshot_for_cwd_with_demand(cwd, None, ALL, &environment);
        app.git_refresh.cache.insert(cwd.clone(), entry.unwrap());
    }

    app.mark_git_status_refresh_due(Instant::now());

    assert_eq!(
        app.git_refresh.cache.keys().collect::<Vec<_>>(),
        [&repository]
    );
}

#[test]
fn due_request_made_in_flight_survives_the_refresh_result() {
    let mut app = test_app(&Config::default());
    app.git_refresh.in_flight = true;

    app.mark_git_status_refresh_due(Instant::now());
    assert!(app.git_refresh.due_after_in_flight);

    app.handle_internal_event(AppEvent::GitStatusRefreshed {
        results: Vec::new(),
        cache_updates: Vec::new(),
    });

    assert!(!app.git_refresh.in_flight);
    assert!(!app.git_refresh.due_after_in_flight);
    assert_eq!(app.git_refresh_deadline(), None);
    add_workspace(&mut app, Path::new("/workspace"));
    let deadline = app.git_refresh_deadline().unwrap();
    assert!(deadline <= Instant::now());
}

#[test]
fn terminal_directory_report_requests_a_git_identity_refresh() {
    let mut app = app_with_rows(vec![vec![SpaceSidebarToken::Workspace]]);
    add_workspace(&mut app, Path::new("/workspace"));
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;

    app.handle_internal_event(AppEvent::TerminalCwdReported {
        pane_id: pane,
        cwd: "/workspace".into(),
    });

    assert!(app.git_refresh.identity_requested);
    assert!(app.git_refresh_deadline().is_some());
}

#[test]
fn refresh_publishes_its_result_when_the_refresh_panics() {
    crate::tests::block_on(async {
        let (events, mut receiver) = mpsc::channel(1);

        spawn_refresh(events, || panic!("the refresh failed"));

        let event = receiver.recv().await;
        assert!(matches!(
            event,
            Some(AppEvent::GitStatusRefreshed { results, cache_updates })
                if results.is_empty() && cache_updates.is_empty()
        ));
    });
}

#[test]
fn daemon_git_refresh_deadline_is_suppressed_while_in_flight() {
    let mut app = test_app(&Config::default());
    add_workspace(&mut app, Path::new("/workspace"));
    assert!(app.git_refresh_deadline().is_some());
    app.git_refresh.in_flight = true;

    assert_eq!(app.git_refresh_deadline(), None);
}

#[test]
fn daemon_unchanged_git_status_event_has_no_render_impact() {
    let mut app = test_app(&Config::default());
    app.git_refresh.in_flight = true;

    let changed = app.handle_internal_event_with_render_impact(AppEvent::GitStatusRefreshed {
        results: Vec::new(),
        cache_updates: Vec::new(),
    });

    assert!(!changed);
    assert!(!app.git_refresh.in_flight);
}

#[test]
fn daemon_git_status_event_clears_in_flight_refresh() {
    let mut app = test_app(&Config::default());
    app.git_refresh.in_flight = true;
    let previous_refresh = Instant::now().checked_sub(Duration::from_secs(10)).unwrap();
    app.git_refresh.last_remote_refresh = previous_refresh;

    app.handle_internal_event(AppEvent::GitStatusRefreshed {
        results: Vec::new(),
        cache_updates: Vec::new(),
    });

    assert!(!app.git_refresh.in_flight);
    assert!(app.git_refresh.last_remote_refresh > previous_refresh);
}

#[test]
fn daemon_git_status_event_marks_render_dirty_when_status_changes() {
    let mut app = test_app(&Config::default());
    add_workspace(&mut app, Path::new("/workspace"));
    let _ = app.launch.render_dirty.take();
    let workspace = &app.state.topology.workspaces[0];
    let cwd = workspace.identity_cwd.clone();
    let result = maestro_workspace::workspace::WorkspaceGitStatus {
        workspace_id: workspace.id.clone(),
        resolved_identity_cwd: cwd.clone(),
        status_cache_key: cwd,
        demand: ALL,
        auto_label: "workspace".into(),
        branch: Some("render-dirty-test".into()),
        ahead_behind: Some((1, 0)),
        space: None,
    };

    app.handle_internal_event(AppEvent::GitStatusRefreshed {
        results: vec![result],
        cache_updates: Vec::new(),
    });

    assert!(app.launch.render_dirty.is_pending());
}

#[test]
fn daemon_unchanged_git_status_drain_has_no_render_impact() {
    let mut app = test_app(&Config::default());
    app.git_refresh.in_flight = true;
    app.launch
        .events
        .try_send(AppEvent::GitStatusRefreshed {
            results: Vec::new(),
            cache_updates: Vec::new(),
        })
        .unwrap();

    assert!(!app.drain_internal_events());
    assert!(!app.git_refresh.in_flight);
}

/// Git reads HOME as set on every platform and never substitutes the account home.
#[test]
fn git_config_home_uses_only_home_variable() {
    let cases = [
        (None, None, ""),
        (None, Some(""), ""),
        (None, Some("x"), "x/git/config"),
        (Some(""), None, "/.config/git/config\n/.gitconfig"),
        (Some(""), Some(""), "/.config/git/config\n/.gitconfig"),
        (Some(""), Some("x"), "x/git/config\n/.gitconfig"),
        (Some("h"), None, "h/.config/git/config\nh/.gitconfig"),
        (Some("h"), Some(""), "h/.config/git/config\nh/.gitconfig"),
        (Some("h"), Some("x"), "x/git/config\nh/.gitconfig"),
    ];
    let Some(cases) = maestro_test_process::run_per_environment(
        "app::git_refresh::tests::git_config_home_uses_only_home_variable",
        cases,
        |(home, xdg, _)| {
            [("HOME", *home), ("XDG_CONFIG_HOME", *xdg)]
                .into_iter()
                .filter_map(|(key, value)| value.map(|value| (key, value)))
                .collect()
        },
    )
    .unwrap() else {
        return;
    };
    for (home, _, files) in cases {
        let environment = git_config_environment();
        assert_eq!(environment.home, home.map(PathBuf::from), "{home:?}");
        let expected: Vec<_> = files.lines().map(PathBuf::from).collect();
        assert_eq!(environment.user_files, expected);
    }
}

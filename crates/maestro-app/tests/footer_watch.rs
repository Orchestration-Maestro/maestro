//! Live footer refresh through controlled and native effects.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[cfg(unix)]
use crate::native;
use crate::watching;
use maestro_app::presentation_data::footer_data_provider::{
    FooterDataProvider, FooterFileStats, ReadonlyFooterDataProvider,
};
#[cfg(any(unix, windows))]
use maestro_app::presentation_data::footer_data_provider::{
    FooterOperations, NativeFooterOperations,
};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
    time::Duration,
};
use watching::Effects;

type Seen = Rc<RefCell<Vec<Option<String>>>>;
fn provider(effects: &Rc<Effects>, cwd: &str) -> FooterDataProvider {
    FooterDataProvider::new(effects.path(cwd), effects.clone())
}
fn subscribed(p: &FooterDataProvider) -> Seen {
    let seen = Seen::default();
    let reader = p.clone();
    let result = seen.clone();
    drop(p.on_branch_change(Rc::new(move || {
        seen.borrow_mut().push(reader.get_git_branch());
    })));
    result
}
fn begin(e: &Effects) {
    e.event(1, None);
    e.advance(500);
}
fn cases<T: serde::de::DeserializeOwned>(source: &str) -> Vec<T> {
    serde_json::from_str(source).unwrap()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    entries: Vec<String>,
    cwd: String,
    paths: Vec<String>,
    polls: Vec<ExpectedPoll>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedPoll {
    path: String,
    interval: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Filename {
    filename: Option<String>,
    branch: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Stats {
    mtime_ms: f64,
    ctime_ms: f64,
    size: f64,
}
impl From<Stats> for FooterFileStats {
    fn from(s: Stats) -> Self {
        Self {
            mtime_ms: s.mtime_ms,
            ctime_ms: s.ctime_ms,
            size: s.size,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatCase {
    before: Stats,
    after: Stats,
    calls: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    head: String,
    branch: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "outcome", rename_all = "lowercase")]
enum OutputCase {
    Success { stdout: String, branch: String },
    Failure { branch: String },
}

#[test]
fn footer_watch_selection_tracks_common_metadata() {
    let stars = keeps_stars();
    for case in cases::<Selection>(include_str!("fixtures/footer_selection.json")) {
        if !stars
            && case
                .entries
                .iter()
                .chain([&case.cwd])
                .any(|path| path.contains('*'))
        {
            continue;
        }
        let e = Rc::new(Effects::default());
        for path in case.entries {
            if path.ends_with("HEAD") || path.ends_with("tables.list") {
                e.text(&path, "ref: refs/heads/main");
            } else {
                e.directory(&path);
            }
        }
        let p = provider(&e, &case.cwd);
        assert_eq!(
            e.active(),
            case.paths.iter().map(|p| e.path(p)).collect::<Vec<_>>()
        );
        let polls: Vec<_> = e
            .polls
            .borrow()
            .iter()
            .map(|p| (p.path.clone(), p.interval))
            .collect();
        assert_eq!(
            polls,
            case.polls
                .into_iter()
                .map(|p| (e.path(&p.path), Duration::from_millis(p.interval)))
                .collect::<Vec<_>>()
        );
        assert_eq!(e.pending_timers(), 0);
        p.dispose();
    }
    if stars {
        literal_pattern_worktree_links();
    }
}

/// Whether this host stores a name holding `*`; Windows refuses one, so the literal-pattern
/// premise cannot be exercised there.
fn keeps_stars() -> bool {
    let name = std::ffi::OsStr::new("storage [one]*");
    let kept = maestro_test_directory::Scratch::new()
        .unwrap()
        .keeps_distinct(&[name])
        .unwrap();
    if !kept {
        eprintln!("premise not met: this host refuses the name {name:?}");
    }
    kept
}

/// Worktree links through a directory whose name holds pattern characters stay literal.
fn literal_pattern_worktree_links() {
    for link in [
        "/storage [one]*/worktrees/one",
        "../storage [one]*/worktrees/one",
    ] {
        let e = Rc::new(Effects::default());
        e.text("/repo/.git", &format!("gitdir: {link}"));
        e.text("/storage [one]*/worktrees/one/HEAD", "ref: refs/heads/main");
        e.text("/storage [one]*/worktrees/one/commondir", "../..");
        e.directory("/storage [one]*/reftable");
        e.text("/storage [one]*/reftable/tables.list", "0");
        e.directory("/storage o/worktrees/one/reftable");
        let p = provider(&e, "/repo");
        assert_eq!(
            e.active(),
            e.paths([
                "/storage [one]*/worktrees/one",
                "/storage [one]*/reftable",
                "/storage [one]*/reftable/tables.list"
            ])
        );
        p.dispose();
    }
}

#[test]
fn footer_head_events_filter_before_scheduling() {
    for case in cases::<Filename>(include_str!("fixtures/footer_filenames.json")) {
        let e = Effects::repo();
        e.text("/repo/.git/HEAD", "ref: refs/heads/old");
        let p = provider(&e, "/repo");
        p.get_git_branch();
        e.text("/repo/.git/HEAD", "ref: refs/heads/new");
        e.event(0, case.filename.as_deref());
        e.advance(500);
        assert_eq!(p.get_git_branch().as_deref(), Some(case.branch.as_str()));
        p.dispose();
    }
    for source in [1, 2] {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        e.event(source, Some("unrelated.table"));
        e.advance(500);
        assert_eq!(e.query_dirs.borrow().as_slice(), e.paths(["/repo"]));
        e.answer("updated");
        assert_eq!(p.get_git_branch().as_deref(), Some("updated"));
        p.dispose();
    }
}

#[test]
fn footer_poll_compares_each_stat_dimension() {
    for case in cases::<StatCase>(include_str!("fixtures/footer_stats.json")) {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        e.poll(0, case.after.into(), case.before.into());
        e.advance(500);
        assert_eq!(e.queries.borrow().len(), case.calls);
        if case.calls == 1 {
            e.answer("poll-result");
            assert_eq!(p.get_git_branch().as_deref(), Some("poll-result"));
        }
        p.dispose();
    }
}

#[test]
fn footer_first_async_lookup_primes_without_notification() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    let notifications = Rc::new(Cell::new(0));
    let log = notifications.clone();
    let _keep = p.on_branch_change(Rc::new(move || log.set(log.get() + 1)));
    begin(&e);
    e.answer("primed\n");
    assert_eq!(p.get_git_branch().as_deref(), Some("primed"));
    assert_eq!(e.sync_calls.get(), 0);
    assert_eq!(notifications.get(), 0);
    p.dispose();
}

#[test]
fn footer_burst_refresh_uses_first_event_deadline() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    e.event(0, Some("HEAD"));
    e.advance(400);
    e.event(1, Some("next.table"));
    e.event(2, None);
    e.poll(
        0,
        FooterFileStats {
            size: 1.0,
            ..FooterFileStats::default()
        },
        FooterFileStats::default(),
    );
    e.advance(99);
    assert!(e.queries.borrow().is_empty(), "no query at 499 ms");
    e.advance(1);
    assert_eq!(
        e.queries.borrow().len(),
        1,
        "one query at first 500 ms deadline"
    );
    e.answer("coalesced");
    assert_eq!(p.get_git_branch().as_deref(), Some("coalesced"));
    p.dispose();
}

#[test]
fn footer_inflight_events_queue_one_later_refresh() {
    for answer in ["main", "first"] {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        p.get_git_branch();
        let seen = subscribed(&p);
        begin(&e);
        e.event(1, None);
        e.event(2, None);
        e.answer(answer);
        assert_eq!(p.get_git_branch().as_deref(), Some(answer));
        e.advance(499);
        assert!(e.queries.borrow().is_empty());
        e.advance(1);
        assert_eq!(e.queries.borrow().len(), 1);
        e.answer("second");
        assert_eq!(
            e.query_dirs.borrow().as_slice(),
            e.paths(["/repo", "/repo"])
        );
        let expected = if answer == "main" {
            vec![Some("second".into())]
        } else {
            vec![Some("first".into()), Some("second".into())]
        };
        assert_eq!(*seen.borrow(), expected);
        p.dispose();
    }
}

#[test]
fn footer_registration_failures_retry_after_five_seconds() {
    for fail in 1..=3 {
        let e = Effects::repo();
        *e.failures.borrow_mut() = vec![fail];
        let p = provider(&e, "/repo");
        assert!(e.active().is_empty());
        assert_eq!(e.live_polls(), 0);
        e.advance(4999);
        assert_eq!(e.attempts.get(), fail);
        e.advance(1);
        assert_eq!(e.active().len(), 3);
        assert_eq!(e.live_polls(), 1);
        p.dispose();
    }
    let e = Effects::repo();
    *e.failures.borrow_mut() = vec![1, 2, 3];
    let p = provider(&e, "/repo");
    let mut attempts = vec![e.attempts.get()];
    for _ in 0..3 {
        e.advance(4999);
        attempts.push(e.attempts.get());
        e.advance(1);
        attempts.push(e.attempts.get());
    }
    assert_eq!(attempts, [1, 1, 2, 2, 3, 3, 6]);
    assert_eq!(e.active().len(), 3);
    p.dispose();
    let e = Rc::new(Effects::default());
    let p = provider(&e, "/missing");
    e.advance(15000);
    assert_eq!(e.attempts.get(), 0);
    assert_eq!(e.pending_timers(), 0);
    p.dispose();
}

#[test]
fn footer_async_watch_failure_closes_and_retries() {
    for failed in 0..3 {
        let e = Effects::repo();
        e.close_failure.set(failed + 1);
        let p = provider(&e, "/repo");
        e.error(failed);
        e.error((failed + 1) % 3);
        assert!(e.active().is_empty(), "error releases all watches");
        assert_eq!(e.live_polls(), 0);
        assert_eq!(e.pending_timers(), 1);
        e.advance(4999);
        assert_eq!(e.attempts.get(), 3);
        e.advance(1);
        assert_eq!(e.active().len(), 3);
        assert_eq!(e.live_polls(), 1);
        p.dispose();
        e.error(failed);
        e.advance(5000);
        assert_eq!(e.attempts.get(), 6);
        assert_eq!(e.pending_timers(), 0);
    }
}

#[test]
fn footer_cwd_change_rejects_old_completion() {
    cwd_plain_completion();
    for (primed, pending, failed) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let e = Effects::repo();
        e.text("/other/.git/HEAD", "ref: refs/heads/.invalid");
        let p = provider(&e, "/repo");
        p.get_git_branch();
        begin(&e);
        if pending {
            e.event(1, None);
        }
        let calls = Rc::new(Cell::new(0));
        let log = calls.clone();
        let _keep = p.on_branch_change(Rc::new(move || log.set(log.get() + 1)));
        p.set_cwd(e.path("/other"));
        if primed {
            p.get_git_branch();
        }
        if failed {
            e.finish(Err(io::Error::other("old query")));
        } else {
            e.answer("stale");
        }
        assert_eq!(
            p.get_git_branch().as_deref(),
            Some("main"),
            "old cwd result cannot publish"
        );
        assert_eq!(calls.get(), 1);
        e.advance(499);
        assert!(e.queries.borrow().is_empty());
        e.advance(1);
        assert_eq!(e.queries.borrow().len(), usize::from(pending));
        if pending {
            assert_eq!(
                e.query_dirs.borrow().as_slice(),
                e.paths(["/repo", "/other"])
            );
            e.answer("new-cwd");
            assert_eq!(p.get_git_branch().as_deref(), Some("new-cwd"));
        }
        p.dispose();
    }
}

#[test]
fn footer_disposal_rejects_late_work() {
    for pending in [false, true] {
        for failed in [false, true] {
            let e = Effects::repo();
            let p = provider(&e, "/repo");
            p.get_git_branch();
            let seen = subscribed(&p);
            begin(&e);
            if pending {
                e.event(1, None);
            }
            p.dispose();
            e.event(0, Some("HEAD"));
            e.error(1);
            if failed {
                e.finish(Err(io::Error::other("late query")));
            } else {
                e.answer("late");
            }
            e.advance(10000);
            assert_eq!(p.get_git_branch().as_deref(), Some("main"));
            assert!(seen.borrow().is_empty());
            assert_eq!(e.query_dirs.borrow().len(), 1);
            assert_eq!(e.pending_timers(), 0);
        }
    }
}

#[test]
fn footer_async_lookup_preserves_fallback_and_trim() {
    for case in cases::<Head>(include_str!("fixtures/footer_heads.json")) {
        let e = Effects::repo();
        e.text("/repo/.git/HEAD", "ref: refs/heads/initial");
        let p = provider(&e, "/repo");
        p.get_git_branch();
        let seen = subscribed(&p);
        e.text("/repo/.git/HEAD", &case.head);
        begin(&e);
        assert_eq!(p.get_git_branch().as_deref(), Some(case.branch.as_str()));
        assert_eq!(*seen.borrow(), vec![Some(case.branch)]);
        assert!(e.queries.borrow().is_empty());
        p.dispose();
    }
    for case in cases::<OutputCase>(include_str!("fixtures/footer_stdout.json")) {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        p.get_git_branch();
        let seen = subscribed(&p);
        begin(&e);
        let branch = match case {
            OutputCase::Success { stdout, branch } => {
                e.answer(&stdout);
                branch
            }
            OutputCase::Failure { branch } => {
                e.finish(Err(io::Error::other("process failed")));
                branch
            }
        };
        assert_eq!(p.get_git_branch().as_deref(), Some(branch.as_str()));
        assert_eq!(*seen.borrow(), vec![Some(branch)]);
        p.dispose();
    }
    for outcome in [
        Ok(None),
        Err(io::Error::other("unavailable")),
        Ok(Some(" \r\n".into())),
    ] {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        begin(&e);
        e.finish(outcome);
        assert_eq!(p.get_git_branch().as_deref(), Some("detached"));
        p.dispose();
    }
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    e.repository.remove("/repo/.git/HEAD");
    begin(&e);
    assert_eq!(p.get_git_branch(), None);
    assert!(e.query_dirs.borrow().is_empty());
    p.dispose();
    let e = Rc::new(Effects::default());
    let p = provider(&e, "/absent");
    assert_eq!(p.get_git_branch(), None);
    p.dispose();
}

#[test]
fn footer_dispose_during_callback_stops_walk() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    let calls = Rc::new(Cell::new(0));
    let log = calls.clone();
    let owner = p.clone();
    drop(p.on_branch_change(Rc::new(move || {
        log.set(log.get() + 1);
        owner.dispose();
    })));
    let second = calls.clone();
    drop(p.on_branch_change(Rc::new(move || second.set(second.get() + 10))));
    begin(&e);
    e.event(1, None);
    e.answer("updated");
    assert_eq!(calls.get(), 1);
    assert!(e.active().is_empty());
    assert_eq!(e.pending_timers(), 0);
    assert_eq!(p.get_git_branch().as_deref(), Some("updated"));
    p.dispose();
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    let other = e.path("/other");
    let reader = p.clone();
    let moved = Cell::new(false);
    drop(p.on_branch_change(Rc::new(move || {
        if !moved.replace(true) {
            reader.set_cwd(other.clone());
        }
    })));
    begin(&e);
    e.event(1, None);
    e.answer("new");
    e.advance(500);
    assert_eq!(p.get_git_branch().as_deref(), Some("other"));
    assert_eq!(e.query_dirs.borrow().as_slice(), e.paths(["/repo"]));
    assert_eq!(e.active(), e.paths(["/other/.git"]));
    p.dispose();
}

#[test]
fn footer_disposal_cancels_registered_sources() {
    for retry in [false, true] {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        e.event(1, None);
        if retry {
            e.error(1);
        }
        p.dispose();
        p.dispose();
        assert!(e.active().is_empty());
        assert_eq!(e.live_polls(), 0);
        assert_eq!(e.pending_timers(), 0);
        e.directory("/other/.git/reftable");
        e.text("/other/.git/reftable/tables.list", "0");
        let seen = subscribed(&p);
        p.set_cwd(e.path("/other"));
        assert_eq!(p.get_git_branch().as_deref(), Some("other"));
        assert_eq!(*seen.borrow(), [Some("other".into())]);
        assert!(e.active().is_empty());
        assert_eq!(e.live_polls(), 0);
        e.advance(10000);
        assert!(e.queries.borrow().is_empty());
        p.dispose();
    }
}

#[test]
fn footer_watch_error_keeps_pending_refresh() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    e.event(1, None);
    e.error(1);
    e.advance(500);
    assert_eq!(e.queries.borrow().len(), 1);
    e.answer("after-error");
    assert_eq!(p.get_git_branch().as_deref(), Some("after-error"));
    assert!(e.active().is_empty());
    assert_eq!(e.pending_timers(), 1);
    p.dispose();
}

#[test]
fn footer_cwd_change_cancels_old_debounce_and_retry() {
    for same in [false, true] {
        let e = Effects::repo();
        let p = provider(&e, "/repo");
        p.get_git_branch();
        e.event(1, None);
        e.error(1);
        p.set_cwd(e.path(if same { "/repo" } else { "/absent" }));
        assert_eq!(e.pending_timers(), if same { 2 } else { 0 });
        e.advance(500);
        assert_eq!(e.queries.borrow().len(), usize::from(same));
        if same {
            e.answer("main");
            assert_eq!(p.get_git_branch().as_deref(), Some("main"));
        } else {
            assert_eq!(p.get_git_branch(), None);
        }
        e.advance(4500);
        assert_eq!(e.active().len(), if same { 3 } else { 0 });
        p.dispose();
    }
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    let seen = subscribed(&p);
    p.set_cwd(e.path("/other"));
    assert_eq!(e.active(), e.paths(["/other/.git"]));
    assert_eq!(e.live_polls(), 0);
    assert_eq!(*seen.borrow(), [Some("other".into())]);
    p.dispose();
}

#[test]
fn footer_reftable_refresh_keeps_unchanged_branch_silent() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    assert_eq!(p.get_git_branch().as_deref(), Some("main"));
    let seen = subscribed(&p);
    begin(&e);
    e.answer("main\n");
    assert_eq!(p.get_git_branch().as_deref(), Some("main"));
    assert!(seen.borrow().is_empty(), "equal result stays silent");
    assert_eq!(e.sync_calls.get(), 1);
    assert_eq!(e.query_dirs.borrow().len(), 1);
    p.dispose();
}

#[test]
fn footer_reftable_refresh_publishes_changed_branch() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    let seen = subscribed(&p);
    begin(&e);
    e.answer("foo");
    assert_eq!(*seen.borrow(), [Some("foo".into())]);
    assert_eq!(p.get_git_branch().as_deref(), Some("foo"));
    assert_eq!(e.sync_calls.get(), 1);
    p.dispose();
}

#[test]
fn footer_drop_releases_captured_handles() {
    let e = Effects::repo();
    let weak = Rc::downgrade(&e);
    let p = provider(&e, "/repo");
    let statuses = p.get_extension_statuses();
    p.set_extension_status("kept", Some("value"));
    let unsubscribe = p.on_branch_change(Rc::new(|| {}));
    e.event(0, Some("HEAD"));
    drop(p);
    assert_eq!(e.active().len(), 3);
    assert_eq!(e.live_polls(), 1);
    drop(unsubscribe);
    assert!(e.active().is_empty());
    assert_eq!(e.live_polls(), 0);
    assert_eq!(e.pending_timers(), 0);
    e.teardown();
    drop(e);
    assert!(weak.upgrade().is_none());
    assert_eq!(statuses.get("kept").as_deref(), Some("value"));
    #[cfg(unix)]
    native_alias_release();
}

#[test]
fn footer_disposal_keeps_other_provider_polling() {
    let e = Effects::repo();
    let first = provider(&e, "/repo");
    let second = provider(&e, "/repo");
    second.get_git_branch();
    let seen = subscribed(&second);
    first.dispose();
    drop(first);
    assert_eq!(
        e.live_polls(),
        1,
        "other provider retains its poll registration"
    );
    e.poll(
        1,
        FooterFileStats {
            ctime_ms: 1.0,
            ..FooterFileStats::default()
        },
        FooterFileStats::default(),
    );
    e.advance(500);
    e.answer("other-provider");
    assert_eq!(*seen.borrow(), [Some("other-provider".into())]);
    second.dispose();
    assert_eq!(e.live_polls(), 0);
}
#[cfg(any(unix, windows))]
fn native_runtime() -> (tokio::runtime::Runtime, Rc<tokio::task::LocalSet>) {
    (
        crate::support::io_runtime(),
        Rc::new(tokio::task::LocalSet::new()),
    )
}
#[cfg(unix)]
#[test]
fn footer_native_watch_observes_atomic_head_replacement() {
    let root = native::Scratch::new();
    native::git(&root.join(""), &["init", "-q", "--initial-branch=main"]);
    let (runtime, local) = native_runtime();
    let p = FooterDataProvider::new(
        root.join(""),
        Rc::new(NativeFooterOperations::new(local.clone())),
    );
    assert_eq!(p.get_git_branch().as_deref(), Some("main"));
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = RefCell::new(Some(tx));
    let reader = p.clone();
    drop(p.on_branch_change(Rc::new(move || {
        let branch = reader.get_git_branch();
        if let Some(tx) = tx.borrow_mut().take() {
            tx.send(branch).unwrap();
        }
    })));
    native::git(
        &root.join(""),
        &["symbolic-ref", "HEAD", "refs/heads/updated"],
    );
    assert_eq!(
        local.block_on(&runtime, rx).unwrap().as_deref(),
        Some("updated")
    );
    assert_eq!(p.get_git_branch().as_deref(), Some("updated"));
    p.dispose();
    drop(p);
    finish_native(&runtime, local);
}
#[cfg(unix)]
fn native_poll_ctime() {
    use std::os::unix::fs::PermissionsExt;
    let root = native::Scratch::new();
    let path = root.join("table");
    std::fs::write(&path, "content").unwrap();
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    let first = ops.watch_file(&path, Duration::from_millis(250), Rc::new(|_, _| {}));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let second = ops.watch_file(
        &path,
        Duration::from_millis(250),
        Rc::new(move |now, prev| {
            tx.send((now, prev)).unwrap();
        }),
    );
    drop(first);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    local.block_on(&runtime, async {
        let (now, prev) = rx.recv().await.unwrap();
        assert_ne!(now.ctime_ms.to_bits(), prev.ctime_ms.to_bits());
        assert_eq!(now.mtime_ms.to_bits(), prev.mtime_ms.to_bits());
        assert_eq!(now.size.to_bits(), prev.size.to_bits());
        drop(second);
        assert!(rx.recv().await.is_none(), "drop releases the producer");
    });
    drop(ops);
    finish_native(&runtime, local);
}
#[cfg(unix)]
fn native_poll_reappearance() {
    let root = native::Scratch::new();
    let live = root.join("live");
    let parked = root.join("parked");
    std::fs::create_dir(&live).unwrap();
    let path = root.join("live/tables.list");
    std::fs::write(&path, "content").unwrap();
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let listener = Rc::new(move |now, prev| {
        tx.send((now, prev)).unwrap();
    });
    let weak = Rc::downgrade(&listener);
    let mut watch = ops.watch_file(&path, Duration::from_millis(250), listener);
    std::fs::rename(&live, &parked).unwrap();
    local.block_on(&runtime, async {
        let (missing, successful) = rx.recv().await.unwrap();
        assert_eq!(missing, FooterFileStats::default());
        assert_eq!(successful.size.to_bits(), 7.0_f64.to_bits());
        std::fs::rename(&parked, &live).unwrap();
        let (returned, previous) = rx.recv().await.unwrap();
        assert!(
            previous == successful,
            "reappearance retains last successful sample"
        );
        assert!(
            returned == previous,
            "unchanged reappearance preserves selected stats"
        );
        watch.close().unwrap();
        drop(watch);
        assert!(rx.recv().await.is_none());
        assert!(weak.upgrade().is_none());
    });
    drop(ops);
    finish_native(&runtime, local);
}
#[cfg(unix)]
fn native_poll_milliseconds() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = native::Scratch::new();
    let path = root.join("table");
    std::fs::write(&path, "content").unwrap();
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watch = ops.watch_file(
        &path,
        Duration::from_millis(250),
        Rc::new(move |now, _| {
            tx.send(now.mtime_ms).unwrap();
        }),
    );
    local.block_on(&runtime, async {
        for (seconds, nanos, expected) in [
            (0, 0, 0.0_f64),
            (0, 1000, 0.001),
            (1_700_000_000, 123_456_001, 1_700_000_000_123.456),
            (2_147_483_647, 999_999_046, 2_147_483_647_999.999),
        ] {
            let time: SystemTime = UNIX_EPOCH + Duration::new(seconds, nanos);
            let file = std::fs::File::open(&path).unwrap();
            file.set_times(std::fs::FileTimes::new().set_modified(time))
                .unwrap();
            let actual = rx.recv().await.unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        watch.close().unwrap();
        drop(watch);
        assert!(rx.recv().await.is_none());
    });
    drop(ops);
    finish_native(&runtime, local);
}
#[cfg(unix)]
#[test]
fn footer_native_poll_detects_change_time() {
    native_poll_ctime();
    native_poll_reappearance();
    native_poll_milliseconds();
    native_poll_callback_teardown(true);
    native_poll_callback_teardown(false);
    native_poll_overdue_teardown(true);
    native_poll_overdue_teardown(false);
}
/// Windows has no stable file change time, so a rewrite is seen through the modification
/// time, which also stands in for the change time.
#[cfg(windows)]
#[test]
fn footer_native_poll_detects_a_replaced_table_on_windows() {
    let root = maestro_test_directory::Scratch::new().unwrap();
    let (path, staged) = (
        root.path().join("tables.list"),
        root.path().join("tables.new"),
    );
    std::fs::write(&path, "content").unwrap();
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watch = ops.watch_file(
        path.to_str().unwrap(),
        Duration::from_millis(250),
        Rc::new(move |now, prev| {
            tx.send((now, prev)).unwrap();
        }),
    );
    std::fs::write(&staged, "CONTENT").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&staged)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1)),
        )
        .unwrap();
    std::fs::rename(&staged, &path).unwrap();
    local.block_on(&runtime, async {
        let (now, prev): (FooterFileStats, FooterFileStats) = rx.recv().await.unwrap();
        assert_eq!(now.mtime_ms.to_bits(), 1000.0_f64.to_bits());
        assert_eq!(now.ctime_ms.to_bits(), now.mtime_ms.to_bits());
        assert_eq!(prev.ctime_ms.to_bits(), prev.mtime_ms.to_bits());
        assert_eq!(now.size.to_bits(), prev.size.to_bits());
        watch.close().unwrap();
        drop(watch);
        assert!(rx.recv().await.is_none());
    });
    drop(ops);
    finish_native(&runtime, local);
}
#[cfg(unix)]
fn native_query(
    ops: &NativeFooterOperations,
    local: &tokio::task::LocalSet,
    runtime: &tokio::runtime::Runtime,
    cwd: &str,
) -> io::Result<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    ops.symbolic_ref_async(
        cwd,
        Box::new(move |output| {
            tx.send(output).unwrap();
        }),
    );
    local.block_on(runtime, rx).unwrap()
}
#[cfg(unix)]
#[test]
fn footer_native_async_lookup_runs_selected_command() {
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    if let Ok(repo) = std::env::var("FOOTER_ASYNC_REPO") {
        let result = native_query(&ops, &local, &runtime, &repo).unwrap();
        for line in result.unwrap_or_else(|| "absent".into()).lines() {
            println!("\nPROBE {line}");
        }
        drop(ops);
        finish_native(&runtime, local);
        return;
    }
    let root = native::Scratch::new();
    native::git(&root.join(""), &["init", "-q", "--initial-branch=main"]);
    assert_eq!(
        native_query(&ops, &local, &runtime, &root.join(""))
            .unwrap()
            .as_deref(),
        Some("main\n")
    );
    std::fs::write(
        root.join(".git/HEAD"),
        "0123456789abcdef0123456789abcdef01234567ab\n",
    )
    .unwrap();
    assert_eq!(
        native_query(&ops, &local, &runtime, &root.join("")).unwrap(),
        None
    );
    assert!(native_query(&ops, &local, &runtime, &root.join("missing")).is_err());
    native_command_probe(&root);
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    begin(&e);
    assert_eq!(e.query_dirs.borrow().as_slice(), e.paths(["/repo"]));
    e.finish(Err(io::Error::other("process failed")));
    assert_eq!(p.get_git_branch().as_deref(), Some("detached"));
    p.dispose();
    drop(ops);
    finish_native(&runtime, local);
}

#[cfg(unix)]
fn native_command_probe(root: &native::Scratch) {
    std::fs::create_dir(root.join("bin")).unwrap();
    std::fs::write(
        root.join("git.sh"),
        "#!/bin/sh\nprintf '%s\\n' \"$*\"\npwd -P\nexit \"${FOOTER_ASYNC_EXIT:-0}\"\n",
    )
    .unwrap();
    native::install_executable(
        std::path::Path::new(&root.join("git.sh")),
        std::path::Path::new(&root.join("bin/git")),
    );
    let output = native::probe(
        maestro_test_process::TestProcess::current(
            "footer_native_async_lookup_runs_selected_command",
        )
        .unwrap()
        .case("*"),
        root.0.path(),
        &[
            ("FOOTER_ASYNC_REPO", &root.join("")),
            ("PATH", &root.join("bin")),
        ],
        &[],
    );
    assert_eq!(
        output,
        [
            "--no-optional-locks symbolic-ref --quiet --short HEAD",
            std::fs::canonicalize(root.0.path())
                .unwrap()
                .to_str()
                .unwrap()
        ]
    );
    let failed = native::probe(
        maestro_test_process::TestProcess::current(
            "footer_native_async_lookup_runs_selected_command",
        )
        .unwrap()
        .case("*"),
        root.0.path(),
        &[
            ("FOOTER_ASYNC_REPO", &root.join("")),
            ("PATH", &root.join("bin")),
            ("FOOTER_ASYNC_EXIT", "1"),
        ],
        &[],
    );
    assert_eq!(
        failed,
        ["absent"],
        "unsuccessful exit discards nonempty stdout"
    );
}

#[cfg(unix)]
fn native_alias_release() {
    let root = native::Scratch::new();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
    let (_, local) = native_runtime();
    let weak_local = Rc::downgrade(&local);
    let ops = Rc::new(NativeFooterOperations::new(local.clone()));
    let weak_ops = Rc::downgrade(&ops);
    let p = FooterDataProvider::new(root.join(""), ops.clone());
    let statuses = p.get_extension_statuses();
    let unsubscribe = p.on_branch_change(Rc::new(|| {}));
    drop((p, ops, local));
    assert!(weak_ops.upgrade().is_some());
    assert!(weak_local.upgrade().is_some());
    drop(unsubscribe);
    assert!(weak_ops.upgrade().is_none());
    assert!(weak_local.upgrade().is_none());
    assert!(statuses.is_empty());
}
#[cfg(unix)]
fn native_poll_callback_teardown(close: bool) {
    use maestro_watch::fs_watch::FsWatcher;
    let root = native::Scratch::new();
    let path = root.join("table");
    std::fs::write(&path, "initial").unwrap();
    let (runtime, local) = native_runtime();
    let ops = NativeFooterOperations::new(local.clone());
    let slot = Rc::new(RefCell::new(None::<Box<dyn FsWatcher>>));
    let captured = slot.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let listener = Rc::new(move |now: FooterFileStats, _: FooterFileStats| {
        let mut registration = captured.borrow_mut().take().unwrap();
        if close {
            registration.close().unwrap();
        }
        drop(registration);
        tx.send(now.size).unwrap();
    });
    let weak = Rc::downgrade(&listener);
    *slot.borrow_mut() = Some(ops.watch_file(&path, Duration::from_millis(250), listener));
    std::fs::write(&path, "changed content").unwrap();
    local.block_on(&runtime, async {
        assert_eq!(rx.recv().await.unwrap().to_bits(), 15.0_f64.to_bits());
        assert!(
            rx.recv().await.is_none(),
            "teardown releases callback before another delivery"
        );
        assert!(weak.upgrade().is_none());
    });
    assert!(slot.borrow().is_none());
    drop(ops);
    finish_native(&runtime, local);
}

fn cwd_plain_completion() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    p.get_git_branch();
    let seen = subscribed(&p);
    begin(&e);
    p.set_cwd(e.path("/other"));
    e.answer("stale");
    assert_eq!(p.get_git_branch().as_deref(), Some("other"));
    assert_eq!(*seen.borrow(), [Some("other".into())]);
    p.dispose();
}

#[cfg(any(unix, windows))]
fn finish_native(runtime: &tokio::runtime::Runtime, local: Rc<tokio::task::LocalSet>) {
    let local = Rc::try_unwrap(local).expect("all native effect owners released");
    runtime.block_on(local);
}

#[cfg(unix)]
fn native_poll_overdue_teardown(close: bool) {
    use maestro_watch::fs_watch::FsWatcher;
    let root = native::Scratch::new();
    let path = root.join("table");
    std::fs::write(&path, "initial").unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let ops = NativeFooterOperations::new(local.clone());
    let slot = Rc::new(RefCell::new(None::<Box<dyn FsWatcher>>));
    let captured = slot.clone();
    let count = Rc::new(Cell::new(0));
    let observed = count.clone();
    let (start, entered) = tokio::sync::oneshot::channel();
    let start = RefCell::new(Some(start));
    let (ack, elapsed) = std::sync::mpsc::channel();
    let helper = runtime.spawn(async move {
        tokio::time::sleep_until(entered.await.unwrap()).await;
        ack.send(()).unwrap();
    });
    let changed = path.clone();
    let listener = Rc::new(move |_: FooterFileStats, _: FooterFileStats| {
        observed.set(observed.get() + 1);
        let registration = captured.borrow_mut().take();
        if let Some(mut registration) = registration {
            if close {
                registration.close().unwrap();
            }
            drop(registration);
            std::fs::write(&changed, "changed again after registration ended").unwrap();
            let deadline = tokio::time::Instant::now() + Duration::from_millis(250);
            let sender = start.borrow_mut().take().unwrap();
            sender.send(deadline).unwrap();
            elapsed.recv().unwrap();
        }
    });
    let weak = Rc::downgrade(&listener);
    *slot.borrow_mut() = Some(ops.watch_file(&path, Duration::from_millis(250), listener));
    std::fs::write(&path, "first changed content").unwrap();
    local.block_on(&runtime, helper).unwrap();
    drop(ops);
    finish_native(&runtime, local);
    assert!(slot.borrow().is_none());
    assert!(weak.upgrade().is_none());
    assert_eq!(
        count.get(),
        1,
        "closing callback must stop delivery before the already-due next tick"
    );
}

#[test]
fn footer_linked_worktree_without_commondir_watches_its_own_metadata() {
    let e = Rc::new(Effects::default());
    e.text("/repo/.git", "gitdir: /storage/worktrees/one");
    e.text("/storage/worktrees/one/HEAD", "ref: refs/heads/main");
    e.directory("/storage/worktrees/one/reftable");
    e.text("/storage/worktrees/one/reftable/tables.list", "0");
    let p = provider(&e, "/repo");
    assert_eq!(
        e.active(),
        e.paths([
            "/storage/worktrees/one",
            "/storage/worktrees/one/reftable",
            "/storage/worktrees/one/reftable/tables.list",
        ])
    );
    assert_eq!(e.live_polls(), 1);
    assert_eq!(p.get_git_branch().as_deref(), Some("main"));
    p.dispose();
}

#[test]
fn footer_pending_refresh_after_leaving_repository_keeps_branch_absent() {
    let e = Effects::repo();
    let p = provider(&e, "/repo");
    assert_eq!(p.get_git_branch().as_deref(), Some("main"));
    let seen = subscribed(&p);
    begin(&e);
    e.event(1, None);
    p.set_cwd(e.path("/absent"));
    e.answer("stale");
    assert_eq!(e.pending_timers(), 1);
    e.advance(500);
    assert_eq!(e.pending_timers(), 0);
    assert_eq!(p.get_git_branch(), None);
    assert_eq!(*seen.borrow(), [None]);
    assert_eq!(e.query_dirs.borrow().as_slice(), e.paths(["/repo"]));
    assert!(e.active().is_empty());
    p.dispose();
}

#[cfg(unix)]
#[test]
fn footer_async_lookup_keeps_output_until_pipe_eof() {
    if std::env::var_os("MAESTRO_FOOTER_HOLDER").is_some() {
        footer_pipe_child();
        return;
    }
    let root = native::Scratch::new();
    std::fs::create_dir(root.join("bin")).unwrap();
    let python = std::process::Command::new("sh")
        .args(["-c", "command -v python3"])
        .output()
        .unwrap();
    let python = String::from_utf8(python.stdout).unwrap();
    let script = format!(
        "#!{}\n{}",
        python.trim(),
        r"
import os, socket
os.write(1, b' prefix')
pid = os.getpid()
gone, alive = os.pipe()
if os.fork(): os._exit(0)
os.close(alive); os.read(gone, 1); os.close(gone)
s = socket.create_connection(('127.0.0.1', int(open('holder-port').read())))
s.sendall(str(pid).encode() + b'\n')
assert s.recv(1) == b'!'
os.write(1, b' tail\n')
s.close()
os._exit(0)
"
    );
    std::fs::write(root.join("git.py"), script).unwrap();
    native::install_executable(
        std::path::Path::new(&root.join("git.py")),
        std::path::Path::new(&root.join("bin/git")),
    );
    assert!(
        native::probe(
            maestro_test_process::TestProcess::current(
                "footer_async_lookup_keeps_output_until_pipe_eof"
            )
            .unwrap()
            .case("*"),
            root.0.path(),
            &[("PATH", &root.join("bin")), ("MAESTRO_FOOTER_HOLDER", "1")],
            &[]
        )
        .is_empty()
    );
}

#[cfg(unix)]
/// Drive the isolated native query while its inherited pipe is held.
fn footer_pipe_child() {
    use std::io::{BufRead, BufReader, Read, Write};
    let runtime = crate::support::paused_io_runtime();
    let local = Rc::new(tokio::task::LocalSet::new());
    runtime.block_on(local.run_until(async {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut address = std::fs::File::create_new("holder-port").unwrap();
        write!(address, "{}", listener.local_addr().unwrap().port()).unwrap();
        drop(address);
        let ops = NativeFooterOperations::new(Rc::clone(&local));
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        ops.symbolic_ref_async(
            ".",
            Box::new(move |result| {
                sender.send(result).unwrap();
            }),
        );
        tokio::task::yield_now().await;
        let (mut holder, _) = listener.accept().unwrap();
        let mut pid = String::new();
        BufReader::new(&holder).read_line(&mut pid).unwrap();
        let pid = rustix::process::Pid::from_raw(pid.trim().parse().unwrap()).unwrap();
        footer_reaping_barrier(pid).await;
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert!(
            matches!(
                receiver.try_recv(),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty)
            ),
            "footer still awaits EOF after exit grace"
        );
        holder.write_all(b"!").unwrap();
        assert_eq!(
            receiver.await.unwrap().unwrap().as_deref(),
            Some(" prefix tail\n")
        );
        let mut done = Vec::new();
        holder.read_to_end(&mut done).unwrap();
        assert!(done.is_empty());
    }));
}

#[cfg(unix)]
/// Yield until the process owner has reaped the selected child, without consuming status.
async fn footer_reaping_barrier(pid: rustix::process::Pid) {
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG | WaitIdOptions::EXITED,
        ) {
            Err(rustix::io::Errno::CHILD) => return,
            Ok(_) => tokio::task::yield_now().await,
            Err(error) => panic!("native reaping observation failed: {error}"),
        }
    }
}

/// Embedded bare metadata keeps checkout HEAD and common watch paths distinct.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn footer_embedded_checkout_retains_branch_and_watch_identity() {
    use maestro_workspace::workspace::git::native::discover_git_paths;
    let e = Rc::new(Effects::default());
    e.directory("/container/.bare/objects");
    e.directory("/container/.bare/refs");
    e.text("/container/.bare/config", "[core]\n bare = true\n");
    e.text("/container/.bare/HEAD", "ref: refs/heads/base");
    e.text("/container/.git", "gitdir: .bare");
    e.text(
        "/checkout/.git",
        "gitdir: ../container/.bare/worktrees/topic",
    );
    e.text(
        "/container/.bare/worktrees/topic/HEAD",
        "ref: refs/heads/topic",
    );
    e.text("/container/.bare/worktrees/topic/commondir", "../..");
    e.directory("/container/.bare/reftable");
    e.text("/container/.bare/reftable/tables.list", "0");
    let cwd = e.path("/checkout");
    let shared = discover_git_paths(std::path::Path::new(&cwd)).unwrap();
    let p = provider(&e, "/checkout");
    assert_eq!(shared.repo_root, std::path::Path::new(&cwd));
    assert_eq!(
        shared.git_dir.to_string_lossy(),
        e.path("/container/.bare/worktrees/topic")
    );
    assert_eq!(
        shared.git_common_dir.to_string_lossy(),
        e.path("/container/.bare")
    );
    assert_eq!(p.get_git_branch().as_deref(), Some("topic"));
    assert_eq!(
        e.active(),
        e.paths([
            "/container/.bare/worktrees/topic",
            "/container/.bare/reftable",
            "/container/.bare/reftable/tables.list",
        ])
    );
    assert_eq!(e.live_polls(), 1);
    e.text(
        "/container/.bare/worktrees/topic/HEAD",
        "ref: refs/heads/updated",
    );
    e.event(0, Some("HEAD"));
    e.advance(500);
    assert_eq!(p.get_git_branch().as_deref(), Some("updated"));
    p.dispose();
    assert!(e.active().is_empty());
    assert_eq!(e.live_polls(), 0);
}

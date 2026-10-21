//! Footer metadata: branch discovery, cached lookup, statuses, count and
//! branch-change subscriptions.
#![cfg(test)]

use crate::footer_support as support;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
#[cfg(unix)]
use std::path::Path;
use std::rc::Rc;

use maestro_app::ReadonlyFooterDataProvider;
use maestro_app::presentation_data::footer_data_provider::{ExtensionStatuses, FooterDataProvider};
#[cfg(unix)]
use maestro_app::presentation_data::footer_data_provider::{
    FooterOperations, NativeFooterOperations,
};
use serde::Deserialize;
use support::{Entry, Git, MetadataEffects, load, plain, provider, target};
#[cfg(unix)]
use support::{Scratch, git, install_executable, probe};

/// An optional branch as a fixture spells it.
type Branch = Option<String>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HeadCase {
    #[serde(rename = "test")]
    _test: String,
    input: HeadInput,
    expected: HeadExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HeadInput {
    head: Option<String>,
    head_bytes: Option<Vec<u8>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HeadExpected {
    branch: String,
    git_calls: Option<usize>,
}

#[test]
fn footer_head_records_keep_exact_branch_text() {
    for (index, case) in load::<HeadCase>("footer_head_records_keep_exact_branch_text")
        .into_iter()
        .enumerate()
    {
        let branch = match (case.input.head, case.input.head_bytes) {
            (Some(head), None) => {
                let ops = MetadataEffects::new(plain(Entry::Text(head)));
                let branch = provider(&ops, "/repo").get_git_branch();
                assert_eq!(
                    ops.git_dirs.borrow().len(),
                    case.expected.git_calls.expect("fake case counts Git calls"),
                    "case {index}"
                );
                branch
            }
            #[cfg(unix)]
            (None, Some(bytes)) => {
                let scratch = Scratch::new();
                std::fs::create_dir(scratch.join(".git")).expect("metadata directory");
                std::fs::write(scratch.join(".git/HEAD"), bytes).expect("HEAD written");
                let native: Rc<dyn FooterOperations> = Rc::new(NativeFooterOperations::new(
                    Rc::new(tokio::task::LocalSet::new()),
                ));
                FooterDataProvider::new(scratch.join(""), native).get_git_branch()
            }
            #[cfg(not(unix))]
            (None, Some(_)) => continue,
            _ => panic!("case {index} names one HEAD form"),
        };
        assert_eq!(branch, Some(case.expected.branch), "case {index}");
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncOutcome {
    status: Option<i32>,
    stdout: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncInput {
    sync: SyncOutcome,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandOptions {
    cwd: String,
}

#[derive(Deserialize)]
struct Command(String, String, Vec<String>, CommandOptions);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncExpected {
    branch: String,
    commands: Vec<Command>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncCase {
    #[serde(rename = "test")]
    _test: String,
    input: SyncInput,
    expected: SyncExpected,
}

const SYMBOLIC_ARGS: [&str; 5] = [
    "--no-optional-locks",
    "symbolic-ref",
    "--quiet",
    "--short",
    "HEAD",
];

fn reftable_files() -> BTreeMap<String, Entry> {
    let mut files = plain(Entry::Text("ref: refs/heads/.invalid\n".to_owned()));
    files.insert("/repo/.git/reftable".to_owned(), Entry::Flag(true));
    files.insert(
        "/repo/.git/reftable/tables.list".to_owned(),
        Entry::Text("0\n".to_owned()),
    );
    files
}

/// The directories the recorded Git commands ran in, after checking each one
/// is the synchronous `git` call with the arguments the native probe observes.
fn command_dirs(ops: &MetadataEffects, commands: &[Command], args: &[String]) -> Vec<String> {
    commands
        .iter()
        .map(|command| {
            assert_eq!((command.0.as_str(), command.1.as_str()), ("sync", "git"));
            assert_eq!(command.2, args);
            ops.path(&command.3.cwd)
        })
        .collect()
}

#[test]
fn footer_symbolic_lookup_uses_exact_command_and_fallback() {
    #[cfg(unix)]
    if let Ok(repo) = std::env::var("FOOTER_PROBE_REPO") {
        let native: Rc<dyn FooterOperations> = Rc::new(NativeFooterOperations::new(Rc::new(
            tokio::task::LocalSet::new(),
        )));
        let branch = FooterDataProvider::new(repo, native).get_git_branch();
        println!("\nPROBE branch={}", branch.unwrap_or_default());
        return;
    }
    let cases = load::<SyncCase>("footer_symbolic_lookup_uses_exact_command_and_fallback");
    let args = SYMBOLIC_ARGS.map(str::to_owned);
    for (index, case) in cases.iter().enumerate() {
        let ops = MetadataEffects::new(reftable_files());
        let SyncOutcome { status, stdout } = &case.input.sync;
        *ops.git.borrow_mut() = Git::Output((*status == Some(0)).then(|| stdout.clone()));
        assert_eq!(
            provider(&ops, "/repo").get_git_branch(),
            Some(case.expected.branch.clone()),
            "case {index}"
        );
        assert_eq!(
            *ops.git_dirs.borrow(),
            command_dirs(&ops, &case.expected.commands, &args),
            "case {index}"
        );
    }
    let ops = MetadataEffects::new(reftable_files());
    *ops.git.borrow_mut() = Git::SpawnError;
    assert_eq!(
        provider(&ops, "/repo").get_git_branch().as_deref(),
        Some("detached")
    );
    #[cfg(unix)]
    native_git_observations(&args);
}

#[cfg(unix)]
/// Run the native adapter against a stand-in `git` in a child with its own PATH.
fn native_git_observations(args: &[String]) {
    let scratch = Scratch::new();
    for dir in ["bin", "repo", "repo/.git", "empty"] {
        std::fs::create_dir(scratch.join(dir)).expect("directory");
    }
    std::fs::write(scratch.join("repo/.git/HEAD"), "ref: refs/heads/.invalid\n").expect("HEAD");
    let script = "#!/bin/sh\n{ printf '%s\\n' \"$*\"; pwd -P; readlink /proc/self/fd/0; readlink /proc/self/fd/2; } > \"$FOOTER_LOG\"\necho noise >&2\necho feature/native\nexit \"$FOOTER_EXIT\"\n";
    std::fs::write(scratch.join("git.sh"), script).expect("script");
    install_executable(
        Path::new(&scratch.join("git.sh")),
        Path::new(&scratch.join("bin/git")),
    );
    let log = scratch.join("log");
    let path = format!("{}:/usr/bin:/bin", scratch.join("bin"));
    let repo = scratch.join("repo");
    let run = |path: &str, exit: &str| {
        probe(
            maestro_test_process::TestProcess::current(
                "footer_symbolic_lookup_uses_exact_command_and_fallback",
            )
            .unwrap()
            .case("*"),
            Path::new(scratch.0.path()),
            &[
                ("FOOTER_PROBE_REPO", &repo),
                ("PATH", path),
                ("FOOTER_LOG", &log),
                ("FOOTER_EXIT", exit),
            ],
            &[],
        )
    };
    assert_eq!(run(&path, "0"), ["branch=feature/native"]);
    let logged = std::fs::read_to_string(&log).expect("log");
    let lines: Vec<_> = logged.lines().collect();
    let repo_real = std::fs::canonicalize(&repo).expect("repo path");
    assert_eq!(lines[0], args.join(" "));
    assert_eq!(Path::new(lines[1]), repo_real);
    if cfg!(target_os = "linux") {
        assert_eq!(&lines[2..], ["/dev/null", "/dev/null"]);
    }
    assert_eq!(run(&path, "1"), ["branch=detached"]);
    assert_eq!(run(&scratch.join("empty"), "0"), ["branch=detached"]);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DiscoveryInput {
    name: String,
    current: Option<String>,
    files: BTreeMap<String, Entry>,
    cwd: Option<String>,
    fail_stat: Option<Vec<String>>,
    fail_read: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BranchExpected {
    branch: Branch,
    current_dir_calls: Option<usize>,
    unread: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoveryCase {
    #[serde(rename = "test")]
    _test: String,
    input: DiscoveryInput,
    expected: BranchExpected,
}

/// Observe native absolute and relative links with a child-local ambient directory.
#[cfg(unix)]
fn check_discovery_ambient(
    ops: &MetadataEffects,
    cwd: &str,
    calls: usize,
    has_current: bool,
    expected: &Branch,
) {
    let current = if calls == 0 || !has_current {
        "/gone"
    } else {
        "/abs"
    };
    ops.repository.directory(current);
    let cwd = if calls == 0 {
        ops.path(cwd)
    } else if !has_current {
        "../repo".to_owned()
    } else {
        cwd.to_owned()
    };
    let mut env = vec![("FOOTER_DISCOVERY_CWD", cwd.as_str())];
    if current == "/gone" {
        env.push(("FOOTER_REMOVE_CWD", "1"));
    }
    let observed = probe(
        maestro_test_process::TestProcess::current(
            "footer_discovers_metadata_without_skipping_broken_inner_repo",
        )
        .unwrap()
        .name(),
        Path::new(&ops.path(current)),
        &env,
        &[],
    );
    assert_eq!(
        observed,
        [format!(
            "branch={}",
            expected.as_deref().unwrap_or_default()
        )],
        "{cwd}"
    );
}

#[test]
fn footer_discovers_metadata_without_skipping_broken_inner_repo() {
    #[cfg(unix)]
    if let Ok(cwd) = std::env::var("FOOTER_DISCOVERY_CWD") {
        if std::env::var_os("FOOTER_REMOVE_CWD").is_some() {
            std::fs::remove_dir(std::env::current_dir().unwrap()).unwrap();
            assert!(std::env::current_dir().is_err());
        }
        println!("\nPROBE branch={}", native_branch(&cwd).unwrap_or_default());
        return;
    }
    let cases =
        load::<DiscoveryCase>("footer_discovers_metadata_without_skipping_broken_inner_repo");
    for case in cases {
        let mut files = case.input.files;
        if case.input.current.is_some() {
            let marker = files.remove("repo/.git").unwrap();
            files.insert("/abs/repo/.git".to_owned(), marker);
        }
        let ops = MetadataEffects::new(files);
        ops.fail(
            &case.input.fail_stat.unwrap_or_default(),
            &case.input.fail_read.unwrap_or_default(),
        );
        let cwd = case.input.cwd.as_deref().unwrap_or("/repo");
        let name = &case.input.name;
        if let Some(calls) = case.expected.current_dir_calls {
            #[cfg(not(unix))]
            let _ = calls;
            #[cfg(unix)]
            check_discovery_ambient(
                &ops,
                cwd,
                calls,
                case.input.current.is_some(),
                &case.expected.branch,
            );
            continue;
        }
        if case.expected.unread.is_some() {
            ops.set("/.git/HEAD", text("ref: refs/heads/outer"));
        }
        let p = provider(&ops, cwd);
        assert_eq!(p.get_git_branch(), case.expected.branch, "{name}");
        for path in case.expected.unread.unwrap_or_default() {
            // Missing HEAD must prevent publication even if optional metadata is repaired.
            ops.repository.remove(&path);
            ops.set(&path, text("../.."));
            let repaired = provider(&ops, cwd);
            assert_eq!(repaired.get_git_branch(), None, "{name}: {path}");
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorktreeCase {
    #[serde(rename = "test")]
    _test: String,
    input: WorktreeInput,
    expected: SyncExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorktreeInput {
    files: BTreeMap<String, Entry>,
    cwd: String,
    sync: SyncOutcome,
}

#[test]
fn footer_reftable_worktree_resolves_using_worktree_cwd() {
    let args = SYMBOLIC_ARGS.map(str::to_owned);
    for case in load::<WorktreeCase>("footer_reftable_worktree_resolves_using_worktree_cwd") {
        let ops = MetadataEffects::new(case.input.files);
        *ops.git.borrow_mut() =
            Git::Output((case.input.sync.status == Some(0)).then_some(case.input.sync.stdout));
        let branch = provider(&ops, &case.input.cwd).get_git_branch();
        assert_eq!(branch, Some(case.expected.branch));
        assert_eq!(
            *ops.git_dirs.borrow(),
            command_dirs(&ops, &case.expected.commands, &args)
        );
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CachedCase {
    Plain {
        #[serde(rename = "test")]
        _test: String,
        #[serde(rename = "input")]
        _input: Empty,
        expected: CachedExpected,
    },
    Edited {
        #[serde(rename = "test")]
        _test: String,
        input: Scenario,
        expected: EditedExpected,
    },
    Head {
        #[serde(rename = "test")]
        _test: String,
        input: HeadScenario,
        expected: CachedExpected,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    scenario: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeadScenario {
    scenario: String,
    head: Entry,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CachedExpected {
    first: Branch,
    second: Branch,
    #[serde(rename = "headReads")]
    _head_reads: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EditedExpected {
    #[serde(rename = "readsBefore")]
    _reads_before: usize,
    branch: Branch,
}

const HEAD: &str = "/repo/.git/HEAD";

fn text(value: &str) -> Entry {
    Entry::Text(value.to_owned())
}

#[test]
fn footer_initial_lookup_is_lazy_and_cached() {
    for case in load::<CachedCase>("footer_initial_lookup_is_lazy_and_cached") {
        match case {
            CachedCase::Plain { expected, .. } => {
                let ops = MetadataEffects::new(plain(text("ref: refs/heads/main")));
                check_cached(&ops, &expected, text("ref: refs/heads/new"));
            }
            CachedCase::Head {
                input, expected, ..
            } => {
                assert_eq!(input.scenario, "cached-head-result");
                let ops = MetadataEffects::new(plain(input.head));
                check_cached(&ops, &expected, text("ref: refs/heads/repaired"));
            }
            CachedCase::Edited {
                input, expected, ..
            } => {
                let files = match input.scenario.as_str() {
                    "head-changed-before-first-read" => plain(text("ref: refs/heads/old")),
                    "repo-created-after-construction" => BTreeMap::new(),
                    other => panic!("unknown scenario {other}"),
                };
                let ops = MetadataEffects::new(files);
                let provider = provider(&ops, "/repo");
                // Editing before the first getter distinguishes lazy from eager HEAD reads.
                ops.set("/repo/.git", Entry::Flag(true));
                ops.set(HEAD, text("ref: refs/heads/new"));
                assert_eq!(provider.get_git_branch(), expected.branch);
            }
        }
    }
}

/// Look up twice around an edit of HEAD and check the first result is kept.
fn check_cached(ops: &Rc<MetadataEffects>, expected: &CachedExpected, edit: Entry) {
    let provider = provider(ops, "/repo");
    let first = provider.get_git_branch();
    ops.set(HEAD, edit);
    assert_eq!(first, expected.first.clone());
    assert_eq!(provider.get_git_branch(), expected.second.clone());
    ops.repository.remove(HEAD);
    assert_eq!(provider.get_git_branch(), expected.second.clone());
    ops.set(HEAD, text("ref: refs/heads/after-invalidation"));
    provider.set_cwd(ops.path("/repo/child"));
    assert_eq!(
        provider.get_git_branch().as_deref(),
        Some("after-invalidation")
    );
}

type Pair = (String, String);

#[derive(Deserialize)]
#[serde(untagged)]
enum StatusCase {
    Script {
        #[serde(rename = "test")]
        _test: String,
        input: StatusScript,
        expected: ScriptExpected,
    },
    Iterator {
        #[serde(rename = "test")]
        _test: String,
        input: Scenario,
        expected: IteratorExpected,
    },
    Accessors {
        #[serde(rename = "test")]
        _test: String,
        input: Scenario,
        expected: AccessorsExpected,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusScript {
    operations: Vec<(String, Option<String>)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ScriptExpected {
    observations: Vec<Vec<Pair>>,
    first: Pair,
    remaining: Vec<Pair>,
    cleared: Vec<Pair>,
    after_dispose: Vec<Pair>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct IteratorExpected {
    first: Pair,
    replaced: Pair,
    reinserted: Pair,
    exhausted: bool,
    still_exhausted: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Emptiness {
    size: usize,
    is_empty: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lookup {
    key: String,
    has: bool,
    value: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AccessorsExpected {
    empty: Emptiness,
    size: usize,
    is_empty: bool,
    lookups: Vec<Lookup>,
}

#[test]
fn footer_status_view_is_live_and_insertion_ordered() {
    for case in load::<StatusCase>("footer_status_view_is_live_and_insertion_ordered") {
        let provider = provider(&MetadataEffects::new(BTreeMap::new()), "/repo");
        let view = provider.get_extension_statuses();
        match case {
            StatusCase::Script {
                input, expected, ..
            } => check_script(&provider, &view, &input, expected),
            StatusCase::Iterator {
                input, expected, ..
            } => {
                assert_eq!(input.scenario, "live-iterator-clear-update-exhaustion");
                check_iterator(&provider, &view, expected);
            }
            StatusCase::Accessors {
                input, expected, ..
            } => {
                assert_eq!(input.scenario, "view-accessors");
                check_accessors(&provider, &view, expected);
            }
        }
    }
}

fn check_script(
    provider: &FooterDataProvider,
    view: &ExtensionStatuses,
    input: &StatusScript,
    expected: ScriptExpected,
) {
    let mut observations = Vec::new();
    for (key, status) in &input.operations {
        provider.set_extension_status(key, status.as_deref());
        observations.push(view.iter().collect::<Vec<_>>());
    }
    assert_eq!(observations, expected.observations);
    let mut walk = view.iter();
    assert_eq!(walk.next(), Some(expected.first));
    provider.set_extension_status("empty", None);
    provider.set_extension_status("late", Some("fourth"));
    assert_eq!(walk.collect::<Vec<_>>(), expected.remaining);
    provider.clear_extension_statuses();
    assert_eq!(view.iter().collect::<Vec<_>>(), expected.cleared);
    provider.set_extension_status("final", Some("kept"));
    provider.dispose();
    assert_eq!(view.iter().collect::<Vec<_>>(), expected.after_dispose);
}

fn check_iterator(
    provider: &FooterDataProvider,
    view: &ExtensionStatuses,
    expected: IteratorExpected,
) {
    provider.set_extension_status("a", Some("old"));
    provider.set_extension_status("b", Some("old"));
    let mut walk = view.iter();
    assert_eq!(walk.next(), Some(expected.first));
    provider.set_extension_status("b", Some("new"));
    assert_eq!(walk.next(), Some(expected.replaced));
    provider.clear_extension_statuses();
    provider.set_extension_status("a", Some("reinserted"));
    assert_eq!(walk.next(), Some(expected.reinserted));
    assert_eq!(walk.next().is_none(), expected.exhausted);
    provider.set_extension_status("late", Some("not-visited"));
    assert_eq!(walk.next().is_none(), expected.still_exhausted);
}

fn check_accessors(
    provider: &FooterDataProvider,
    view: &ExtensionStatuses,
    expected: AccessorsExpected,
) {
    assert_eq!(
        (view.len(), view.is_empty()),
        (expected.empty.size, expected.empty.is_empty)
    );
    provider.set_extension_status("x", Some("before"));
    provider.set_extension_status("", Some(""));
    provider.set_extension_status("x", Some("after"));
    assert_eq!(
        (view.len(), view.is_empty()),
        (expected.size, expected.is_empty)
    );
    for lookup in expected.lookups {
        assert_eq!(view.contains_key(&lookup.key), lookup.has);
        assert_eq!(view.get(&lookup.key), lookup.value);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CountCase {
    #[serde(rename = "test")]
    _test: String,
    input: CountInput,
    expected: CountExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CountInput {
    counts: Vec<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CountExpected {
    counts: Vec<f64>,
    after_dispose: f64,
}

#[test]
fn footer_provider_count_retains_supplied_value() {
    for case in load::<CountCase>("footer_provider_count_retains_supplied_value") {
        let provider = provider(&MetadataEffects::new(BTreeMap::new()), "/repo");
        let notified = Rc::new(RefCell::new(0));
        let counter = Rc::clone(&notified);
        let _keep = provider.on_branch_change(Rc::new(move || *counter.borrow_mut() += 1));
        let mut counts = vec![provider.get_available_provider_count()];
        for count in case.input.counts {
            provider.set_available_provider_count(count);
            counts.push(provider.get_available_provider_count());
        }
        provider.dispose();
        assert_eq!(counts, case.expected.counts);
        assert_eq!(
            provider.get_available_provider_count().to_bits(),
            case.expected.after_dispose.to_bits()
        );
        assert_eq!(*notified.borrow(), 0);
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CwdCase {
    Spelling {
        #[serde(rename = "test")]
        _test: String,
        #[serde(rename = "input")]
        _input: Empty,
        expected: SpellingExpected,
    },
    Nested {
        #[serde(rename = "test")]
        _test: String,
        input: Scenario,
        expected: NestedExpected,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SpellingExpected {
    same: usize,
    spelling_change: usize,
    branch: Branch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NestedExpected {
    seen: Vec<(String, Branch)>,
    statuses: Vec<Pair>,
    count: f64,
}

type Reads = Rc<RefCell<Vec<(String, Branch)>>>;

/// A callback that records the branch it reads and, when `moves` is set, moves
/// the working directory once from inside the notification.
fn reading_callback(
    provider: &FooterDataProvider,
    seen: &Reads,
    name: &str,
    moves: bool,
) -> Rc<dyn Fn()> {
    let (reader, log, name) = (provider.clone(), Rc::clone(seen), name.to_owned());
    let pending = Cell::new(moves);
    Rc::new(move || {
        log.borrow_mut()
            .push((name.clone(), reader.get_git_branch()));
        if pending.replace(false) {
            reader.set_cwd(target("/missing"));
        }
    })
}

#[test]
fn footer_cwd_change_resets_before_notifying() {
    for case in load::<CwdCase>("footer_cwd_change_resets_before_notifying") {
        let ops = MetadataEffects::new(plain(text("ref: refs/heads/main")));
        let provider = provider(&ops, "/repo");
        match case {
            CwdCase::Spelling { expected, .. } => {
                let calls = Rc::new(RefCell::new(0));
                let counter = Rc::clone(&calls);
                let _keep = provider.on_branch_change(Rc::new(move || *counter.borrow_mut() += 1));
                provider.set_cwd(ops.path("/repo"));
                assert_eq!(*calls.borrow(), expected.same);
                provider.set_cwd(ops.path("/repo/child"));
                assert_eq!(*calls.borrow(), expected.spelling_change);
                provider.dispose();
                assert_eq!(provider.get_git_branch(), expected.branch);
            }
            CwdCase::Nested {
                input, expected, ..
            } => {
                assert_eq!(input.scenario, "nested-publication");
                let seen = Rc::new(RefCell::new(Vec::new()));
                provider.set_extension_status("key", Some("retained"));
                provider.set_available_provider_count(4.0);
                drop(provider.on_branch_change(reading_callback(&provider, &seen, "a", true)));
                drop(provider.on_branch_change(reading_callback(&provider, &seen, "b", false)));
                provider.set_cwd(ops.path("/repo/child"));
                assert_eq!(*seen.borrow(), expected.seen);
                assert_eq!(
                    provider.get_extension_statuses().iter().collect::<Vec<_>>(),
                    expected.statuses
                );
                assert_eq!(
                    provider.get_available_provider_count().to_bits(),
                    expected.count.to_bits()
                );
                provider.dispose();
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenCase {
    #[serde(rename = "test")]
    _test: String,
    #[serde(rename = "input")]
    _input: Empty,
    expected: SeenExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenExpected {
    seen: Vec<String>,
}

type Log = Rc<RefCell<Vec<String>>>;
type Callback = Rc<dyn Fn()>;

fn logger(log: &Log, name: &str) -> Rc<dyn Fn()> {
    let (log, name) = (Rc::clone(log), name.to_owned());
    Rc::new(move || log.borrow_mut().push(name.clone()))
}

fn expected_seen(test: &str) -> Vec<String> {
    let mut cases = load::<SeenCase>(test);
    assert_eq!(cases.len(), 1);
    cases.remove(0).expected.seen
}

#[test]
fn footer_callbacks_follow_live_membership() {
    let provider = provider(&MetadataEffects::new(BTreeMap::new()), "/repo");
    let seen = Log::default();
    let remove_b = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
    let (adder, log, remover) = (provider.clone(), Rc::clone(&seen), Rc::clone(&remove_b));
    let c = logger(&seen, "c");
    let a: Rc<dyn Fn()> = Rc::new(move || {
        log.borrow_mut().push("a".to_owned());
        if let Some(remove) = remover.borrow().as_ref() {
            remove();
        }
        drop(adder.on_branch_change(Rc::clone(&c)));
    });
    let remove_a = provider.on_branch_change(Rc::clone(&a));
    drop(provider.on_branch_change(Rc::clone(&a)));
    *remove_b.borrow_mut() = Some(provider.on_branch_change(logger(&seen, "b")));
    provider.set_cwd(target("/changed"));
    remove_a();
    remove_a();
    provider.set_cwd(target("/again"));
    assert_eq!(
        *seen.borrow(),
        expected_seen("footer_callbacks_follow_live_membership")
    );
    provider.dispose();
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UnsubscribeCase {
    Identity {
        #[serde(rename = "test")]
        _test: String,
        #[serde(rename = "input")]
        _input: Empty,
        expected: SeenExpected,
    },
    Dropped {
        #[serde(rename = "test")]
        _test: String,
        input: Scenario,
        expected: CallsExpected,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallsExpected {
    calls: usize,
}

#[test]
fn footer_unsubscribe_tracks_callback_identity() {
    for case in load::<UnsubscribeCase>("footer_unsubscribe_tracks_callback_identity") {
        let provider = provider(&MetadataEffects::new(BTreeMap::new()), "/repo");
        let seen = Log::default();
        let callback = logger(&seen, "called");
        match case {
            UnsubscribeCase::Identity { expected, .. } => {
                let old = provider.on_branch_change(Rc::clone(&callback));
                drop(provider.on_branch_change(Rc::clone(&callback)));
                provider.set_cwd(target("/first"));
                old();
                drop(provider.on_branch_change(Rc::clone(&callback)));
                old();
                provider.set_cwd(target("/second"));
                assert_eq!(*seen.borrow(), expected.seen);
            }
            UnsubscribeCase::Dropped {
                input, expected, ..
            } => {
                assert_eq!(input.scenario, "dropping-handle-keeps-subscription");
                drop(provider.on_branch_change(callback));
                provider.set_cwd(target("/changed"));
                assert_eq!(seen.borrow().len(), expected.calls);
            }
        }
    }
}

#[test]
fn footer_callbacks_revisit_reinserted_members() {
    let provider = provider(&MetadataEffects::new(BTreeMap::new()), "/repo");
    let seen = Log::default();
    let remove = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
    let slot = Rc::clone(&remove);
    let me: Rc<RefCell<Option<Callback>>> = Rc::default();
    let (this, log, reader) = (Rc::clone(&me), Rc::clone(&seen), provider.clone());
    let moved = Rc::new(RefCell::new(false));
    let a: Rc<dyn Fn()> = Rc::new(move || {
        log.borrow_mut().push("a".to_owned());
        if !std::mem::replace(&mut *moved.borrow_mut(), true) {
            if let Some(remove) = slot.borrow().as_ref() {
                remove();
            }
            if let Some(callback) = this.borrow().as_ref() {
                drop(reader.on_branch_change(Rc::clone(callback)));
            }
        }
    });
    *me.borrow_mut() = Some(Rc::clone(&a));
    let weak = Rc::downgrade(&a);
    *remove.borrow_mut() = Some(provider.on_branch_change(a));
    drop(provider.on_branch_change(logger(&seen, "b")));
    provider.set_cwd(target("/next"));
    assert_eq!(
        *seen.borrow(),
        expected_seen("footer_callbacks_revisit_reinserted_members")
    );
    provider.dispose();
    *me.borrow_mut() = None;
    *remove.borrow_mut() = None;
    assert!(weak.upgrade().is_none());
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AliasesCase {
    #[serde(rename = "test")]
    _test: String,
    #[serde(rename = "input")]
    _input: Empty,
    expected: AliasesExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AliasesExpected {
    branch: Branch,
    statuses: Vec<Pair>,
    count: f64,
    seen: Vec<Branch>,
}

/// Read the metadata only through the read-only interface.
fn read_only(provider: &dyn ReadonlyFooterDataProvider) -> (Branch, Vec<Pair>, f64) {
    (
        provider.get_git_branch(),
        provider.get_extension_statuses().iter().collect(),
        provider.get_available_provider_count(),
    )
}

#[test]
fn footer_readonly_aliases_keep_live_metadata() {
    for case in load::<AliasesCase>("footer_readonly_aliases_keep_live_metadata") {
        let ops = MetadataEffects::new(plain(text("ref: refs/heads/main")));
        let owner = provider(&ops, "/repo");
        let reader = owner.clone();
        let statuses = reader.get_extension_statuses();
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (log, watcher) = (Rc::clone(&seen), reader.clone());
        let witness = Rc::new(());
        let held = Rc::clone(&witness);
        let unsubscribe = reader.on_branch_change(Rc::new(move || {
            let _ = &held;
            log.borrow_mut().push(watcher.get_git_branch());
        }));
        owner.set_extension_status("z", Some("ready"));
        owner.set_available_provider_count(3.0);
        owner.set_cwd(target("/absent"));
        owner.dispose();
        assert_eq!(
            read_only(&reader),
            (
                case.expected.branch,
                case.expected.statuses.clone(),
                case.expected.count
            )
        );
        assert_eq!(*seen.borrow(), case.expected.seen);
        assert_eq!(statuses.iter().collect::<Vec<_>>(), case.expected.statuses);
        drop((owner, reader));
        assert_eq!(statuses.get("z").as_deref(), Some("ready"));
        unsubscribe();
        assert_eq!(
            Rc::strong_count(&witness),
            2,
            "the unsubscribe handle keeps its callback"
        );
        drop(unsubscribe);
        assert_eq!(
            Rc::strong_count(&witness),
            1,
            "the last handle releases the callback"
        );
    }
}

#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCase {
    #[serde(rename = "test")]
    _test: String,
    input: Scenario,
    expected: NativeExpected,
}

#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct NativeExpected {
    branch: String,
    git_output: Option<String>,
}

#[cfg(unix)]
fn native_branch(cwd: &str) -> Branch {
    let native: Rc<dyn FooterOperations> = Rc::new(NativeFooterOperations::new(Rc::new(
        tokio::task::LocalSet::new(),
    )));
    FooterDataProvider::new(cwd.to_owned(), native).get_git_branch()
}

#[cfg(unix)]
#[test]
fn footer_native_and_controlled_adapters_agree() {
    let scratch = Scratch::new();
    let (repo, linked) = (scratch.join("repo"), scratch.join("linked"));
    git(
        &scratch.join(""),
        &["init", "-q", "--initial-branch=main", &repo],
    );
    let nested = scratch.join("repo/a/b");
    std::fs::create_dir_all(&nested).expect("nested directory");
    let scenarios: BTreeMap<_, _> =
        load::<NativeCase>("footer_native_and_controlled_adapters_agree")
            .into_iter()
            .map(|case| (case.input.scenario, case.expected))
            .collect();
    assert_eq!(scenarios.len(), 4);
    let nested_case = &scenarios["real nested unborn repository"];
    assert_eq!(native_branch(&nested), Some(nested_case.branch.clone()));
    let output = NativeFooterOperations::new(Rc::new(tokio::task::LocalSet::new()))
        .symbolic_ref_sync(&repo)
        .expect("git runs");
    assert_eq!(output, nested_case.git_output);
    let controlled = MetadataEffects::new(plain(text("ref: refs/heads/main\n")));
    assert_eq!(
        provider(&controlled, "/repo/a/b").get_git_branch(),
        Some(nested_case.branch.clone())
    );
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "fixture"]);
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "linked-topic", &linked],
    );
    assert_eq!(
        native_branch(&linked),
        Some(scenarios["real linked worktree"].branch.clone())
    );
    git(&linked, &["checkout", "-q", "--detach"]);
    assert_eq!(
        native_branch(&linked),
        Some(scenarios["real detached worktree"].branch.clone())
    );
    let link = scratch.join("symlinked");
    std::fs::create_dir(&link).expect("link directory");
    std::os::unix::fs::symlink(scratch.join("repo/.git"), scratch.join("symlinked/.git"))
        .expect("symlink");
    git(&repo, &["symbolic-ref", "HEAD", "refs/heads/updated"]);
    assert_eq!(
        native_branch(&link),
        Some(scenarios["symlinked git metadata"].branch.clone())
    );
}

#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbientCase {
    #[serde(rename = "test")]
    _test: String,
    input: AmbientInput,
    expected: BranchExpected,
}

#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AmbientInput {
    name: String,
    cwd: String,
}

#[cfg(unix)]
const AMBIENT: &str = "footer_paths_read_ambient_cwd_only_when_required";

#[cfg(unix)]
/// Child side: remove the working directory, then print what the native
/// adapter finds for each case.
fn ambient_child(root: &str) {
    std::fs::remove_dir(std::env::current_dir().expect("start directory")).expect("removed");
    assert!(std::env::current_dir().is_err());
    for case in load::<AmbientCase>(AMBIENT) {
        let cwd = if case.input.cwd == "ABSOLUTE_WORKTREE" {
            format!("{root}/worktree")
        } else {
            case.input.cwd
        };
        println!(
            "\nPROBE {}={}",
            case.input.name,
            native_branch(&cwd).unwrap_or_default()
        );
    }
}

/// Drive-relative links are resolved by native drive observations, not a fake file service.
#[cfg(windows)]
#[test]
fn worktree_resolution_keeps_eager_drive_observations() {
    if let Ok(worktree) = std::env::var("FOOTER_DRIVE_WORKTREE") {
        let ops = MetadataEffects::new(BTreeMap::new());
        for cwd in [worktree, ".".to_owned()] {
            let p = FooterDataProvider::new(cwd, ops.clone());
            assert_eq!(p.get_git_branch().as_deref(), Some("updated"));
        }
        return;
    }
    let scratch = support::Repository::default();
    scratch.directory("worktree");
    scratch.text("repo/.git/HEAD", "ref: refs/heads/updated");
    let repo = scratch.path("repo/.git");
    let drive = repo[..2].to_lowercase();
    scratch.text("worktree/.git", &format!("gitdir: {drive}../repo/.git"));
    let output = maestro_test_process::TestProcess::current(
        "worktree_resolution_keeps_eager_drive_observations",
    )
    .unwrap()
    .current_dir(scratch.path("worktree"))
    .env("FOOTER_DRIVE_WORKTREE", scratch.path("worktree"))
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// What the native adapter finds for each ambient case, read in a child whose
/// working directory is gone.
#[cfg(unix)]
fn native_ambient_lines() -> Vec<String> {
    let scratch = Scratch::new();
    for dir in ["repo", "repo/.git", "worktree", "gone"] {
        std::fs::create_dir(scratch.join(dir)).expect("directory");
    }
    std::fs::write(scratch.join("repo/.git/HEAD"), "ref: refs/heads/updated\n").expect("HEAD");
    std::fs::write(scratch.join("worktree/.git"), "gitdir: ../repo/.git\n").expect("gitfile");
    let root = scratch.join("");
    probe(
        maestro_test_process::TestProcess::current(AMBIENT)
            .unwrap()
            .case("*"),
        Path::new(&scratch.join("gone")),
        &[("FOOTER_PROBE_AMBIENT", &root)],
        &[],
    )
}

#[test]
fn footer_paths_read_ambient_cwd_only_when_required() {
    #[cfg(unix)]
    if let Ok(root) = std::env::var("FOOTER_PROBE_AMBIENT") {
        return ambient_child(&root);
    }
    #[cfg(unix)]
    let observed = native_ambient_lines();
    #[cfg(unix)]
    for case in load::<AmbientCase>(AMBIENT) {
        let name = &case.input.name;
        let native = observed
            .iter()
            .find_map(|line| line.strip_prefix(&format!("{name}=")));
        let native = native.map(|found| (!found.is_empty()).then(|| found.to_owned()));
        assert_eq!(native, Some(case.expected.branch), "{name}");
    }
    let ops = MetadataEffects::new(BTreeMap::from([
        (
            "/repo/.git/HEAD".to_owned(),
            text("ref: refs/heads/updated"),
        ),
        ("/worktree/.git".to_owned(), text("gitdir: ../repo/.git")),
    ]));
    assert_eq!(
        provider(&ops, "/worktree").get_git_branch().as_deref(),
        Some("updated")
    );
}

/// Bare metadata is shared by the daemon and footer, including the checkout root.
#[cfg(unix)]
#[test]
fn footer_bare_checkout_uses_shared_repository_discovery() {
    let scratch = Scratch::new();
    git(
        &scratch.join(""),
        &[
            "init",
            "--bare",
            "--initial-branch=bare-topic",
            &scratch.join("bare"),
        ],
    );
    let bare = scratch.join("bare");
    let discovered =
        maestro_workspace::workspace::git::native::discover_git_paths(Path::new(&bare)).unwrap();
    assert_eq!(discovered.repo_root, Path::new(&bare));
    assert_eq!(
        std::fs::read_to_string(discovered.git_dir.join("HEAD"))
            .unwrap()
            .trim()
            .strip_prefix("ref: refs/heads/"),
        Some("bare-topic")
    );
    assert_eq!(native_branch(&bare).as_deref(), Some("bare-topic"));
}

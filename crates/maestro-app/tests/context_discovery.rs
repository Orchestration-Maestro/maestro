//! Context files from the agent directory and from the filesystem root down to the
//! working directory, and the warning for one that cannot be read.
#![cfg(unix)]
use crate::resource_fixtures::{
    Entry, Root, child_stderr, directory_read_error, loader_case, on_task_set, run_loader_case,
    warnings,
};
use maestro_app::load_project_context_files;
use serde::Deserialize;
use serde_json::{Value, json};

/// One recorded context case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextCase {
    name: String,
    tree: Vec<Entry>,
    cwd: String,
    expected: Value,
    #[serde(default)]
    stderr: Vec<String>,
}

/// The named recorded context case.
fn context_case(name: &str) -> ContextCase {
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/resources/context_files.json")).unwrap();
    let cases: Vec<ContextCase> = serde_json::from_value(corpus["cases"].clone()).unwrap();
    cases.into_iter().find(|case| case.name == name).unwrap()
}

/// The context files found for the case, limited to the case root, with placeholders.
fn discover(base: &str, case: &ContextCase) -> Value {
    let root = Root::new(base, &case.name);
    root.write_tree(&case.tree);
    let cwd = format!("{}/{}", root.path, case.cwd);
    let files = load_project_context_files(&cwd, &root.agent()).unwrap();
    let within: Vec<_> = (files.iter())
        .filter(|file| file.path.starts_with(&root.path))
        .collect();
    root.normalize(&json!(within))
}

/// Runs the named in-process case, which writes nothing to standard error.
fn check(name: &str) {
    let case = context_case(name);
    assert!(case.stderr.is_empty());
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    assert_eq!(
        discover(&scratch.path().display().to_string(), &case),
        case.expected
    );
}

/// Whether this host keeps the candidate spellings `AGENTS.md` and `AGENTS.MD` apart. The
/// recorded cases that author an upper-case candidate assume it does; a case-folding host
/// finds the first candidate under either spelling, so those cases state their premise.
fn candidates_kept_apart(test: &str) -> bool {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let names = ["AGENTS.md", "AGENTS.MD"].map(std::ffi::OsStr::new);
    let kept = scratch.keeps_distinct(&names).unwrap();
    if !kept {
        eprintln!("{test} needs a host that keeps {names:?} apart");
    }
    kept
}

#[test]
fn context_files_run_from_global_through_root_to_cwd() {
    if candidates_kept_apart("context_files_run_from_global_through_root_to_cwd") {
        check("global_then_root_to_cwd");
    }
}

#[test]
fn first_context_candidate_wins_in_each_directory() {
    if candidates_kept_apart("first_context_candidate_wins_in_each_directory") {
        check("first_candidate_wins_per_directory");
    }
}

#[test]
fn global_context_file_is_listed_once() {
    check("global_file_seen_once_when_cwd_is_agent_dir");
}

#[test]
fn missing_context_files_yield_nothing() {
    check("no_context_files_anywhere");
}

/// The warning line written for `path`, a directory, with the native error for reading one.
fn read_warning(prefix: &str, path: &str) -> String {
    format!(
        "Warning: Could not read {prefix}{path}: {}",
        directory_read_error()
    )
}

#[test]
fn unreadable_context_candidate_warns_and_falls_through() {
    if !candidates_kept_apart("unreadable_context_candidate_warns_and_falls_through") {
        return;
    }
    let case = context_case("unreadable_candidate_falls_through");
    let Some((base, stderr)) = child_stderr(
        "unreadable_context_candidate_warns_and_falls_through",
        |base| {
            assert_eq!(discover(base, &case), case.expected);
        },
    ) else {
        return;
    };
    let unreadable = format!("{base}/{}/project/AGENTS.md", case.name);
    assert_eq!(warnings(&stderr), [read_warning("", &unreadable)]);
}

#[test]
fn unreadable_system_prompt_file_warns_and_keeps_its_path() {
    let name = "unreadable_system_file_keeps_its_path";
    let Some((base, stderr)) = child_stderr(
        "unreadable_system_prompt_file_warns_and_keeps_its_path",
        |base| {
            let case = loader_case(name);
            let root = Root::new(base, name);
            let observed =
                on_task_set(|local| async move { run_loader_case(&root, &case, local).await });
            assert_eq!(observed, loader_case(name).expected);
        },
    ) else {
        return;
    };
    let unreadable = format!("{base}/{name}/project/.maestro/SYSTEM.md");
    assert_eq!(
        warnings(&stderr),
        [read_warning("system prompt file ", &unreadable)]
    );
}

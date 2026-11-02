//! The shipped search tool through the composition root's native process and filesystem owners.
#![cfg(unix)]
use crate::support::Root;
use maestro_app::AgentSession;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap, fmt::Write as _, os::unix::fs::PermissionsExt as _, path::PathBuf,
    rc::Rc,
};

/// Component path inherited by the checked child, whose environment is cleared.
const COMPONENT: &str = "MAESTRO_TRUNCATED_TOOL_COMPONENT";

/// Returns supplied process streams and status without interpreting search operands.
const SEARCH_PROCESS: &str = r#"#!/usr/bin/python3
import json, os, sys
from pathlib import Path
root = Path(__file__).parent
(root / "invocation.json").write_text(json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd()}))
if (root / "vanish").exists():
    Path(__file__).unlink()
sys.stdout.buffer.write((root / "stdout").read_bytes())
sys.stderr.buffer.write((root / "stderr").read_bytes())
sys.exit(int((root / "code").read_text()))
"#;

/// Runs each process fixture on a checked child PATH, independent of installed search tools.
fn isolated(name: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let executable = directory.path().join("rg");
    std::fs::write(&executable, SEARCH_PROCESS).unwrap();
    std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let component =
        std::env::var(COMPONENT).expect("nextest author-component setup builds truncated_tool");
    let environment = BTreeMap::from([
        ("PATH", directory.path().display().to_string()),
        (COMPONENT, component),
    ]);
    maestro_test_process::run_per_environment(name, [()], |()| environment.clone())
        .unwrap()
        .is_none()
}

/// Resolves a fixture file beside the executable at the front of the checked child PATH.
fn process_file(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .next()
        .unwrap()
        .join(name)
}

/// Supplies one complete process outcome independently of the process working directory.
fn supply(stdout: &str, stderr: &str, code: i32) {
    for (name, content) in [
        ("stdout", stdout),
        ("stderr", stderr),
        ("code", &code.to_string()),
    ] {
        std::fs::write(process_file(name), content).unwrap();
    }
}

/// Checks literal operands and the actual native process working directory.
fn assert_process(root: &Root, argv: &[&str]) {
    let observed: Value =
        serde_json::from_slice(&std::fs::read(process_file("invocation.json")).unwrap()).unwrap();
    assert_eq!(observed, json!({"argv":argv,"cwd":root.path("project")}));
}

/// Runs a component session on the application's local executor.
fn run(action: impl AsyncFnOnce(Rc<tokio::task::LocalSet>)) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, action(local.clone()));
}

/// Loads the prebuilt shipped copy with the application's native effects.
async fn loaded(root: &Root, local: Rc<tokio::task::LocalSet>) -> Rc<AgentSession> {
    crate::support::session::component_session(
        root,
        local,
        std::env::var(COMPONENT).unwrap(),
        None,
        crate::support::widgets(),
    )
    .await
}

/// Executes the registered tool, retaining its complete caller-visible result.
async fn search(session: &AgentSession, args: Value) -> Result<Value, String> {
    let runner = session.extension_runner();
    let tools = runner.get_all_registered_tools();
    assert_eq!(tools.len(), 1);
    let tool = maestro_app::wrap_registered_tool(&tools[0], runner);
    (tool.execute)("search".into(), args, None, None)
        .await
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

#[test]
fn truncated_tool_component_reports_no_matches_and_search_errors() {
    if isolated("truncated_tool_component_reports_no_matches_and_search_errors") {
        return;
    }
    run(async |local| {
        let root = Root::new();
        supply("", "", 1);
        let session = loaded(&root, local).await;
        assert_eq!(
            search(&session, json!({"pattern":"needle"})).await.unwrap(),
            json!({"content":[{"type":"text","text":"No matches found"}],
                "details":{"pattern":"needle","matchCount":0}})
        );
        assert_process(
            &root,
            &["--line-number", "--color=never", "--", "needle", "."],
        );
        supply("", "regex parse error: controlled error", 2);
        let error = search(&session, json!({"pattern":"["})).await.unwrap_err();
        assert_eq!(error, "ripgrep failed: regex parse error: controlled error");
        assert_process(&root, &["--line-number", "--color=never", "--", "[", "."]);
    });
}

#[test]
fn truncated_tool_component_reports_no_matches_from_its_only_search() {
    if isolated("truncated_tool_component_reports_no_matches_from_its_only_search") {
        return;
    }
    run(async |local| {
        let root = Root::new();
        supply("", "", 1);
        std::fs::write(process_file("vanish"), "").unwrap();
        let session = loaded(&root, local).await;
        let result = search(&session, json!({"pattern":"needle"})).await.unwrap();
        assert_eq!(result["content"][0]["text"], "No matches found");
        assert_process(
            &root,
            &["--line-number", "--color=never", "--", "needle", "."],
        );
        assert!(!process_file("rg").exists());
    });
}

/// Searches after `prevent` stops the process from starting, returning the search error.
fn rejected(name: &str, prevent: impl FnOnce(&Root)) -> String {
    let mut error = String::new();
    run(async |local| {
        let root = Root::new();
        let session = loaded(&root, local).await;
        prevent(&root);
        error = search(&session, json!({"pattern":"needle"}))
            .await
            .unwrap_err();
    });
    assert!(
        !process_file("invocation.json").exists(),
        "{name} started rg"
    );
    error
}

#[test]
fn truncated_tool_component_rejects_missing_executable() {
    if isolated("truncated_tool_component_rejects_missing_executable") {
        return;
    }
    let error = rejected("missing executable", |_| {
        std::fs::remove_file(process_file("rg")).unwrap();
    });
    assert_eq!(
        error,
        "ripgrep failed: No such file or directory (os error 2)"
    );
}

#[test]
fn truncated_tool_component_rejects_removed_working_directory() {
    if isolated("truncated_tool_component_rejects_removed_working_directory") {
        return;
    }
    let error = rejected("removed working directory", |root| {
        std::fs::remove_dir(root.path("project")).unwrap();
    });
    assert_eq!(
        error,
        "ripgrep failed: No such file or directory (os error 2)"
    );
}

#[test]
fn truncated_tool_component_rejects_permission_denied() {
    if isolated("truncated_tool_component_rejects_permission_denied") {
        return;
    }
    let error = rejected("permission denied", |_| {
        std::fs::set_permissions(process_file("rg"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
    });
    assert_eq!(error, "ripgrep failed: Permission denied (os error 13)");
}

#[test]
fn truncated_tool_component_preserves_literal_search_operands_and_output() {
    if isolated("truncated_tool_component_preserves_literal_search_operands_and_output") {
        return;
    }
    run(async |local| {
        let root = Root::new();
        supply("space dir/match me.txt:1:-needle;touch escaped\n", "", 0);
        let session = loaded(&root, local).await;
        let args = json!({"pattern":"-needle;touch escaped","path":"space dir","glob":"*.txt"});
        let result = search(&session, args).await.unwrap();
        assert_eq!(
            result,
            json!({"content":[{"type":"text","text":
            "space dir/match me.txt:1:-needle;touch escaped\n"}],"details":{
            "pattern":"-needle;touch escaped","path":"space dir","glob":"*.txt","matchCount":1}})
        );
        assert_process(
            &root,
            &[
                "--line-number",
                "--color=never",
                "--glob",
                "*.txt",
                "--",
                "-needle;touch escaped",
                "space dir",
            ],
        );
        assert!(!root.path("project/escaped").exists());
    });
}

/// A completed result must already name the full, private artifact it retained.
fn assert_artifact(result: &Value, expected: &str) -> String {
    let path = result["details"]["fullOutputPath"].as_str().unwrap();
    let file = std::path::Path::new(path);
    assert_eq!(file.file_name().unwrap(), "output.txt");
    let directory = file.parent().unwrap();
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("maestro-rg-")
    );
    assert_eq!(std::fs::read_to_string(file).unwrap(), expected);
    assert_eq!(
        std::fs::metadata(directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(directory).unwrap();
    path.into()
}

/// Filename, input, match count, limiting quota, retained/total lines and bytes, and notice.
type TruncationCase = (
    &'static str,
    String,
    i32,
    &'static str,
    i32,
    i32,
    i32,
    i32,
    &'static str,
);

/// Independent line and byte accounting for the authored search fixtures.
fn truncation_cases() -> [TruncationCase; 3] {
    [
        (
            "many.txt",
            "x\n".repeat(2001),
            2001,
            "lines",
            2000,
            2001,
            12892,
            12900,
            "showing 2000 of 2001 lines (12.6KB of 12.6KB). 1 lines (8B) omitted.",
        ),
        (
            "bytes.txt",
            format!("{0}\n{0}\n", "x".repeat(30000)),
            2,
            "bytes",
            1,
            2,
            30002,
            60006,
            "showing 1 of 2 lines (29.3KB of 58.6KB). 1 lines (29.3KB) omitted.",
        ),
        (
            "long.txt",
            format!("{}\n", "雪".repeat(18000)),
            1,
            "bytes",
            0,
            1,
            0,
            54003,
            "showing 0 of 1 lines (0B of 52.7KB). 1 lines (52.7KB) omitted.",
        ),
    ]
}

#[test]
fn truncated_tool_component_limits_lines_and_bytes_and_retains_full_output() {
    if isolated("truncated_tool_component_limits_lines_and_bytes_and_retains_full_output") {
        return;
    }
    run(async |local| {
        let root = Root::new();
        let session = loaded(&root, local).await;
        for (name, input, count, limit, lines, total, bytes, total_bytes, notice) in
            truncation_cases()
        {
            let mut full = String::new();
            for (line, text) in input.lines().enumerate() {
                writeln!(full, "{}:{text}", line + 1).unwrap();
            }
            supply(&full, "", 0);
            let result = search(&session, json!({"pattern":".","path":name}))
                .await
                .unwrap();
            assert_process(&root, &["--line-number", "--color=never", "--", ".", name]);
            let path = assert_artifact(&result, &full);
            assert_eq!(result["details"]["matchCount"], count);
            let truncation = &result["details"]["truncation"];
            assert_eq!(truncation["truncated"], true);
            assert_eq!(truncation["truncatedBy"], limit);
            assert_eq!(truncation["outputLines"].as_f64(), Some(f64::from(lines)));
            assert_eq!(truncation["totalLines"].as_f64(), Some(f64::from(total)));
            assert_eq!(truncation["outputBytes"].as_f64(), Some(f64::from(bytes)));
            assert_eq!(
                truncation["totalBytes"].as_f64(),
                Some(f64::from(total_bytes))
            );
            assert_eq!(truncation["maxLines"].as_f64(), Some(2000.0));
            assert_eq!(truncation["maxBytes"].as_f64(), Some(51200.0));
            let content = result["content"][0]["text"].as_str().unwrap();
            assert!(content.ends_with(&format!(
                "\n\n[Output truncated: {notice} Full output saved to: {path}]"
            )));
            if name == "many.txt" {
                assert!(content.starts_with("1:x\n2:x\n"));
                assert!(content.contains("2000:x\n\n[Output truncated:"));
                assert!(!content.contains("2001:x"));
            }
        }
    });
}

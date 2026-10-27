//! The shipped search tool through the production loader, process and filesystem owners.
#![cfg(test)]
#![cfg(unix)]
use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    rc::Rc,
    sync::RwLock,
};

use super::extension_guide::cargo as component_build;

/// Component path inherited by the checked child, so it need not build with a cleared environment.
const COMPONENT: &str = "MAESTRO_TRUNCATED_TOOL_COMPONENT";

/// Returns supplied process streams and status without interpreting search operands.
const SEARCH_PROCESS: &str = r#"#!/usr/bin/python3
import json, os, sys
from pathlib import Path
root = Path(__file__).parent
(root / "invocation.json").write_text(json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd()}))
sys.stdout.buffer.write((root / "stdout").read_bytes())
sys.stderr.buffer.write((root / "stderr").read_bytes())
sys.exit(int((root / "code").read_text()))
"#;

/// Runs each process fixture on a checked child PATH, independent of installed search tools.
fn isolated(name: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let directory = Scratch::new().unwrap();
    let executable = directory.path().join("rg");
    std::fs::write(&executable, SEARCH_PROCESS).unwrap();
    std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let component = component_build::component(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../maestro-extensions-wasm"
        )),
        &["--example", "truncated_tool"],
        "truncated_tool",
    )
    .unwrap();
    let environment = BTreeMap::from([
        (
            "PATH",
            format!(
                "{}:{}",
                directory.path().display(),
                std::env::var("PATH").unwrap()
            ),
        ),
        (COMPONENT, component.to_str().unwrap().to_owned()),
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
fn assert_process(root: &Scratch, argv: &[&str]) {
    let observed: Value =
        serde_json::from_slice(&std::fs::read(process_file("invocation.json")).unwrap()).unwrap();
    assert_eq!(observed, json!({"argv":argv,"cwd":root.path()}));
}

/// Runs a component invocation on the host's local executor.
fn run(action: impl std::future::Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}

/// Loads the prebuilt shipped copy over real native owners.
async fn loaded(cwd: &Path) -> Rc<ExtensionRunner<Local>> {
    let path = std::env::var(COMPONENT).unwrap();
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    let bus = maestro_extensions::create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no background work")),
        Rc::new(|error| panic!("{error}")),
    );
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session(cwd.to_str().unwrap()).0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    ))
}

/// Executes the registered tool, retaining its complete caller-visible result.
async fn search(runner: &Rc<ExtensionRunner<Local>>, args: Value) -> Result<Value, String> {
    let tools = runner.get_all_registered_tools();
    assert_eq!(tools.len(), 1);
    let tool = maestro_extensions::wrap_registered_tool(&tools[0], runner.clone());
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
    run(async {
        let root = Scratch::new().unwrap();
        supply("", "", 1);
        let runner = loaded(root.path()).await;
        assert_eq!(
            search(&runner, json!({"pattern":"needle"})).await.unwrap(),
            json!({"content":[{"type":"text","text":"No matches found"}],
                "details":{"pattern":"needle","matchCount":0}})
        );
        assert_process(
            &root,
            &["--line-number", "--color=never", "--", "needle", "."],
        );
        supply("", "regex parse error: controlled error", 2);
        let error = search(&runner, json!({"pattern":"["})).await.unwrap_err();
        assert_eq!(error, "ripgrep failed: regex parse error: controlled error");
        assert_process(&root, &["--line-number", "--color=never", "--", "[", "."]);
    });
}

#[test]
fn truncated_tool_component_preserves_literal_search_operands_and_output() {
    if isolated("truncated_tool_component_preserves_literal_search_operands_and_output") {
        return;
    }
    run(async {
        let root = Scratch::new().unwrap();
        supply("space dir/match me.txt:1:-needle;touch escaped\n", "", 0);
        let runner = loaded(root.path()).await;
        let args = json!({"pattern":"-needle;touch escaped","path":"space dir","glob":"*.txt"});
        let result = search(&runner, args).await.unwrap();
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
        assert!(!root.path().join("escaped").exists());
    });
}

/// A completed result must already name the full, private artifact it retained.
fn assert_artifact(result: &Value, expected: &str) -> String {
    let path = result["details"]["fullOutputPath"].as_str().unwrap();
    let file = Path::new(path);
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
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
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
            2002,
            12892,
            12900,
            "showing 2000 of 2002 lines (12.6KB of 12.6KB). 2 lines (8B) omitted.",
        ),
        (
            "bytes.txt",
            format!("{0}\n{0}\n", "x".repeat(30000)),
            2,
            "bytes",
            1,
            3,
            30002,
            60006,
            "showing 1 of 3 lines (29.3KB of 58.6KB). 2 lines (29.3KB) omitted.",
        ),
        (
            "long.txt",
            format!("{}\n", "雪".repeat(18000)),
            1,
            "bytes",
            0,
            2,
            0,
            54003,
            "showing 0 of 2 lines (0B of 52.7KB). 2 lines (52.7KB) omitted.",
        ),
    ]
}

#[test]
fn truncated_tool_component_limits_lines_and_bytes_and_retains_full_output() {
    if isolated("truncated_tool_component_limits_lines_and_bytes_and_retains_full_output") {
        return;
    }
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        for (name, input, count, limit, lines, total, bytes, total_bytes, notice) in
            truncation_cases()
        {
            let mut full = String::new();
            for (line, text) in input.lines().enumerate() {
                writeln!(full, "{}:{text}", line + 1).unwrap();
            }
            supply(&full, "", 0);
            let result = search(&runner, json!({"pattern":".","path":name}))
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

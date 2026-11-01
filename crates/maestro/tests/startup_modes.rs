//! Concrete frontend and native component routes through the executable.
#![cfg(target_os = "linux")]
use crate::support::{
    Root,
    rpc::Rpc,
    screen::{benchmark, resource},
};
use serde_json::{Value, json};

#[test]
fn root_dispatches_concrete_chat_and_rpc() {
    let root = Root::new();
    let output = benchmark(&root, &[]);
    assert!(
        output.contains("\x1b[?2004h"),
        "terminal never started: {output:?}"
    );
    assert!(
        output.contains("\x1b[?2004l"),
        "terminal never stopped: {output:?}"
    );
    root.models("http://127.0.0.1:1/v1");
    let mut rpc = Rpc::start(&root);
    assert_eq!(
        rpc.request(json!({"type":"get_state"}))["model"]["id"],
        "controlled"
    );
    rpc.finish();
}

/// The component's current application view, delivered through the real RPC UI adapter.
fn snapshot(rpc: &mut Rpc) -> Value {
    let start = rpc.events.len();
    rpc.request(json!({"type":"prompt", "message":"/observe-resources observe"}));
    rpc.events[start..]
        .iter()
        .find_map(|event| {
            event["title"]
                .as_str()
                .and_then(|title| title.strip_prefix("ROOT-OBSERVE:"))
                .map(|text| serde_json::from_str(text).unwrap())
        })
        .expect("component title through RPC")
}

/// Start the shipped root with its native loader and a startup-relative component operand.
fn loaded(root: &Root) -> Rpc {
    root.models("http://127.0.0.1:1/v1");
    Rpc::command(&mut root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        "./extensions/index.wasm",
    ]))
}

#[test]
fn app_loader_uses_components_for_initial_and_replaced_cwd() {
    let root = Root::new();
    let base = crate::support::resources(&root);
    let mut rpc = loaded(&root);
    let initial = snapshot(&mut rpc);
    initial_resources(&initial, &root, &base);
    let output = resource(
        &root,
        &["--verbose", "--extension", "./extensions/index.wasm"],
        "one/dynamic.json",
    );
    assert!(output.contains("one/dynamic.json"), "{output}");
    let other = root.path("other");
    std::fs::create_dir_all(other.join(".maestro")).unwrap();
    std::fs::write(
        other.join(".maestro/settings.json"),
        r#"{"defaultThinkingLevel":"high"}"#,
    )
    .unwrap();
    let session = root.path("replacement.jsonl");
    std::fs::write(&session, json!({"type":"session","version":3,"id":"replaced","timestamp":"2026-01-01T00:00:00.000Z","cwd":other}).to_string()+"\n").unwrap();
    rpc.request(json!({"type":"switch_session", "sessionPath":session}));
    let replaced = snapshot(&mut rpc);
    assert_eq!(replaced["cwd"], other.to_str().unwrap());
    assert_eq!(replaced["calls"], 1);
    assert_eq!(replaced["thinking"], "high");
    assert!(
        replaced["commands"]
            .to_string()
            .contains(&base.join("one/dynamic.md").display().to_string())
    );
    assert!(
        !replaced["commands"]
            .to_string()
            .contains("other/extensions")
    );
    rpc.finish();
    let output = resource(
        &root,
        &[
            "--verbose",
            "--extension",
            "./extensions/index.wasm",
            "--session",
            session.to_str().unwrap(),
        ],
        "one/dynamic.json",
    );
    assert!(output.contains("one/dynamic.json"), "{output}");
    let bad = base.join("bad.wasm");
    std::fs::write(&bad, b"invalid component").unwrap();
    let failure = root.output(&["--extension", bad.to_str().unwrap()]);
    assert_eq!(failure.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failure.stderr).contains(bad.to_str().unwrap()));
}

#[test]
fn app_reload_reads_current_component_bytes_and_resources() {
    let root = Root::new();
    let base = crate::support::resources(&root);
    let mut rpc = loaded(&root);
    let before = snapshot(&mut rpc);
    assert_eq!(before["calls"], 1);
    assert_eq!(snapshot(&mut rpc)["calls"], 2);
    std::fs::copy(
        std::env::var_os("MAESTRO_RESOURCE_RELOADED_COMPONENT")
            .expect("nextest author-component setup"),
        base.join("index.wasm"),
    )
    .unwrap();
    rpc.request(json!({"type":"prompt", "message":"/observe-resources reload"}));
    let after = snapshot(&mut rpc);
    assert_eq!(after["version"], "two");
    assert_eq!(after["calls"], 1);
    assert!(
        after["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "resource-two")
    );
    assert!(
        !after["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "resource-one")
    );
    assert!(
        after["commands"]
            .to_string()
            .contains(&base.join("two/dynamic.md").display().to_string())
    );
    assert!(after["commands"].to_string().contains("resource-two"));
    assert_eq!(before["version"], "one");
    assert_eq!(before["calls"], 1);
    rpc.finish();
    let output = resource(
        &root,
        &["--verbose", "--extension", "./extensions/index.wasm"],
        "two/dynamic.json",
    );
    assert!(output.contains("two/dynamic.json"), "{output}");
}

/// Check the first externally returned component registration and resource view.
fn initial_resources(initial: &Value, root: &Root, base: &std::path::Path) {
    assert_eq!(initial["version"], "one");
    assert_eq!(initial["flag"], "one");
    assert_eq!(initial["calls"], 1);
    assert_eq!(initial["cwd"], root.path("project").to_str().unwrap());
    assert!(
        initial["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool == "resource-one")
    );
    assert!(
        initial["commands"]
            .to_string()
            .contains(&base.join("one/dynamic.md").display().to_string())
    );
    assert!(initial["commands"].to_string().contains("resource-one"));
}

#[test]
fn compiled_user_bash_operations_reach_the_interactive_shell_cursor() {
    let root = Root::new();
    root.models("http://127.0.0.1:1/v1");
    std::fs::write(
        root.path("agent/settings.json"),
        r#"{"shellPath":"/missing/user-shell"}"#,
    )
    .unwrap();
    let session = root.path("shell.jsonl");
    std::fs::write(&session, json!({"type":"session","version":3,"id":"local-shell","timestamp":"2026-01-01T00:00:00.000Z","cwd":root.path("project")}).to_string()+"\n").unwrap();
    let component = std::env::var("MAESTRO_LOCAL_SHELL_COMPONENT").unwrap();
    let command = "exec 2>&1; printf '\\033[31mRAW\\033[0m\\000\\r\\n'; exit 7";
    let output = crate::support::screen::shell(
        &root,
        &[
            "--provider",
            "startup",
            "--model",
            "controlled",
            "--extension",
            &component,
            "--session",
            session.to_str().unwrap(),
        ],
        &format!("!{command}"),
    );
    assert!(output.contains("RAW"));
    let history = std::fs::read_to_string(session).unwrap();
    let records: Vec<Value> = history
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter_map(|record| {
            (record["message"]["role"] == "bashExecution").then(|| record["message"].clone())
        })
        .collect();
    assert_eq!(records.len(), 1, "{history}");
    assert_eq!(records[0]["command"], command);
    assert_eq!(records[0]["output"], "RAW\n");
    assert_eq!(records[0]["exitCode"], 7.0);
    assert_eq!(records[0]["cancelled"], false);
}

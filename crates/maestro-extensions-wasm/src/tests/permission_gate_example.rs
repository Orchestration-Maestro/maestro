//! The shipped permission hook delivered through native and built-component registrations.
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use super::ui_host;
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/permission_gate.rs"]
mod permission_gate;

/// The factory exported by the shipped example.
struct PermissionGate;

impl Extension for PermissionGate {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        permission_gate::factory(api)
    }
}

/// One tool call and the scripted UI decision, expected verdict and optional host rejection.
struct Case {
    /// Original event document.
    event: Value,
    /// Whether the host has an interactive UI.
    has_ui: bool,
    /// Choice returned by the host, or null for dismissal.
    choice: Value,
    /// Whether a selection must be requested.
    asks: bool,
    /// Exact caller-visible termination.
    ended: Ended,
    /// Optional error returned by the UI host.
    error: Option<&'static str>,
}

/// Creates a tool call with a literal, independent expected result.
fn case(tool: &str, command: &str, dangerous: bool, choice: Value) -> Case {
    let result =
        (dangerous && choice != "Yes").then(|| json!({"block":true,"reason":"Blocked by user"}));
    Case {
        event: json!({"type":"tool_call","toolCallId":"gate-call","toolName":tool,
            "input":{"command":command,"untouched":777}}),
        has_ui: true,
        choice,
        asks: dangerous,
        ended: Ended::Returned(result),
        error: None,
    }
}

/// Commands matched by the authored expressions, including their prefix and Unicode cases.
const DANGEROUS: &[&str] = &[
    "rm -r work",
    "rm -rf work",
    "RM\t--recursive work",
    "sudo whoami",
    "echo SUDO",
    "chmod 777 work",
    "chown root 777work",
    "chmod\t-R 777 work",
    "rm\u{feff}-r work",
    "ésudo whoami",
    "chmod\u{85}777 work",
    "rm -rubbish",
    "rm --recursively work",
];

/// Safe commands and near misses, including excluded line terminators.
const SAFE: &[&str] = &[
    "",
    "ls -la",
    "rm -f work",
    "rm -fr work",
    "arm -rf work",
    "_sudo",
    "sudoku",
    "chmod 755 work",
    "chown root work",
    "chmodx 777 work",
    "chmod\n777 work",
    "chown\r777 work",
    "chmod\u{2028}777 work",
    "chmod\u{2029}777 work",
    "rm\u{85}-rf work",
];

/// Authored pattern examples, near misses, UI outcomes and a rejected dialog.
fn cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for command in DANGEROUS {
        for choice in [json!("Yes"), json!("No"), Value::Null, json!("yes")] {
            rows.push(case("bash", command, true, choice));
        }
        let mut headless = case("bash", command, true, json!("Yes"));
        headless.has_ui = false;
        headless.asks = false;
        headless.ended = Ended::Returned(Some(json!({"block":true,
            "reason":"Dangerous command blocked (no UI for confirmation)"})));
        rows.push(headless);
    }
    for command in SAFE {
        rows.push(case("bash", command, false, json!("No")));
    }
    for tool in ["write", "edit", "custom", "Bash"] {
        let mut row = case(tool, "sudo rm -rf work", false, json!("No"));
        row.event["input"] = json!({"path":"sudo"});
        rows.push(row);
    }
    let mut rejected = case("bash", "sudo whoami", true, Value::Null);
    rejected.error = Some("selector unavailable");
    rejected.ended = Ended::Failed("selector unavailable".to_owned());
    rows.push(rejected);
    rows
}

/// Checks the unmodified event, exact verdict, prompt, choices and resource balance.
fn assert_case(row: &Case, answer: &Answer, lines: &[String]) {
    assert_eq!(answer.event.as_ref(), Some(&row.event));
    assert_eq!(
        answer.ended, row.ended,
        "{}; UI {}; choice {}",
        row.event, row.has_ui, row.choice
    );
    let requests = ui_host::receipts(lines);
    assert_eq!(requests.len(), usize::from(row.asks), "{}", row.event);
    if row.asks {
        assert_eq!(requests[0].method, "select");
        let command = row.event["input"]["command"].as_str().unwrap();
        assert_eq!(
            requests[0].operands,
            json!([
                format!("⚠️ Dangerous command:\n\n  {command}\n\nAllow?"),
                ["Yes", "No"],
                null,
                null
            ])
        );
    }
}

/// Every recorded decision runs through the shipped factory's registered event callback.
#[test]
fn permission_gate_hook_preserves_patterns_and_decisions() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<PermissionGate>().await?;
        let key = host.observed().callback("event tool_call")?;
        for row in cases() {
            host.observe().catalog.data.has_ui = row.has_ui;
            host.observe()
                .ui
                .control(&json!({"answer":row.choice,"error":row.error}).to_string());
            let from = host.observed().transcript.len();
            let lent = host.observed().live;
            let outcome = exports
                .invoke_event(
                    key,
                    row.event.to_string(),
                    Capabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                    },
                )
                .await?;
            assert_case(
                &row,
                &Answer::of(super::guest_family::delivery(outcome))?,
                &host.observed().transcript[from..],
            );
            assert_eq!(host.observed().live, lent);
        }
        release_all(&host, &["event tool_call"]);
        Ok(())
    })
}

/// The built component runs the same calls without executing any shell command.
#[test]
fn permission_gate_component_preserves_patterns_and_decisions() -> Result<(), String> {
    let path = super::build::example_component("permission_gate")?;
    let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
    let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|e| e.to_string())?;
    let key = harness
        .callback("event tool_call")
        .map_err(|e| e.to_string())?;
    for row in cases() {
        harness.observed().catalog.data.has_ui = row.has_ui;
        harness
            .observed()
            .ui
            .control(&json!({"answer":row.choice,"error":row.error}).to_string());
        let from = harness.observed().transcript.len();
        let lent = harness.observed().live;
        let capabilities = super::host_family::guest::Capabilities {
            ctx: harness.ordinary("/controlled").map_err(|e| e.to_string())?,
            signal: None,
        };
        let exports = harness.exports.clone();
        let event = row.event.to_string();
        let outcome = super::block_on(harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), event, capabilities)
                .await
        }))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())??;
        let lines = harness.observed().transcript[from..].to_vec();
        assert_case(
            &row,
            &Answer::of(super::host_family::delivery(outcome))?,
            &lines,
        );
        assert_eq!(harness.observed().live, lent);
    }
    super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

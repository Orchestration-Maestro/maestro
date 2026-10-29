//! The shipped dirty-repository guard at the registered guest event interface.
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use super::ui_host;
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/dirty_repo_guard.rs"]
mod dirty_repo_guard;

/// Loads the same factory exported by the built component.
struct DirtyRepoGuard;
impl Extension for DirtyRepoGuard {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        dirty_repo_guard::factory(api)
    }
}

/// The only command the guard runs.
const STATUS: &str = r#"action exec ["git",["status","--porcelain"],null]"#;

/// The selector choices, affirmative first.
const CHOICES: [&str; 2] = ["Continue", "Cancel and commit first"];

/// One session change, supplied status output, UI state and exact observable outcome.
struct Case {
    /// Event document supplied to its registered callback.
    event: Value,
    /// The status run's result, or the host's refusal to run it.
    status: Result<Value, &'static str>,
    /// Whether UI is available.
    has_ui: bool,
    /// Scripted selector answer, null for dismissal.
    choice: Value,
    /// Optional UI method the host refuses.
    reject: Option<&'static str>,
    /// Exact ordered UI requests.
    requests: Value,
    /// Callback verdict or propagated error.
    ended: Ended,
}

/// The three guarded session changes and the action each names in its prompt.
fn changes() -> [(Value, &'static str); 3] {
    [
        (
            json!({"type":"session_before_switch","reason":"new"}),
            "Start a new session",
        ),
        (
            json!({"type":"session_before_switch","reason":"resume","targetSessionFile":"other.jsonl"}),
            "Switch sessions",
        ),
        (
            json!({"type":"session_before_fork","entryId":"entry","position":"at"}),
            "Fork",
        ),
    ]
}

/// A completed status run with the supplied output and exit code.
fn status(stdout: &str, code: i32) -> Value {
    json!({"stdout":stdout,"stderr":"","code":code,"killed":false})
}

/// The selector request for `changed` files before `action`.
fn select(action: &str, changed: usize) -> Value {
    json!([
        "select",
        [
            format!("{action} with {changed} uncommitted file(s)?"),
            CHOICES,
            null,
            null
        ]
    ])
}

/// The warning sent after a refusal.
fn warning() -> Value {
    json!([
        "notify",
        ["Cancelled. Commit the changes, then try again.", "Warning"]
    ])
}

/// A case that asks nothing and lets the change go ahead.
fn allowed(event: &Value, status: Result<Value, &'static str>, has_ui: bool) -> Case {
    Case {
        event: event.clone(),
        status,
        has_ui,
        choice: json!(CHOICES[0]),
        reject: None,
        requests: json!([]),
        ended: Ended::Returned(None),
    }
}

/// Failed status runs, clean trees, headless refusal, every selector answer and host failures.
fn cases() -> Vec<Case> {
    let mut rows = Vec::new();
    let dirty = " M tracked.txt\n\n?? new file.txt\n";
    for (event, action) in changes() {
        for code in [128, 1, -1] {
            rows.push(allowed(&event, Ok(status(dirty, code)), true));
        }
        for clean in ["", "\n", " \n\t"] {
            rows.push(allowed(&event, Ok(status(clean, 0)), true));
        }
        let mut headless = allowed(&event, Ok(status(dirty, 0)), false);
        headless.ended = Ended::Returned(Some(json!({"cancel":true})));
        rows.push(headless);
        rows.push(Case {
            requests: json!([select(action, 2)]),
            ..allowed(&event, Ok(status(dirty, 0)), true)
        });
        rows.push(Case {
            requests: json!([select(action, 1)]),
            ..allowed(&event, Ok(status("?? only.txt", 0)), true)
        });
        for choice in [json!(CHOICES[1]), Value::Null, json!("continue")] {
            rows.push(Case {
                choice,
                requests: json!([select(action, 2), warning()]),
                ended: Ended::Returned(Some(json!({"cancel":true}))),
                ..allowed(&event, Ok(status(dirty, 0)), true)
            });
        }
        let mut refused = allowed(&event, Err("cannot run git"), true);
        refused.ended = Ended::Failed("cannot run git".into());
        rows.push(refused);
        for (method, requests) in [
            ("select", json!([select(action, 2)])),
            ("notify", json!([select(action, 2), warning()])),
        ] {
            rows.push(Case {
                choice: json!(CHOICES[1]),
                reject: Some(method),
                requests,
                ended: Ended::Failed("UI refused".into()),
                ..allowed(&event, Ok(status(dirty, 0)), true)
            });
        }
    }
    rows
}

/// The registered callback that receives `event`.
fn callback(event: &Value) -> String {
    format!("event {}", event["type"].as_str().unwrap())
}

/// Applies a case's status, UI availability and scripted answer to a controlled host.
fn script(observed: &mut super::observed::Observed, row: &Case) {
    observed.catalog.data.has_ui = row.has_ui;
    observed.actions.value = row.status.clone().unwrap_or(Value::Null);
    observed.actions.error = row.status.clone().err().map(str::to_owned);
    observed.ui.control(
        &json!({"answer":row.choice,"method":row.reject,
            "error":row.reject.map(|_| "UI refused")})
        .to_string(),
    );
}

/// Verifies the unchanged event, verdict, single status run and exact UI requests.
fn assert_case(row: &Case, answer: &Answer, lines: &[String]) {
    let context = format!("{} {:?} ui={}", row.event, row.status, row.has_ui);
    assert_eq!(answer.event.as_ref(), Some(&row.event), "{context}");
    assert_eq!(answer.ended, row.ended, "{context}");
    let actions: Vec<_> = lines
        .iter()
        .filter(|line| line.starts_with("action "))
        .collect();
    let expected = if row.status.is_ok() {
        vec![STATUS]
    } else {
        vec![]
    };
    assert_eq!(actions, expected, "{context}");
    let requests: Vec<_> = ui_host::receipts(lines)
        .into_iter()
        .map(|request| json!([request.method, request.operands]))
        .collect();
    assert_eq!(json!(requests), row.requests, "{context}");
}

/// Every case through the in-process guest event interface.
#[test]
fn dirty_repo_guard_session_changes() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<DirtyRepoGuard>().await?;
        for row in cases() {
            script(&mut host.observe(), &row);
            let key = host.observed().callback(&callback(&row.event))?;
            let from = host.observed().transcript.len();
            let live = host.observed().live;
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
            assert_eq!(host.observed().live, live);
        }
        release_all(
            &host,
            &["event session_before_switch", "event session_before_fork"],
        );
        Ok(())
    })
}

/// The built component replays every case with the same requests and verdicts.
#[test]
fn dirty_repo_guard_component_session_changes() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "dirty_repo_guard")?;
    let mut harness =
        super::block_on(super::host::Harness::start(&path)).map_err(|e| e.to_string())?;
    for row in cases() {
        script(harness.observed(), &row);
        let key = harness
            .callback(&callback(&row.event))
            .map_err(|e| e.to_string())?;
        let from = harness.observed().transcript.len();
        let live = harness.observed().live;
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
        assert_case(
            &row,
            &Answer::of(super::host_family::delivery(outcome))?,
            &harness.observed().transcript[from..],
        );
        assert_eq!(harness.observed().live, live);
    }
    for name in ["event session_before_switch", "event session_before_fork"] {
        let key = harness.callback(name).map_err(|e| e.to_string())?;
        super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

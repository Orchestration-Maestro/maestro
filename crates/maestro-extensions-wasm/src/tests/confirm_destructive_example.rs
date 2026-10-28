//! The shipped session confirmations at the registered guest event interface.
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use super::ui_host;
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/confirm_destructive.rs"]
mod confirm_destructive;

/// Loads the same factory exported by the built component.
struct ConfirmDestructive;
impl Extension for ConfirmDestructive {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        confirm_destructive::factory(api)
    }
}

/// One supplied event, history, dialog answer and exact observable outcome.
struct Case {
    /// Event document supplied to its registered callback.
    event: Value,
    /// Session entries supplied by the read-only host.
    entries: Value,
    /// Whether UI is available.
    has_ui: bool,
    /// Scripted dialog decision.
    decision: Value,
    /// Exact ordered UI requests.
    requests: Value,
    /// Callback verdict or propagated error.
    ended: Ended,
    /// Optional rejected UI method.
    reject: Option<&'static str>,
}

/// An assistant message without user history.
fn assistant() -> Value {
    json!({"type":"message","id":"assistant","parentId":"user",
        "timestamp":"2026-01-01T00:00:00Z","message":{
            "role":"assistant","content":[],"api":"controlled","provider":"controlled",
            "model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,
                "totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
            "stopReason":"stop","timestamp":0}})
}

/// Literal user history, followed by an assistant reply and a non-message entry.
fn history() -> Value {
    json!([
        {"type":"message","id":"user","parentId":null,"timestamp":"2026-01-01T00:00:00Z",
         "message":{"role":"user","content":"hello","timestamp":0}},
        assistant(),
        {"type":"custom","id":"state","parentId":"user","timestamp":"2026-01-01T00:00:00Z",
         "customType":"example","data":{}}
    ])
}

/// Replacement behavior with empty, non-user and user history, approval, dismissal and host failures.
fn switch_dialog_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for (reason, entries, title, message, cancelled) in [
        (
            "new",
            json!([]),
            "Clear session?",
            "This will delete all messages in the current session.",
            "Clear cancelled",
        ),
        (
            "new",
            history(),
            "Clear session?",
            "This will delete all messages in the current session.",
            "Clear cancelled",
        ),
        (
            "resume",
            history(),
            "Switch session?",
            "You have messages in the current session. Switch anyway?",
            "Switch cancelled",
        ),
    ] {
        for decision in [json!(true), json!(false), Value::Null] {
            let mut requests = vec![json!(["confirm", [title, message, null, null]])];
            let result = if decision == true {
                None
            } else {
                requests.push(json!(["notify", [cancelled, "Info"]]));
                Some(json!({"cancel":true}))
            };
            rows.push(Case {
                event: json!({"type":"session_before_switch","reason":reason,"targetSessionFile":"supplied.jsonl"}),
                entries: entries.clone(), has_ui: true, decision,
                requests: json!(requests), ended: Ended::Returned(result), reject: None,
            });
        }
    }
    rows
}

/// Headless delivery and histories without user messages never ask.
fn switch_bypass_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for reason in ["new", "resume"] {
        rows.push(Case {
            event: json!({"type":"session_before_switch","reason":reason}),
            entries: history(),
            has_ui: false,
            decision: json!(false),
            requests: json!([]),
            ended: Ended::Returned(None),
            reject: None,
        });
    }
    for entries in [
        json!([]),
        json!([assistant()]),
        json!([{"type":"custom","id":"only","parentId":null,
        "timestamp":"2026-01-01T00:00:00Z","customType":"user","data":{"role":"user"}}]),
    ] {
        rows.push(Case {
            event: json!({"type":"session_before_switch","reason":"resume"}),
            entries,
            has_ui: true,
            decision: json!(false),
            requests: json!([]),
            ended: Ended::Returned(None),
            reject: None,
        });
    }
    rows
}

/// Dialog and notification failures remain caller-visible errors.
fn switch_rejection_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for method in ["confirm", "notify"] {
        rows.push(Case {
            event: json!({"type":"session_before_switch","reason":"new"}),
            entries: json!([]),
            has_ui: true,
            decision: json!(false),
            requests: if method == "confirm" {
                json!([[
                    "confirm",
                    [
                        "Clear session?",
                        "This will delete all messages in the current session.",
                        null,
                        null
                    ]
                ]])
            } else {
                json!([
                    [
                        "confirm",
                        [
                            "Clear session?",
                            "This will delete all messages in the current session.",
                            null,
                            null
                        ]
                    ],
                    ["notify", ["Clear cancelled", "Info"]]
                ])
            },
            ended: Ended::Failed("UI refused".into()),
            reject: Some(method),
        });
    }
    rows
}

/// All replacement cases replayed on both adapters.
fn switch_cases() -> Vec<Case> {
    switch_dialog_cases()
        .into_iter()
        .chain(switch_bypass_cases())
        .chain(switch_rejection_cases())
        .collect()
}

/// Verifies exact requests, unchanged input and caller-visible termination.
fn assert_case(row: &Case, answer: &Answer, lines: &[String]) {
    assert_eq!(answer.event.as_ref(), Some(&row.event));
    assert_eq!(answer.ended, row.ended, "{}", row.event);
    let requests: Vec<_> = ui_host::receipts(lines)
        .into_iter()
        .map(|request| json!([request.method, request.operands]))
        .collect();
    assert_eq!(json!(requests), row.requests, "{}", row.event);
}

/// Delivers supplied cases through the in-process guest event interface.
fn native_cases(name: &str, cases: Vec<Case>) -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<ConfirmDestructive>().await?;
        let key = host.observed().callback(name)?;
        for row in cases {
            host.observe().catalog.data.has_ui = row.has_ui;
            host.observe().readers.control(
                &json!({"bind":{"replies":[{
                    "method":"getEntries","args":[],"value":row.entries
                }]}})
                .to_string(),
            );
            host.observe().ui.control(
                &json!({"answer":row.decision,
                "method":row.reject,"error":row.reject.map(|_| "UI refused")})
                .to_string(),
            );
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

/// Delivers supplied cases through the built shipped component.
fn component_cases(name: &str, cases: Vec<Case>) -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "confirm_destructive")?;
    let mut harness =
        super::block_on(super::host::Harness::start(&path)).map_err(|e| e.to_string())?;
    let key = harness.callback(name).map_err(|e| e.to_string())?;
    for row in cases {
        harness.observed().catalog.data.has_ui = row.has_ui;
        harness.observed().readers.control(
            &json!({"bind":{"replies":[{
                "method":"getEntries","args":[],"value":row.entries
            }]}})
            .to_string(),
        );
        harness.observed().ui.control(
            &json!({"answer":row.decision,
            "method":row.reject,"error":row.reject.map(|_| "UI refused")})
            .to_string(),
        );
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

/// Fork approval is exact, independent of fork position, and shows only the ID prefix.
fn fork_dialog_cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for (id, title, position) in [
        ("123456789abcdef", "Fork from entry 12345678?", "at"),
        ("short", "Fork from entry short?", "at"),
        ("", "Fork from entry ?", "at"),
        ("123456789abcdef", "Fork from entry 12345678?", "before"),
    ] {
        for decision in [
            json!("Yes, create fork"),
            json!("No, stay in current session"),
            Value::Null,
            json!("Yes, create fork please"),
            json!("yes, create fork"),
        ] {
            let mut requests = vec![json!([
                "select",
                [
                    title,
                    ["Yes, create fork", "No, stay in current session"],
                    null,
                    null
                ]
            ])];
            let result = if decision == "Yes, create fork" {
                None
            } else {
                requests.push(json!(["notify", ["Fork cancelled", "Info"]]));
                Some(json!({"cancel":true}))
            };
            rows.push(Case {
                event: json!({"type":"session_before_fork","entryId":id,"position":position}),
                entries: json!([]),
                has_ui: true,
                decision,
                requests: json!(requests),
                ended: Ended::Returned(result),
                reject: None,
            });
        }
    }
    rows
}

/// A headless host continues; refused selectors and notifications propagate their errors.
fn fork_cases() -> Vec<Case> {
    let mut rows = fork_dialog_cases();
    for position in ["at", "before"] {
        rows.push(Case {
            event: json!({"type":"session_before_fork","entryId":"headless","position":position}),
            entries: json!([]),
            has_ui: false,
            decision: Value::Null,
            requests: json!([]),
            ended: Ended::Returned(None),
            reject: None,
        });
    }
    for method in ["select", "notify"] {
        let mut requests = vec![json!([
            "select",
            [
                "Fork from entry rejected?",
                ["Yes, create fork", "No, stay in current session"],
                null,
                null
            ]
        ])];
        if method == "notify" {
            requests.push(json!(["notify", ["Fork cancelled", "Info"]]));
        }
        rows.push(Case {
            event: json!({"type":"session_before_fork","entryId":"rejected","position":"at"}),
            entries: json!([]),
            has_ui: true,
            decision: Value::Null,
            requests: json!(requests),
            ended: Ended::Failed("UI refused".into()),
            reject: Some(method),
        });
    }
    rows
}

/// Replacement decisions retain their exact prompts and cancellation outcomes.
#[test]
fn confirm_destructive_switch_decisions() -> Result<(), String> {
    native_cases("event session_before_switch", switch_cases())
}

/// Built-component replacement decisions match the controlled native run.
#[test]
fn confirm_destructive_component_switch_decisions() -> Result<(), String> {
    component_cases("event session_before_switch", switch_cases())
}

/// Forks need the exact affirmative choice unless no UI is available.
#[test]
fn confirm_destructive_fork_decisions() -> Result<(), String> {
    native_cases("event session_before_fork", fork_cases())
}

/// Built-component forks preserve prompts, cancellation and host rejections.
#[test]
fn confirm_destructive_component_fork_decisions() -> Result<(), String> {
    component_cases("event session_before_fork", fork_cases())
}

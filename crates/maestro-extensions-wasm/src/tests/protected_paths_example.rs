//! The shipped protected-paths hook exercised through registration and delivery.
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::documents::Answer;
use super::example_support::{registered, release_all, start};
use super::ui_host;
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture, NotificationType};

#[path = "../../../../examples/extensions/protected_paths.rs"]
mod protected_paths;

/// The same factory exported by the shipped component.
struct ProtectedPaths;

impl Extension for ProtectedPaths {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        protected_paths::factory(api)
    }
}

/// Recorded delivery rows, host-failure probes and the shared failure message.
fn corpus() -> Value {
    serde_json::from_str(include_str!("protected_paths_expected.json")).unwrap()
}

/// The answer a recorded row expects: its result document, or no verdict.
fn expected_answer(row: &Value) -> Answer {
    let result = (!row["result"].is_null()).then(|| row["result"].clone());
    Answer::returned(&row["event"], result)
}

/// The UI requests a recorded row expects, with the severity the host records.
fn expected_notifications(row: &Value) -> Vec<(String, Value)> {
    row["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|expected| {
            let message = expected[0].as_str().unwrap();
            assert_eq!(expected[1], "warning", "the recorded severity");
            let severity = ui_host::variant(NotificationType::Warning);
            (
                "notify".to_owned(),
                ui_host::notify(message, Some(severity.as_str())),
            )
        })
        .collect()
}

/// The UI requests recorded in `lines`, in time order.
fn notifications(lines: &[String]) -> Vec<(String, Value)> {
    ui_host::receipts(lines)
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect()
}

/// Sets the live UI flag and clears the injected failures on the supplied host.
fn make_available(host: &Controlled, available: bool) {
    let mut observed = host.observe();
    observed.catalog.data.has_ui = available;
    observed.catalog.data.error = None;
    observed.ui.control("{}");
}

/// The shared message every injected host failure reports.
fn host_failure() -> String {
    corpus()["host_errors"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|probe| probe["error"].as_str())
        .unwrap()
        .to_owned()
}

/// The recorded failure `target` injects, at the availability getter or the notification
/// route.
fn inject_failure(host: &Controlled, probe: &Value, failure: &str) {
    match probe["target"].as_str().unwrap() {
        "hasUI" => host.observe().catalog.data.error = Some(failure.to_owned()),
        _ => host
            .observe()
            .ui
            .control(&json!({"error": failure}).to_string()),
    }
}

/// The shipped hook registers one event callback and nothing else.
#[test]
fn protected_paths_registers_one_hook() -> Result<(), String> {
    super::block_on(async {
        let (host, ..) = registered::<ProtectedPaths>("event tool_call").await?;
        assert!(host.observed().tools.is_empty(), "no tool is registered");
        release_all(&host, &["event tool_call"]);
        Ok(())
    })
}

/// Every recorded tool call keeps its verdict, its untouched event and its notifications.
#[test]
fn protected_paths_matches_authored_paths() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<ProtectedPaths>().await?;
        let key = host.observed().callback("event tool_call")?;
        let rows = corpus();
        let rows = rows["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 87, "recorded tool calls");
        for row in rows {
            make_available(&host, row["hasUI"].as_bool().unwrap());
            let lent = host.observed().live;
            let start = host.observed().transcript.len();
            let outcome = exports
                .invoke_event(
                    key,
                    row["event"].to_string(),
                    Capabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                    },
                )
                .await?;
            let name = row["name"].as_str().unwrap();
            assert_eq!(
                Answer::of(super::guest_family::delivery(outcome))?,
                expected_answer(row),
                "{name}"
            );
            assert_eq!(
                notifications(&host.observed().transcript[start..]),
                expected_notifications(row),
                "{name}"
            );
            assert_eq!(host.observed().live, lent, "{name} lends nothing it keeps");
        }
        release_all(&host, &["event tool_call"]);
        Ok(())
    })
}

/// A reached host failure stays catchable, and an allowed call never reaches it.
#[test]
fn protected_paths_keeps_host_errors_catchable() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<ProtectedPaths>().await?;
        let key = host.observed().callback("event tool_call")?;
        let probes = corpus();
        let probes = probes["host_errors"].as_array().unwrap();
        assert_eq!(probes.len(), 6, "recorded host-failure probes");
        let failure = host_failure();
        for probe in probes {
            make_available(&host, true);
            inject_failure(&host, probe, &failure);
            let start = host.observed().transcript.len();
            let event = json!({
                "type": "tool_call",
                "toolCallId": "controlled-call",
                "toolName": probe["toolName"],
                "input": {"path": probe["path"]},
            });
            let outcome = exports
                .invoke_event(
                    key,
                    event.to_string(),
                    Capabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                    },
                )
                .await?;
            let lines = &host.observed().transcript[start..];
            let answer = Answer::of(super::guest_family::delivery(outcome))?;
            match probe["branch"].as_str().unwrap() {
                "error" => {
                    assert_eq!(
                        answer.ended,
                        super::documents::Ended::Failed(failure.clone())
                    );
                }
                _ => assert_eq!(answer.ended, super::documents::Ended::Returned(None)),
            }
            assert_eq!(
                notifications(lines),
                expected_notifications(probe),
                "{} {}",
                probe["target"],
                probe["path"]
            );
        }
        release_all(&host, &["event tool_call"]);
        Ok(())
    })
}

/// The built single-copy hook executes the same policy across the component interface.
#[test]
fn protected_paths_component_matches_the_hook() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "protected_paths")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|error| error.to_string())?;
    let key = harness
        .callback("event tool_call")
        .map_err(|error| error.to_string())?;
    let rows = corpus();
    for row in rows["rows"].as_array().unwrap() {
        harness.observed().catalog.data.has_ui = row["hasUI"].as_bool().unwrap();
        let start = harness.observed().transcript.len();
        let capabilities = super::host_family::guest::Capabilities {
            ctx: harness
                .ordinary("/controlled")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = harness.exports.clone();
        let event = row["event"].to_string();
        let outcome = super::block_on(harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), event, capabilities)
                .await
        }))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())??;
        let name = row["name"].as_str().unwrap();
        assert_eq!(
            Answer::of(super::host_family::delivery(outcome))?,
            expected_answer(row),
            "{name}"
        );
        assert_eq!(
            notifications(&harness.observed().transcript[start..]),
            expected_notifications(row),
            "{name}"
        );
    }
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

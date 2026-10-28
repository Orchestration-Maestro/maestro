//! The shipped model-status factory exercised through registration and invocation.
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::session_corpus::same;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/model_status.rs"]
mod model_status;

/// The same factory exported by the shipped component.
struct ModelStatus;

impl Extension for ModelStatus {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        model_status::factory(api)
    }
}

/// Selection events, effects and outcomes recorded from the reference run.
fn corpus() -> Value {
    serde_json::from_str(include_str!("model_status_expected.json")).unwrap()
}

/// The recorded rows, with `error` selecting the host-rejection rows.
fn rows(error: bool) -> Vec<Value> {
    corpus()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["outcome"]["error"].is_string() == error)
        .cloned()
        .collect()
}

/// The host spelling of a recorded severity; no other text is rewritten.
fn severity(text: &str) -> &str {
    match text {
        "info" => "Info",
        "warning" => "Warning",
        "error" => "Error",
        other => other,
    }
}

/// The host requests a recorded row reaches, in order, as the host receives them.
fn reached(row: &Value) -> Vec<(String, Value)> {
    row["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect[0] != "stdout")
        .map(|effect| {
            let method = effect[0].as_str().unwrap().to_owned();
            let operands = if method == "notify" {
                Value::Array(vec![
                    effect[1].clone(),
                    severity(effect[2].as_str().unwrap()).into(),
                ])
            } else {
                Value::Array(vec![effect[1].clone(), effect[2].clone()])
            };
            (method, operands)
        })
        .collect()
}

/// Checks that the handler left the row's event as it was.
fn same_event(answer: &Answer, row: &Value) {
    let name = row["name"].as_str().unwrap();
    match &answer.event {
        Some(event) => same(event, &row["event"]),
        None => panic!("{name}: the handler replaced the supplied event"),
    }
}

/// Checks that the row records a successful outcome, and that the handler left its event as it
/// was and returned no result.
fn unchanged(answer: &Answer, row: &Value) {
    same_event(answer, row);
    assert!(
        matches!(answer.ended, Ended::Returned(None)),
        "{}: the handler returned a result",
        row["name"]
    );
    assert_eq!(
        row["outcome"]["ok"].as_bool(),
        Some(true),
        "{}: recorded outcome",
        row["name"]
    );
}

/// Checks that the recorded transcript reached exactly the row's recorded host requests, in order.
fn assert_requests(transcript: &[String], row: &Value) {
    let seen: Vec<(String, Value)> = receipts(transcript)
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect();
    let expected = reached(row);
    assert_eq!(seen.len(), expected.len(), "{}: host requests", row["name"]);
    for (index, (method, operands)) in expected.iter().enumerate() {
        assert_eq!(&seen[index].0, method, "{}: request {index}", row["name"]);
        same(&seen[index].1, operands);
    }
}

/// The factory registers one selection hook and requests nothing while starting.
#[test]
fn model_status_registers_only_selection_hook() -> Result<(), String> {
    super::block_on(async {
        let (host, _exports) = super::example_support::start::<ModelStatus>().await?;
        let recorded: Vec<String> = corpus()["registrations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| format!("event {}", name.as_str().unwrap()))
            .collect();
        assert_eq!(recorded, ["event model_select"], "recorded registrations");
        assert!(
            receipts(&host.observed().transcript).is_empty(),
            "the factory requests no host UI while starting"
        );
        super::example_support::release_all(&host, &recorded);
        Ok(())
    })
}

/// Every recorded successful selection keeps its literal identifiers and its requests.
#[test]
fn model_status_selection_preserves_literal_identifiers() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<ModelStatus>().await?;
        let key = host.observed().callback("event model_select")?;
        let successes = rows(false);
        assert_eq!(successes.len(), 14, "recorded successes");
        for row in successes {
            let from = host.observed().transcript.len();
            let event = row["event"].to_string();
            let outcome = exports
                .invoke_event(
                    key,
                    event,
                    Capabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                    },
                )
                .await?;
            unchanged(&Answer::of(super::guest_family::delivery(outcome))?, &row);
            assert_requests(&host.observed().transcript[from..], &row);
        }
        release(key);
        assert_eq!(host.observed().live, 0, "the host lent nothing it kept");
        Ok(())
    })
}

/// The capacity of the captured stream: every recorded diagnostic byte, plus the one byte that
/// keeps the stream from reporting itself closed once the last of them is written.
fn stream_capacity() -> usize {
    let corpus = corpus();
    let recorded: usize = corpus["rows"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|row| row["effects"].as_array().unwrap())
        .filter(|effect| effect[0] == "stdout")
        .map(|effect| effect[1].as_str().unwrap().len())
        .sum();
    recorded + 1
}

/// The diagnostic a recorded row writes, as bytes.
fn diagnostic(row: &Value) -> Vec<u8> {
    row["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect[0] == "stdout")
        .flat_map(|effect| effect[1].as_str().unwrap().bytes())
        .collect()
}

/// Checks that every captured-stream checkpoint reads `start`, and returns how many the host
/// recorded.
fn checkpoints(transcript: &[String], start: usize) -> usize {
    let mut seen = 0;
    for line in transcript {
        let Some(length) = line.strip_prefix("captured ") else {
            continue;
        };
        assert_eq!(
            length.parse::<usize>().ok(),
            Some(start),
            "the diagnostic ran before a host request"
        );
        seen += 1;
    }
    seen
}

/// Invokes the built component's registered callback for a recorded row.
fn invoke_on_component(
    harness: &mut super::host::Harness,
    key: u32,
    row: &Value,
) -> Result<Answer, String> {
    let capabilities = super::host_family::guest::Capabilities {
        ctx: harness
            .ordinary("/controlled")
            .map_err(|error| error.to_string())?,
        signal: None,
    };
    let exports = harness.exports.clone();
    let event = row["event"].to_string();
    let called = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(accessor, super::host::borrow(key), event, capabilities)
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    Answer::of(super::host_family::delivery(called?))
}

/// Every recorded successful selection reaches the built component's real callback.
#[test]
fn model_status_component_runs_selection_hook() -> Result<(), String> {
    let path = super::build::example_component("model_status")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let (mut harness, stdout) = super::block_on(super::host::Harness::from_compiled_capturing(
        &engine,
        &component,
        stream_capacity(),
    ))
    .map_err(|error| error.to_string())?;
    let key = harness
        .callback("event model_select")
        .map_err(|error| error.to_string())?;
    for row in rows(false) {
        let from = harness.observed().transcript.len();
        let start = stdout.contents().len();
        let answer = invoke_on_component(&mut harness, key, &row)?;
        unchanged(&answer, &row);
        assert_requests(&harness.observed().transcript[from..], &row);
        let seen = checkpoints(&harness.observed().transcript[from..], start);
        let reached = reached(&row);
        assert_eq!(seen, reached.len(), "{}: stream checkpoints", row["name"]);
        let written = &stdout.contents()[start..];
        assert_eq!(
            written,
            diagnostic(&row).as_slice(),
            "{}: diagnostics",
            row["name"]
        );
    }
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0, "the host lent nothing it kept");
    Ok(())
}

/// Runs both recorded rejections and a later control through the in-process adapter.
fn rejections_in_process() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<ModelStatus>().await?;
        let key = host.observed().callback("event model_select")?;
        for row in rows(true) {
            let reject = row["reject"].as_str().unwrap();
            let error = row["outcome"]["error"].as_str().unwrap().to_owned();
            host.observe()
                .ui
                .control(&json!({ "error": reject }).to_string());
            let from = host.observed().transcript.len();
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
            let answer = Answer::of(super::guest_family::delivery(outcome))?;
            same_event(&answer, &row);
            assert_eq!(answer.ended, Ended::Failed(error), "{}", row["name"]);
            assert_requests(&host.observed().transcript[from..], &row);
        }
        let control = &rows(false)[0];
        let outcome = exports
            .invoke_event(
                key,
                control["event"].to_string(),
                Capabilities {
                    ctx: host.context("/controlled"),
                    signal: None,
                },
            )
            .await?;
        unchanged(
            &Answer::of(super::guest_family::delivery(outcome))?,
            control,
        );
        release(key);
        assert_eq!(host.observed().live, 0, "the host lent nothing it kept");
        Ok(())
    })
}

/// Runs both recorded rejections and a later control through the built component.
fn rejections_on_component() -> Result<(), String> {
    let path = super::build::example_component("model_status")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let (mut harness, stdout) = super::block_on(super::host::Harness::from_compiled_capturing(
        &engine,
        &component,
        stream_capacity(),
    ))
    .map_err(|error| error.to_string())?;
    let key = harness
        .callback("event model_select")
        .map_err(|error| error.to_string())?;
    for row in rows(true) {
        let reject = row["reject"].as_str().unwrap();
        let error = row["outcome"]["error"].as_str().unwrap().to_owned();
        harness
            .observed()
            .ui
            .control(&json!({ "error": reject }).to_string());
        let from = harness.observed().transcript.len();
        let start = stdout.contents().len();
        let answer = invoke_on_component(&mut harness, key, &row)?;
        same_event(&answer, &row);
        assert_eq!(answer.ended, Ended::Failed(error), "{}", row["name"]);
        let seen = checkpoints(&harness.observed().transcript[from..], start);
        assert_eq!(seen, 1, "{}: stream checkpoints", row["name"]);
        assert_eq!(
            stdout.contents().len(),
            start,
            "{}: diagnostics after a rejection",
            row["name"]
        );
        assert_requests(&harness.observed().transcript[from..], &row);
    }
    let control = &rows(false)[0];
    let start = stdout.contents().len();
    let answer = invoke_on_component(&mut harness, key, control)?;
    unchanged(&answer, control);
    assert_eq!(
        &stdout.contents()[start..],
        diagnostic(control).as_slice(),
        "the control still writes its diagnostic"
    );
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0, "the host lent nothing it kept");
    Ok(())
}

/// A rejected host request stops every later effect of the selection on both adapters.
#[test]
fn model_status_host_rejections_stop_later_effects() -> Result<(), String> {
    rejections_in_process()?;
    rejections_on_component()
}

//! The shipped model-status factory exercised through registration and invocation.
use serde_json::Value;

use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use super::session_corpus::same;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, Exports, release};
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

/// Checks that the handler left the row's event as it was and returned no result.
fn unchanged(answer: &Answer, row: &Value) {
    let name = row["name"].as_str().unwrap();
    match &answer.event {
        Some(event) => same(event, &row["event"]),
        None => panic!("{name}: the handler replaced the supplied event"),
    }
    assert!(
        matches!(answer.ended, Ended::Returned(None)),
        "{name}: the handler returned a result"
    );
}

/// Checks that the handler reached exactly the row's recorded host requests, in order.
fn assert_requests(host: &Controlled, from: usize, row: &Value) {
    let seen: Vec<(String, Value)> = receipts(&host.observed().transcript[from..])
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
        let host = Controlled::default();
        let exports = Exports::new(host.clone());
        exports.start::<ModelStatus>().await?;
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
            assert_requests(&host, from, &row);
        }
        release(key);
        assert_eq!(host.observed().live, 0, "the host lent nothing it kept");
        Ok(())
    })
}

//! The shipped working-message startup example exercised through both hosts.
use serde::Deserialize;
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use super::session_corpus::same;
use super::ui_host::receipts;
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/working_message_test.rs"]
mod working_message_test;

/// The same factory exported by the shipped component.
struct WorkingMessage;

impl Extension for WorkingMessage {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        working_message_test::factory(api)
    }
}

/// The cases these tests consume.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Corpus {
    /// Every session-start reason and a repeated callback.
    requests: Vec<Row>,
}

/// One case: its name and the callback invocations it covers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Name the case reports in a failed assertion.
    name: String,
    /// The callback invocations it covers, in order.
    steps: Vec<Step>,
}

/// One callback invocation and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Event delivered to the registered callback.
    event: Value,
    /// Identity of the UI binding the host resolves for the acquisition.
    binding: usize,
    /// Method whose request the host refuses.
    #[serde(default)]
    reject: Option<String>,
    /// Message the host refuses with.
    #[serde(default)]
    error: Option<String>,
    /// How the callback ends.
    outcome: Outcome,
    /// The requests the host records, in order.
    requests: Vec<Request>,
}

/// One request the host records.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Method the host reached.
    method: String,
    /// Operands the host received.
    operands: Value,
}

/// How a recorded callback ends.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    /// It returns no event result.
    Returned,
    /// It returns the host's refusal.
    Rejected,
}

/// The recorded cases.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("working_message_expected.json")).unwrap()
}

/// The host state one recorded step supplies.
fn control(step: &Step) -> String {
    json!({
        "binding": step.binding,
        "error": step.error,
        "method": step.reject,
    })
    .to_string()
}

/// The answer the recorded step requires.
fn expected(step: &Step) -> Answer {
    match step.outcome {
        Outcome::Returned => Answer::returned(&step.event, None),
        Outcome::Rejected => Answer {
            event: Some(step.event.clone()),
            ended: Ended::Failed(step.error.clone().unwrap()),
        },
    }
}

/// Asserts one invocation's answer, its ordered requests and the owner they reached.
fn assert_step(name: &str, step: &Step, answer: &Answer, lines: &[String]) {
    assert_eq!(*answer, expected(step), "{name}: the callback's answer");
    let seen = receipts(lines);
    assert_eq!(seen.len(), step.requests.len(), "{name}: recorded requests");
    for (seen, request) in seen.iter().zip(&step.requests) {
        assert_eq!(seen.method, request.method, "{name}: request method");
        assert_eq!(seen.key, step.binding, "{name}: current UI owner");
        same(&seen.operands, &request.operands);
    }
}

/// Runs `rows` through the controlled adapter on one registration.
fn controlled_cases(rows: &[Row]) -> Result<(), String> {
    let (host, exports) = super::block_on(start::<WorkingMessage>())?;
    let key = host.observed().callback("event session_start")?;
    for row in rows {
        for step in &row.steps {
            host.observe().ui.control(&control(step));
            let mark = host.observed().transcript.len();
            let outcome = super::block_on(exports.invoke_event(
                key,
                step.event.to_string(),
                Capabilities {
                    ctx: host.context("/controlled"),
                    signal: None,
                },
            ))?;
            let lines = host.observed().transcript[mark..].to_vec();
            assert_step(
                &row.name,
                step,
                &Answer::of(super::guest_family::delivery(outcome))?,
                &lines,
            );
        }
    }
    release_all(&host, &["event session_start"]);
    Ok(())
}

/// The controlled adapter records one startup registration and no request before it runs.
#[test]
fn working_message_registers_only_session_start() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<WorkingMessage>().await?;
        assert!(
            receipts(&host.observed().transcript).is_empty(),
            "a request before the callback ran"
        );
        drop(exports);
        release_all(&host, &["event session_start"]);
        Ok(())
    })
}

/// Every session-start reason, and a repeated callback, request the same text and frame.
#[test]
fn working_message_requests_keep_style_and_order() -> Result<(), String> {
    controlled_cases(&corpus().requests)
}

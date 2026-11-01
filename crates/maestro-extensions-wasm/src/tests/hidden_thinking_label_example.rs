//! The shipped hidden-thinking-label factory exercised through registration and invocation.
use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use super::example_support::start;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/hidden_thinking_label.rs"]
mod hidden_thinking_label;

/// The same factory exported by the shipped component.
struct HiddenThinkingLabel;

impl Extension for HiddenThinkingLabel {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        hidden_thinking_label::factory(api)
    }
}

/// One registration the recorded metadata describes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    /// The kind of registration.
    operation: String,
    /// Literal registration name.
    name: String,
    /// Supplied command description.
    #[serde(default)]
    description: Option<String>,
    /// Whether argument completions were supplied.
    #[serde(default)]
    completions: Option<bool>,
}

/// One recorded sequence of operations against a fresh factory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Recorded name of the sequence.
    name: String,
    /// The operations and their expected outcomes.
    steps: Vec<Step>,
}

/// One operation and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// The operation supplied to a registered callback.
    input: Input,
    /// The requests and error it must produce.
    expected: Expected,
}

/// The operation supplied to a registered callback.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Operation name: `session_start` or `command`.
    operation: String,
    /// Command argument text.
    #[serde(default)]
    args: Option<String>,
    /// Host rejection armed for one method.
    #[serde(default)]
    reject: Option<Rejection>,
}

/// A host rejection armed for one method.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejection {
    /// Method the rejection applies to.
    method: String,
    /// Message the host rejects that method with.
    message: String,
}

/// The host requests a step must produce, and its error when it fails.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Host requests in time order.
    calls: Vec<Call>,
    /// The error the step ends with.
    error: Option<String>,
}

/// One host request: the method reached and the operands it received.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    /// Method the host reached.
    method: String,
    /// Operands the host received.
    operands: Value,
}

/// The recorded metadata and sequences.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Registrations in the order the host receives them.
    metadata: Vec<Registration>,
    /// The recorded sequences.
    cases: Vec<Case>,
}

/// The recorded corpus the tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("hidden_thinking_label_expected.json")).unwrap()
}

/// The session-start document every event step delivers.
fn session_start() -> Value {
    json!({ "type": "session_start", "reason": "new" })
}

/// The callback keys one factory instance registered.
#[derive(Clone, Copy)]
struct Keys {
    /// The `session_start` event handler.
    event: u32,
    /// The `thinking-label` command.
    command: u32,
}

/// What one recorded step produced.
struct StepOutcome {
    /// Host requests in time order.
    calls: Vec<(String, Value)>,
    /// The error the step ended with.
    error: Option<String>,
    /// The answer of an event step.
    answer: Option<Answer>,
}

/// The error an event answer ended with.
fn ended_error(answer: &Answer) -> Option<String> {
    match &answer.ended {
        Ended::Failed(message) => Some(message.clone()),
        Ended::Returned(_) => None,
    }
}

/// Delivers one recorded step to a controlled factory and reads what the host observed.
async fn controlled_step(
    host: &Controlled,
    exports: &Exports<Controlled>,
    keys: Keys,
    step: &Step,
) -> Result<StepOutcome, String> {
    if let Some(rejection) = &step.input.reject {
        host.observe()
            .ui
            .reject(&rejection.method, &rejection.message);
    }
    let from = host.observed().transcript.len();
    let (error, answer) = match step.input.operation.as_str() {
        "session_start" => {
            let event = session_start();
            let outcome = exports
                .invoke_event(
                    keys.event,
                    event.to_string(),
                    Capabilities {
                        ctx: host.context("/work"),
                        signal: None,
                    },
                )
                .await?;
            let answer = Answer::of(super::guest_family::delivery(outcome))?;
            (ended_error(&answer), Some(answer))
        }
        "command" => {
            let args = step.input.args.clone().unwrap_or_default();
            let outcome = exports
                .invoke_command(keys.command, args, host.command_context("/work"))
                .await;
            (outcome.err(), None)
        }
        other => return Err(format!("unknown recorded operation {other}")),
    };
    let calls = receipts(&host.observed().transcript[from..])
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect();
    Ok(StepOutcome {
        calls,
        error,
        answer,
    })
}

/// Asserts that one step produced exactly its recorded requests and outcome.
fn checked(case: &Case, index: usize, step: &Step, outcome: StepOutcome) {
    let calls: Vec<(String, Value)> = step
        .expected
        .calls
        .iter()
        .map(|call| (call.method.clone(), call.operands.clone()))
        .collect();
    assert_eq!(outcome.calls, calls, "{} step {index} requests", case.name);
    assert_eq!(
        outcome.error,
        step.expected.error.clone(),
        "{} step {index} error",
        case.name
    );
    if let Some(answer) = outcome.answer {
        let ended = match &step.expected.error {
            Some(message) => Ended::Failed(message.clone()),
            None => Ended::Returned(None),
        };
        assert_eq!(
            answer,
            Answer {
                event: Some(session_start()),
                ended
            },
            "{} step {index} event",
            case.name
        );
    }
}

/// Starts a fresh factory and returns its host, exports and callback keys.
async fn instance() -> Result<(Controlled, Exports<Controlled>, Keys), String> {
    let (host, exports) = start::<HiddenThinkingLabel>().await?;
    let keys = Keys {
        event: host.observed().callback("event session_start")?,
        command: host.observed().callback("command thinking-label")?,
    };
    Ok((host, exports, keys))
}

/// The recorded case named `name`.
fn case_of(name: &str) -> Case {
    corpus()
        .cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap()
}

/// Runs every step of one recorded case against a fresh factory.
async fn checked_case(name: &str) -> Result<(), String> {
    let case = case_of(name);
    let (host, exports, keys) = instance().await?;
    for (index, step) in case.steps.iter().enumerate() {
        let outcome = controlled_step(&host, &exports, keys, step).await?;
        checked(&case, index, step, outcome);
    }
    super::example_support::release_all(&host, &["event session_start", "command thinking-label"]);
    Ok(())
}

/// Register only the session hook and the command, then apply the current label at each start.
#[test]
fn hidden_thinking_label_registers_the_session_hook_and_command() -> Result<(), String> {
    super::block_on(async {
        let (host, exports, keys) = instance().await?;
        let corpus = corpus();
        let command = &corpus.metadata[1];
        assert_eq!(command.operation, "command");
        assert_eq!(
            host.observed().registrations.commands,
            [(
                command.name.clone(),
                command.description.clone(),
                command.completions.unwrap_or(false)
            )],
            "the command metadata the host received"
        );
        let order: Vec<String> = corpus
            .metadata
            .iter()
            .map(|record| format!("register {} {}", record.operation, record.name))
            .collect();
        assert_eq!(
            host.observed().transcript,
            order,
            "the registrations in the order the host received them"
        );
        assert!(
            receipts(&host.observed().transcript).is_empty(),
            "registration requests no UI"
        );
        let case = case_of("startup_and_repeat");
        for (index, step) in case.steps.iter().enumerate() {
            let outcome = controlled_step(&host, &exports, keys, step).await?;
            checked(&case, index, step, outcome);
        }
        super::example_support::release_all(
            &host,
            &["event session_start", "command thinking-label"],
        );
        Ok(())
    })
}

/// Every recorded custom label, reset and replay sequence produces its exact requests.
#[test]
fn hidden_thinking_label_commands_update_and_reset_shared_state() -> Result<(), String> {
    super::block_on(async {
        for name in [
            "literal_custom_label",
            "empty_resets_then_start_reapplies_example_default",
            "ascii_whitespace_resets",
            "unicode_whitespace_resets",
            "unicode_whitespace_trims_custom",
            "interior_bytes_remain_literal",
            "nonwhitespace_neighbour_remains",
            "default_text_is_still_a_custom_set",
            "successive_custom_and_reset",
        ] {
            checked_case(name).await?;
        }
        Ok(())
    })
}

/// Every rejected request keeps the retained label and the order of the requests.
#[test]
fn hidden_thinking_label_failures_keep_state_and_order() -> Result<(), String> {
    super::block_on(async {
        for name in [
            "start_rejection_is_returned",
            "custom_set_rejection_keeps_new_label",
            "reset_set_rejection_keeps_default",
            "custom_notify_rejection_keeps_new_label",
            "reset_notify_rejection_keeps_default",
        ] {
            checked_case(name).await?;
        }
        Ok(())
    })
}

/// A released factory leaves nothing lent, and a fresh one starts from the example default.
#[test]
fn hidden_thinking_label_instances_start_fresh_and_release() -> Result<(), String> {
    super::block_on(async {
        let case = case_of("literal_custom_label");
        let (host, exports, keys) = instance().await?;
        let outcome = controlled_step(&host, &exports, keys, &case.steps[1]).await?;
        checked(&case, 1, &case.steps[1], outcome);
        super::example_support::release_all(
            &host,
            &["event session_start", "command thinking-label"],
        );
        let startup = case_of("startup_and_repeat");
        let (fresh, exports, keys) = instance().await?;
        let outcome = controlled_step(&fresh, &exports, keys, &startup.steps[0]).await?;
        checked(&startup, 0, &startup.steps[0], outcome);
        super::example_support::release_all(
            &fresh,
            &["event session_start", "command thinking-label"],
        );
        Ok(())
    })
}

/// Delivers one recorded step to the built component and reads what the host observed.
fn component_step(
    harness: &mut super::host::Harness,
    keys: Keys,
    step: &Step,
) -> Result<StepOutcome, String> {
    if let Some(rejection) = &step.input.reject {
        harness
            .observed()
            .ui
            .reject(&rejection.method, &rejection.message);
    }
    let from = harness.observed().transcript.len();
    let (error, answer) = match step.input.operation.as_str() {
        "session_start" => {
            let event = session_start();
            let capabilities = super::host_family::guest::Capabilities {
                ctx: harness
                    .ordinary("/work")
                    .map_err(|error| error.to_string())?,
                signal: None,
            };
            let exports = harness.exports.clone();
            let called = super::block_on(harness.store.run_concurrent(async |accessor| {
                exports
                    .call_invoke_event(
                        accessor,
                        super::host::borrow(keys.event),
                        event.to_string(),
                        capabilities,
                    )
                    .await
            }))
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
            let answer = Answer::of(super::host_family::delivery(called?))?;
            (ended_error(&answer), Some(answer))
        }
        "command" => {
            let args = step.input.args.clone().unwrap_or_default();
            let outcome = super::example_support::invoke_command_on_component(
                harness,
                keys.command,
                &args,
                false,
            );
            (outcome.err(), None)
        }
        other => return Err(format!("unknown recorded operation {other}")),
    };
    let calls = receipts(&harness.observed().transcript[from..])
        .into_iter()
        .map(|receipt| (receipt.method, receipt.operands))
        .collect();
    Ok(StepOutcome {
        calls,
        error,
        answer,
    })
}

/// Runs every recorded sequence against a fresh guest of the built single-copy example.
#[test]
fn hidden_thinking_label_component_runs_registered_callbacks() -> Result<(), String> {
    let path = super::build::example_component("hidden_thinking_label")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let metadata = corpus().metadata;
    for case in corpus().cases {
        let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
            .map_err(|error| error.to_string())?;
        assert_eq!(
            harness.observed().registrations.commands,
            [(
                metadata[1].name.clone(),
                metadata[1].description.clone(),
                metadata[1].completions.unwrap_or(false)
            )],
            "{} command metadata",
            case.name
        );
        let keys = Keys {
            event: harness
                .callback("event session_start")
                .map_err(|error| error.to_string())?,
            command: harness
                .callback("command thinking-label")
                .map_err(|error| error.to_string())?,
        };
        for (index, step) in case.steps.iter().enumerate() {
            let outcome = component_step(&mut harness, keys, step)?;
            checked(&case, index, step, outcome);
        }
        for key in [keys.event, keys.command] {
            super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
        }
        assert_eq!(harness.observed().live, 0, "{} lent nothing", case.name);
    }
    Ok(())
}

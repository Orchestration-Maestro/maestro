//! The shipped system-prompt status factory exercised through registration and invocation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use super::example_support::{release_all, start};
use super::host::Harness;
use super::observed::Observed;
use super::ui_host::receipts;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/system_prompt_header.rs"]
mod system_prompt_header;

/// The registrations the factory makes, in the order it makes them.
const REGISTRATIONS: [&str; 2] = ["event agent_start", "event session_shutdown"];

/// The same factory the shipped component exports.
struct SystemPromptHeader;

impl Extension for SystemPromptHeader {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        system_prompt_header::factory(api)
    }
}

/// One recorded invocation of a registered handler.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Event the host delivers.
    event: String,
    /// Prompt the host reports while the handler runs.
    prompt: String,
    /// Rejection the prompt getter reports instead of a prompt.
    #[serde(rename = "getError")]
    get_error: Option<String>,
    /// Rejection the next status request reports.
    #[serde(rename = "setError")]
    set_error: Option<String>,
    /// Operands of each status request the host must record.
    requests: Vec<(String, Option<String>)>,
    /// Message the handler must return instead of completing.
    error: Option<String>,
}

/// One recorded case: the test that owns it and its invocations on one registration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Case name the assertions report.
    name: String,
    /// Test group that owns the case.
    group: String,
    /// Recorded invocations in order.
    steps: Vec<Step>,
}

/// The recorded cases.
fn corpus() -> Vec<Case> {
    serde_json::from_str(include_str!("system_prompt_header_expected.json")).unwrap()
}

/// The keys of the factory's two registrations, in their recorded order.
fn registered(host: &Controlled) -> Result<[u32; 2], String> {
    Ok([
        host.observed().callback(REGISTRATIONS[0])?,
        host.observed().callback(REGISTRATIONS[1])?,
    ])
}

/// The callback key of the registration that receives `event`.
fn key_for(keys: [u32; 2], event: &str) -> Result<u32, String> {
    match event {
        "agent_start" => Ok(keys[0]),
        "session_shutdown" => Ok(keys[1]),
        other => Err(format!("recorded event {other} is not registered")),
    }
}

/// The answer a recorded step requires: `event` as it was delivered and its recorded ending.
fn expected_answer(event: &Value, step: &Step) -> Answer {
    Answer {
        event: Some(event.clone()),
        ended: step
            .error
            .as_ref()
            .map_or(Ended::Returned(None), |message| {
                Ended::Failed(message.clone())
            }),
    }
}

/// Compares the status requests the host recorded in `lines` with the step's.
fn assert_requests(lines: &[String], step: &Step, case: &str) {
    let recorded = receipts(lines);
    for receipt in &recorded {
        assert_eq!(receipt.method, "setStatus", "{case} {}", step.event);
    }
    let operands: Vec<Value> = recorded
        .into_iter()
        .map(|receipt| receipt.operands)
        .collect();
    assert_eq!(
        json!(operands),
        json!(step.requests),
        "{case} {}",
        step.event
    );
}

/// Applies the host state one recorded step requires before its invocation, in process or
/// on the built component's host.
fn supply(observed: &mut Observed, step: &Step) {
    let state = json!({"state": {"prompt": step.prompt, "error": step.get_error}});
    observed.catalog.control(&state.to_string());
    let rejection = json!({"error": step.set_error});
    observed.ui.control(&rejection.to_string());
}

/// Invokes one recorded step through the in-process adapter and asserts its answer.
async fn run_step(
    host: &Controlled,
    exports: &Exports<Controlled>,
    key: u32,
    step: &Step,
    case: &str,
) -> Result<(), String> {
    let from = host.observed().transcript.len();
    let event = super::documents::document(&step.event);
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
    let answer = Answer::of(super::guest_family::delivery(outcome))?;
    assert_eq!(
        answer,
        expected_answer(&event, step),
        "{case} {}",
        step.event
    );
    assert_requests(&host.observed().transcript[from..], step, case);
    Ok(())
}

/// Runs one recorded case on a fresh registration through the in-process adapter.
async fn run_case(case: &Case) -> Result<(), String> {
    let (host, exports) = start::<SystemPromptHeader>().await?;
    let keys = registered(&host)?;
    for step in &case.steps {
        supply(&mut host.observe(), step);
        run_step(
            &host,
            &exports,
            key_for(keys, &step.event)?,
            step,
            &case.name,
        )
        .await?;
    }
    release_all(&host, &REGISTRATIONS);
    Ok(())
}

/// Runs every recorded case of `group` through the in-process adapter.
fn run_group(group: &str) -> Result<(), String> {
    super::block_on(async {
        for case in corpus().into_iter().filter(|case| case.group == group) {
            run_case(&case).await?;
        }
        Ok(())
    })
}

/// Invokes one recorded step on the built component and asserts its answer.
fn component_step(harness: &mut Harness, key: u32, step: &Step, case: &str) -> Result<(), String> {
    let from = harness.observed().transcript.len();
    let event = super::documents::document(&step.event);
    let capabilities = super::host_family::guest::Capabilities {
        ctx: harness
            .ordinary("/controlled")
            .map_err(|error| error.to_string())?,
        signal: None,
    };
    let exports = harness.exports.clone();
    let run = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(
                accessor,
                super::host::borrow(key),
                event.to_string(),
                capabilities,
            )
            .await
    }))
    .map_err(|error| error.to_string())?;
    let outcome = run.map_err(|error| error.to_string())??;
    let answer = Answer::of(super::host_family::delivery(outcome))?;
    assert_eq!(
        answer,
        expected_answer(&event, step),
        "{case} {}",
        step.event
    );
    assert_requests(&harness.observed().transcript[from..], step, case);
    Ok(())
}

/// Loading registers the two lifecycle handlers in order and requests no status.
#[test]
fn system_prompt_status_registers_lifecycle_handlers() -> Result<(), String> {
    super::block_on(async {
        let (host, _exports) = start::<SystemPromptHeader>().await?;
        assert!(
            receipts(&host.observed().transcript).is_empty(),
            "loading requests no status"
        );
        registered(&host)?;
        release_all(&host, &REGISTRATIONS);
        Ok(())
    })
}

/// Every recorded prompt gives its exact UTF-16 length, including one changed after loading.
#[test]
fn system_prompt_status_reads_current_utf16_prompt() -> Result<(), String> {
    run_group("utf16")
}

/// Shutdown clears the status key on each lifecycle path, including before any start.
#[test]
fn system_prompt_status_clears_on_shutdown() -> Result<(), String> {
    run_group("lifecycle")
}

/// Host rejections reach the caller unchanged and the next invocation still works.
#[test]
fn system_prompt_status_propagates_host_errors() -> Result<(), String> {
    run_group("errors")
}

/// The built single-copy example produces every recorded answer across the component interface.
#[test]
fn system_prompt_status_component_matches_lifecycle() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "system_prompt_header")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|error| error.to_string())?;
    let registrations: Vec<&str> = harness
        .observed()
        .transcript
        .iter()
        .filter_map(|line| line.strip_prefix("register "))
        .collect();
    assert_eq!(
        registrations, REGISTRATIONS,
        "the registrations in the order the host received them"
    );
    assert!(
        receipts(&harness.observed().transcript).is_empty(),
        "loading requests no status"
    );
    let keys = [
        harness
            .callback(REGISTRATIONS[0])
            .map_err(|error| error.to_string())?,
        harness
            .callback(REGISTRATIONS[1])
            .map_err(|error| error.to_string())?,
    ];
    for case in corpus() {
        for step in &case.steps {
            supply(harness.observed(), step);
            component_step(&mut harness, key_for(keys, &step.event)?, step, &case.name)?;
        }
    }
    for key in keys {
        super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    }
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

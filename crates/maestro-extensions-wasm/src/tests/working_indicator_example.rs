//! The shipped indicator command exercised through its registered callbacks.
use serde::Deserialize;
use serde_json::{Value, json};

use super::documents::Answer;
use super::example_support::{release_all, start};
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/working_indicator.rs"]
mod working_indicator;

/// The factory exported by the shipped component.
struct WorkingIndicator;

impl Extension for WorkingIndicator {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        working_indicator::factory(api)
    }
}

/// One invocation and the literal requests it must make.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// An event delivered to the startup handler, or a command when absent.
    event: Option<Value>,
    /// The command's arguments, when this is a command invocation.
    args: Option<String>,
    /// The UI method the host refuses, when one is refused.
    reject: Option<String>,
    /// The requests made to the current UI, in order.
    requests: Vec<Request>,
    /// The requests made to its theme, in order.
    #[serde(default)]
    themes: Vec<Request>,
}

/// A method and its caller-visible operands.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Request {
    /// The requested operation.
    method: String,
    /// The supplied values.
    operands: Value,
}

/// Recorded invocation groups, each starting with a fresh factory.
fn corpus() -> std::collections::BTreeMap<String, Vec<Step>> {
    serde_json::from_str(include_str!("working_indicator_expected.json")).unwrap()
}

impl Step {
    /// Sets the current UI binding and an optional one-shot refusal.
    fn control(&self) -> String {
        json!({"binding": 41, "method": self.reject, "error": self.reject.as_ref().map(|_| "refused")}).to_string()
    }

    /// Asserts the callback outcome and every request, including styled status text.
    fn assert(&self, outcome: Result<(), String>, lines: &[String]) {
        assert_eq!(
            outcome.err(),
            self.reject.as_ref().map(|_| "refused".to_owned())
        );
        let ui = super::ui_host::receipts(lines);
        assert!(ui.iter().all(|request| request.key == 41));
        let ui: Vec<_> = ui
            .into_iter()
            .map(|request| Request {
                method: request.method,
                operands: request.operands,
            })
            .collect();
        assert_eq!(ui, self.requests);
        let themes: Vec<_> = super::theme_host::receipts(lines)
            .into_iter()
            .map(|request| {
                assert_eq!(request.key, "dark");
                Request {
                    method: request.method,
                    operands: request.operands,
                }
            })
            .collect();
        assert_eq!(themes, self.themes);
    }
}

/// Supplies dim answers to this host without adding rows to the shared style corpus.
fn supply_theme(observed: &mut super::observed::Observed) {
    for description in [
        "static dot",
        "hidden",
        "custom pulse",
        "custom spinner",
        "Maestro default spinner",
    ] {
        let text = format!("Indicator: {description}");
        observed
            .themes
            .theme("dark")
            .replies
            .push(super::theme_host::Reply {
                method: "fg".into(),
                operands: json!(["dim", text]),
                answer: json!({"ok": format!("\x1b[2m{text}\x1b[22m")}),
            });
    }
}

/// Runs a group on the in-process adapter without reaching inside the example.
fn controlled(group: &str) -> Result<(), String> {
    let (host, exports) = super::block_on(start::<WorkingIndicator>())?;
    let command = host.observed().callback("command working-indicator")?;
    let event = host.observed().callback("event session_start")?;
    assert!(super::ui_host::receipts(&host.observed().transcript).is_empty());
    assert_eq!(
        host.observed().registrations.commands,
        [(
            "working-indicator".into(),
            Some(
                "Set the streaming working indicator: dot, pulse, none, spinner, or reset.".into()
            ),
            false,
        )]
    );
    supply_theme(&mut host.observe());
    for step in &corpus()[group] {
        host.observe().ui.control(&step.control());
        let mark = host.observed().transcript.len();
        let outcome = if let Some(input) = &step.event {
            super::block_on(exports.invoke_event(
                event,
                input.to_string(),
                Capabilities {
                    ctx: host.context("/controlled"),
                    signal: None,
                },
            ))
            .and_then(|answer| {
                assert_eq!(
                    Answer::of(super::guest_family::delivery(answer))?,
                    Answer::returned(input, None)
                );
                Ok(())
            })
        } else {
            super::block_on(exports.invoke_command(
                command,
                step.args.clone().unwrap(),
                host.command_context("/controlled"),
            ))
        };
        step.assert(outcome, &host.observed().transcript[mark..]);
    }
    release_all(&host, &["event session_start", "command working-indicator"]);
    Ok(())
}

/// Startup applies the initially selected custom spinner and its dim status.
#[test]
fn working_indicator_startup_applies_custom_spinner() -> Result<(), String> {
    controlled("startup")
}

/// Every named mode keeps its frames, interval, description and themed status.
#[test]
fn working_indicator_modes_keep_frames_intervals_and_status() -> Result<(), String> {
    controlled("modes")
}

/// Queries and invalid arguments leave the selected mode unchanged.
#[test]
fn working_indicator_queries_and_invalid_input_leave_selection_unchanged() -> Result<(), String> {
    controlled("queries")
}

/// Host refusals keep the selection and stop later UI requests.
#[test]
fn working_indicator_refusals_preserve_selection_and_request_order() -> Result<(), String> {
    controlled("refusals")
}

/// Delivers a startup event through the built component's public callback.
fn component_event(harness: &mut super::host::Harness, event: &Value) -> Result<(), String> {
    let key = harness
        .callback("event session_start")
        .map_err(|e| e.to_string())?;
    let capabilities = super::host_family::guest::Capabilities {
        ctx: harness.ordinary("/controlled").map_err(|e| e.to_string())?,
        signal: None,
    };
    let exports = harness.exports.clone();
    let input = event.to_string();
    let result = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_event(accessor, super::host::borrow(key), input, capabilities)
            .await
    }))
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())??;
    assert_eq!(
        Answer::of(super::host_family::delivery(result))?,
        Answer::returned(event, None)
    );
    Ok(())
}

/// Every recorded transition also runs against the single shipped component.
#[test]
fn working_indicator_component_matches_registered_callbacks() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "working_indicator")?;
    let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
    for steps in corpus().values() {
        let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
            .map_err(|e| e.to_string())?;
        let command = harness
            .callback("command working-indicator")
            .map_err(|e| e.to_string())?;
        supply_theme(harness.observed());
        for step in steps {
            harness.observed().ui.control(&step.control());
            let mark = harness.observed().transcript.len();
            let outcome = if let Some(event) = &step.event {
                component_event(&mut harness, event)
            } else {
                super::example_support::invoke_command_on_component(
                    &mut harness,
                    command,
                    step.args.as_deref().unwrap(),
                    false,
                )
            };
            step.assert(outcome, &harness.observed().transcript[mark..]);
        }
        for name in ["event session_start", "command working-indicator"] {
            let key = harness.callback(name).map_err(|e| e.to_string())?;
            super::block_on(harness.release(key)).map_err(|e| e.to_string())?;
        }
        assert_eq!(harness.observed().live, 0);
    }
    Ok(())
}

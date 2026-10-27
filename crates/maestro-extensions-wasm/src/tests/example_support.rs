//! Support shared by the tests of the shipped extension examples.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::Value;

use super::controlled::Controlled;
use super::host::Harness;
use super::host_family::guest::{ToolCapabilities, ToolInvocation};
use super::observed::Observed;
use crate::component_adapter::{Exports, release};
use crate::loader::Extension;

/// Starts a shipped factory on a fresh controlled host.
pub async fn start<E: Extension>() -> Result<(Controlled, Exports<Controlled>), String> {
    let host = Controlled::default();
    let exports = Exports::new(host.clone());
    exports.start::<E>().await?;
    Ok((host, exports))
}

/// Starts a shipped factory on a fresh controlled host and returns the key of the tool it
/// registered under `tool`.
pub async fn registered<E: Extension>(
    tool: &str,
) -> Result<(Controlled, u32, Exports<Controlled>), String> {
    let (host, exports) = start::<E>().await?;
    let key = host.observed().callback(tool)?;
    Ok((host, key, exports))
}

/// The registered command a corpus records: its name, description and whether it has argument
/// completions.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// The command name.
    pub name: String,
    /// The command description.
    pub description: String,
    /// Whether the factory registers argument completions.
    pub completions: bool,
}

/// Asserts the host received exactly the recorded command.
pub fn assert_registered_command(observed: &Observed, command: &Command) {
    assert_eq!(
        observed.registrations.commands,
        [(
            command.name.clone(),
            Some(command.description.clone()),
            command.completions
        )],
        "the registered command"
    );
}

/// One effect a shipped example asked the host for.
#[derive(Debug, PartialEq)]
pub enum Observation {
    /// A notification request, with the message and severity it carries.
    Notify {
        /// The message text.
        message: String,
        /// The severity, when the request names one.
        kind: Option<String>,
    },
    /// An admitted user-message delivery, with the content and options it carries.
    Delivery {
        /// The delivered content.
        content: Option<Value>,
        /// The delivery options, absent when the request carries none.
        options: Option<Value>,
    },
}

/// The effects the host recorded in `lines`, in the order it recorded them.
pub fn observations(lines: &[String]) -> Vec<Observation> {
    lines
        .iter()
        .filter_map(|line| {
            if let Some(rest) = line.strip_prefix("action sendUserMessage ") {
                let arguments: [Value; 2] = serde_json::from_str(rest).expect("delivery arguments");
                return Some(Observation::Delivery {
                    content: arguments[0].get("value").cloned(),
                    options: arguments[1].get("value").cloned(),
                });
            }
            let receipt = super::ui_host::receipt(line)?;
            if receipt.method != "notify" {
                return None;
            }
            Some(Observation::Notify {
                message: receipt.operands[0]
                    .as_str()
                    .expect("a notification message")
                    .to_owned(),
                kind: receipt.operands[1].as_str().map(str::to_lowercase),
            })
        })
        .collect()
}

/// Parses one registration record without the transport's declared-kinds member.
pub fn registered_metadata(text: &str) -> Value {
    let mut value: Value = serde_json::from_str(text).unwrap();
    value.as_object_mut().unwrap().shift_remove("schemaKinds");
    value
}

/// Parses the caller-visible result without re-decoding its structured details.
pub fn result_value(encoded: &str) -> Value {
    serde_json::from_str(encoded).unwrap()
}

/// Releases list handlers and every registration in arrival order, asserting the host admitted
/// exactly `expected` and kept nothing lent.
pub fn release_all(host: &Controlled, expected: &[impl AsRef<str>]) {
    let handlers = host.observe().ui.lists.take_handlers();
    for key in handlers {
        release(key);
    }
    let expected: Vec<&str> = expected.iter().map(AsRef::as_ref).collect();
    let names = released(host);
    assert_eq!(names, expected, "the registrations the host received");
    assert_eq!(host.observed().live, 0, "the host lent nothing it kept");
}

/// Releases every registration of the controlled host in the order it received them and
/// returns their names.
pub fn released(host: &Controlled) -> Vec<String> {
    let mut names = Vec::new();
    loop {
        let next = host.observe().next_to_release();
        let Some((name, key)) = next else { break };
        release(key);
        names.push(name);
    }
    names
}

/// Releases every registration of the built component in the order the host received them and
/// returns their names.
///
/// # Errors
/// Returns the component failure that ended the walk.
pub fn released_component(harness: &mut Harness) -> Result<Vec<String>, String> {
    super::block_on(harness.release_all())
        .into_iter()
        .map(|(name, outcome)| outcome.map(|()| name).map_err(|error| error.to_string()))
        .collect()
}

/// Runs the built component's command callback and returns its outcome; `held` keeps the
/// operation it awaits pending until the test saw it and released it.
pub fn invoke_command_on_component(
    harness: &mut Harness,
    key: u32,
    args: &str,
    held: bool,
) -> Result<(), String> {
    let ctx = harness
        .command_context("/work")
        .map_err(|error| error.to_string())?;
    let exports = harness.exports.clone();
    let (pending, entered, open) = super::observed::gate();
    if held {
        harness.observed().hold_next_idle(pending);
    }
    super::block_on(harness.store.run_concurrent(async |accessor| {
        let call =
            exports.call_invoke_command(accessor, super::host::borrow(key), args.to_owned(), ctx);
        let release = super::host::release_held(accessor, held.then_some((entered, open)));
        super::join::alongside(call, release).await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    Ok(())
}

/// Invokes a tool callback of the built component and returns its encoded result.
pub fn invoke_tool_on_component(
    harness: &mut Harness,
    key: u32,
    params: &Value,
    resources: ToolCapabilities,
) -> Result<String, String> {
    let invocation = ToolInvocation {
        tool_call_id: "component-call".into(),
        params: params.to_string(),
    };
    let exports = harness.exports.clone();
    let encoded = super::block_on(harness.store.run_concurrent(async |accessor| {
        exports
            .call_invoke_tool(accessor, super::host::borrow(key), invocation, resources)
            .await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())??;
    Ok(encoded)
}

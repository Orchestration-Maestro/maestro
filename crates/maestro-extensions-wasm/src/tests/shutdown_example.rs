//! The shipped shutdown factory exercised through registration and invocation.
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{
    invoke_command_on_component, invoke_tool_on_component, registered_metadata, release_all,
    result_value, start,
};
use super::host_family::guest::ToolCapabilities as ComponentCapabilities;
use crate::component_adapter::{Exports, ToolCapabilities};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/shutdown_command.rs"]
mod shutdown;

/// The same factory exported by the shipped component.
struct Shutdown;

impl Extension for Shutdown {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        shutdown::factory(api)
    }
}

/// The registrations the example makes, in the order the host received them.
const REGISTERED: [&str; 3] = [
    "command quit",
    "tool finish_and_exit",
    "tool deploy_and_exit",
];

/// Recorded metadata, arguments and results these tests consume.
fn corpus() -> Value {
    serde_json::from_str(include_str!("shutdown_expected.json")).unwrap()
}

/// One controlled tool invocation with its arguments and optional progress and cancellation.
struct Invocation {
    /// The tool callback to invoke.
    key: u32,
    /// Whether the host lends a progress handle.
    progress: bool,
    /// Whether the host's cancellation flag is raised before the call.
    aborted: bool,
    /// The flag the host raises on the first progress update, when supplied.
    cancel_on_progress: Option<super::controlled::Flag>,
    /// The arguments document.
    params: Value,
}

/// Runs one controlled tool invocation and returns its encoded result.
async fn run(
    host: &Controlled,
    exports: &Exports<Controlled>,
    invocation: Invocation,
) -> Result<String, String> {
    let flag = invocation.cancel_on_progress.clone().unwrap_or_default();
    if invocation.aborted {
        flag.abort();
    }
    let update = invocation
        .progress
        .then(|| match &invocation.cancel_on_progress {
            Some(_) => host.aborting_update(&flag),
            None => host.update(),
        });
    exports
        .invoke_tool(
            invocation.key,
            crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                tool_call_id: "controlled-call".into(),
                params: invocation.params.to_string(),
            },
            ToolCapabilities {
                ctx: host.context("/work"),
                signal: Some(host.signal(&flag)),
                update,
            },
        )
        .await
}

/// Register once and assert the declared command, tool metadata and schemas in order.
#[test]
fn shutdown_example_registers_command_and_tool_contracts() -> Result<(), String> {
    super::block_on(async {
        let (host, ..) = start::<Shutdown>().await?;
        let expected = corpus();
        assert_eq!(
            host.observed().registrations.commands,
            [(
                "quit".to_owned(),
                Some("Exit Maestro cleanly".to_owned()),
                false
            )]
        );
        let tools: Vec<Value> = host
            .observed()
            .tools
            .iter()
            .map(String::as_str)
            .map(registered_metadata)
            .collect();
        assert_eq!(json!(tools), expected["tools"]);
        assert!(host.observed().callback("prepare finish_and_exit").is_err());
        assert!(host.observed().callback("prepare deploy_and_exit").is_err());
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// Each supplied argument text reaches the command, which requests one shutdown.
#[test]
fn quit_command_requests_shutdown() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<Shutdown>().await?;
        let key = host.observed().callback("command quit")?;
        for args in corpus()["commandArguments"].as_array().unwrap() {
            let start = host.observed().transcript.len();
            exports
                .invoke_command(
                    key,
                    args.as_str().unwrap().to_owned(),
                    host.command_context("/work"),
                )
                .await?;
            assert_eq!(host.observed().transcript[start..], ["context shutdown"]);
        }
        assert!(host.observed().updates.is_empty());
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// The finishing tool requests shutdown before it hands back its exact result.
#[test]
fn finish_tool_returns_after_request() -> Result<(), String> {
    super::block_on(async {
        let (host, finish, _, exports) = started().await?;
        let expected = corpus();
        for row in expected["finish"].as_array().unwrap() {
            let start = host.observed().transcript.len();
            let encoded = run(
                &host,
                &exports,
                Invocation {
                    key: finish,
                    progress: row["update"].as_bool().unwrap(),
                    aborted: row["aborted"].as_bool().unwrap(),
                    cancel_on_progress: None,
                    params: row["params"].clone(),
                },
            )
            .await?;
            assert_eq!(result_value(&encoded), expected["finishResult"]);
            assert_eq!(host.observed().transcript[start..], ["context shutdown"]);
            assert!(host.observed().updates.is_empty());
        }
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// Every admitted environment reaches both progress updates and the final details.
#[test]
fn deployment_progress_keeps_environment_and_order() -> Result<(), String> {
    super::block_on(async {
        let (host, _, deploy, exports) = started().await?;
        let expected = corpus();
        for row in expected["deployments"].as_array().unwrap() {
            let environment = row["environment"].clone();
            let updates = host.observed().updates.len();
            let start = host.observed().transcript.len();
            let encoded = run(
                &host,
                &exports,
                Invocation {
                    key: deploy,
                    progress: true,
                    aborted: true,
                    cancel_on_progress: None,
                    params: json!({"environment": environment.clone()}),
                },
            )
            .await?;
            let delivered: Vec<Value> = host.observed().updates[updates..]
                .iter()
                .map(|update| result_value(update))
                .collect();
            assert_eq!(delivered.len(), 2);
            assert_eq!(delivered[0]["content"][0]["text"], row["progress"]);
            assert_eq!(delivered[0]["details"], json!({}));
            assert_eq!(
                delivered[1]["content"][0]["text"],
                expected["deploymentComplete"]
            );
            assert_eq!(delivered[1]["details"], json!({}));
            assert_eq!(host.observed().transcript[start..], ["context shutdown"]);
            assert_eq!(
                result_value(&encoded),
                json!({
                    "content": [{
                        "type": "text",
                        "text": expected["deploymentResult"]
                    }],
                    "details": {"environment": environment}
                })
            );
        }
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// Without a progress handle the deployment still requests shutdown and answers.
#[test]
fn deployment_without_progress_keeps_result() -> Result<(), String> {
    super::block_on(async {
        let (host, _, deploy, exports) = started().await?;
        let expected = corpus();
        let start = host.observed().transcript.len();
        let encoded = run(
            &host,
            &exports,
            Invocation {
                key: deploy,
                progress: false,
                aborted: true,
                cancel_on_progress: None,
                params: expected["withoutProgress"]["params"].clone(),
            },
        )
        .await?;
        assert!(host.observed().updates.is_empty());
        assert_eq!(host.observed().transcript[start..], ["context shutdown"]);
        assert_eq!(
            result_value(&encoded),
            expected["withoutProgress"]["result"]
        );
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// A cancellation raised while progress runs keeps both updates and the result.
#[test]
fn deployment_survives_cancellation_during_progress() -> Result<(), String> {
    super::block_on(async {
        let (host, _, deploy, exports) = started().await?;
        let expected = corpus();
        let flag = super::controlled::Flag::default();
        let updates = host.observed().updates.len();
        let start = host.observed().transcript.len();
        let encoded = run(
            &host,
            &exports,
            Invocation {
                key: deploy,
                progress: true,
                aborted: false,
                cancel_on_progress: Some(flag.clone()),
                params: json!({"environment": "staging"}),
            },
        )
        .await?;
        assert!(
            flag.shared().load(std::sync::atomic::Ordering::SeqCst),
            "the host raised the flag during progress"
        );
        let delivered: Vec<Value> = host.observed().updates[updates..]
            .iter()
            .map(|update| result_value(update))
            .collect();
        assert_eq!(delivered.len(), 2, "both updates still arrive");
        assert_eq!(
            delivered[1]["content"][0]["text"],
            expected["deploymentComplete"]
        );
        assert_eq!(host.observed().transcript[start..], ["context shutdown"]);
        assert_eq!(
            result_value(&encoded),
            json!({
                "content": [{
                    "type": "text",
                    "text": expected["deploymentResult"]
                }],
                "details": {"environment": "staging"}
            })
        );
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// Argument edits reach the deployment's own member checks: a mutated member, an added
/// member, a replaced document and a deleted member.
#[test]
fn deployment_arguments_edits_keep_the_member_checks() -> Result<(), String> {
    super::block_on(async {
        let (host, _, deploy, exports) = started().await?;
        let expected = corpus();
        let invocation = |params: Value| Invocation {
            key: deploy,
            progress: true,
            aborted: true,
            cancel_on_progress: None,
            params,
        };
        let mutated = expected["deployments"][0]["environment"].clone();
        let encoded = run(&host, &exports, invocation(json!({"environment": mutated}))).await?;
        assert_eq!(
            result_value(&encoded)["details"],
            json!({"environment": mutated})
        );
        let added = run(
            &host,
            &exports,
            invocation(expected["withoutProgress"]["params"].clone()),
        )
        .await?;
        assert_eq!(result_value(&added), expected["withoutProgress"]["result"]);
        for (edited, needle) in [(json!(5), "invalid type"), (json!({}), "missing field")] {
            let updates = host.observed().updates.len();
            let start = host.observed().transcript.len();
            let error = run(&host, &exports, invocation(edited)).await.unwrap_err();
            assert!(error.contains(needle), "{error}");
            assert_eq!(host.observed().updates.len(), updates, "{error}");
            assert!(host.observed().transcript[start..].is_empty(), "{error}");
        }
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// Every pre-registered callback is available by name on a fresh controlled host.
async fn started() -> Result<(Controlled, u32, u32, Exports<Controlled>), String> {
    let (host, exports) = start::<Shutdown>().await?;
    let finish = host.observed().callback("tool finish_and_exit")?;
    let deploy = host.observed().callback("tool deploy_and_exit")?;
    Ok((host, finish, deploy, exports))
}

/// The built component of the shipped example.
fn component_harness() -> Result<super::host::Harness, String> {
    super::host::Harness::example("shutdown_command")
}

/// Runs the built component's quit command and asserts one shutdown request.
fn component_quit(harness: &mut super::host::Harness) -> Result<(), String> {
    let key = harness
        .callback("command quit")
        .map_err(|error| error.to_string())?;
    let start = harness.observed().transcript.len();
    invoke_command_on_component(harness, key, "ignored Ω", false)?;
    assert_eq!(harness.observed().transcript[start..], ["context shutdown"]);
    Ok(())
}

/// Runs the built component's finishing tool and asserts its exact result.
fn component_finish(harness: &mut super::host::Harness, expected: &Value) -> Result<(), String> {
    let key = harness
        .callback("tool finish_and_exit")
        .map_err(|error| error.to_string())?;
    let resources = ComponentCapabilities {
        ctx: harness
            .ordinary("/work")
            .map_err(|error| error.to_string())?,
        signal: None,
        update: None,
    };
    let start = harness.observed().transcript.len();
    let encoded = invoke_tool_on_component(harness, key, &json!({}), resources)?;
    assert_eq!(harness.observed().transcript[start..], ["context shutdown"]);
    assert_eq!(result_value(&encoded), expected["finishResult"]);
    Ok(())
}

/// Runs the built component's deployment tool with progress and a raised flag.
fn component_deploy(harness: &mut super::host::Harness, expected: &Value) -> Result<(), String> {
    let key = harness
        .callback("tool deploy_and_exit")
        .map_err(|error| error.to_string())?;
    let resources = ComponentCapabilities {
        ctx: harness
            .ordinary("/work")
            .map_err(|error| error.to_string())?,
        signal: Some(wasmtime::component::Resource::new_own(
            harness.flag(true).map_err(|error| error.to_string())?,
        )),
        update: Some(harness.update().map_err(|error| error.to_string())?),
    };
    let start = harness.observed().transcript.len();
    let encoded =
        invoke_tool_on_component(harness, key, &json!({"environment": "staging"}), resources)?;
    assert_eq!(harness.observed().transcript[start..], ["context shutdown"]);
    let updates: Vec<Value> = harness
        .observed()
        .updates
        .iter()
        .map(|update| result_value(update))
        .collect();
    assert_eq!(updates.len(), 2);
    assert_eq!(
        updates[0]["content"][0]["text"],
        expected["deployments"][1]["progress"]
    );
    assert_eq!(
        result_value(&encoded),
        json!({
            "content": [{
                "type": "text",
                "text": expected["deploymentResult"]
            }],
            "details": {"environment": "staging"}
        })
    );
    Ok(())
}

/// Runs the built component's deployment tool without a progress handle.
fn component_deploy_without_progress(
    harness: &mut super::host::Harness,
    expected: &Value,
) -> Result<(), String> {
    let key = harness
        .callback("tool deploy_and_exit")
        .map_err(|error| error.to_string())?;
    let resources = ComponentCapabilities {
        ctx: harness
            .ordinary("/work")
            .map_err(|error| error.to_string())?,
        signal: Some(wasmtime::component::Resource::new_own(
            harness.flag(true).map_err(|error| error.to_string())?,
        )),
        update: None,
    };
    let updates = harness.observed().updates.len();
    let start = harness.observed().transcript.len();
    let encoded = invoke_tool_on_component(
        harness,
        key,
        &expected["withoutProgress"]["params"],
        resources,
    )?;
    assert_eq!(
        harness.observed().updates.len(),
        updates,
        "no handle, no updates"
    );
    assert_eq!(harness.observed().transcript[start..], ["context shutdown"]);
    assert_eq!(
        result_value(&encoded),
        expected["withoutProgress"]["result"]
    );
    Ok(())
}

/// The built example runs the command and both tools through the component interface.
#[test]
fn shutdown_component_runs_every_registration() -> Result<(), String> {
    let expected = corpus();
    let mut harness = component_harness()?;
    assert_eq!(harness.observed().transcript, ["register command quit"]);
    component_quit(&mut harness)?;
    component_finish(&mut harness, &expected)?;
    component_deploy(&mut harness, &expected)?;
    component_deploy_without_progress(&mut harness, &expected)?;
    assert_eq!(
        harness.observed().transcript,
        [
            "register command quit",
            "context shutdown",
            "context shutdown",
            "context shutdown",
            "context shutdown"
        ]
    );
    Ok(())
}

/// Starts the factory with every context call refused with `message`.
async fn refused_shutdown(message: &str) -> Result<(Controlled, Exports<Controlled>), String> {
    let (host, exports) = start::<Shutdown>().await?;
    host.observe()
        .catalog
        .control(&json!({"stale": message}).to_string());
    Ok((host, exports))
}

/// A refused shutdown on the command surfaces the message and requests nothing.
fn refused_command(shutdown: &str) -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = refused_shutdown(shutdown).await?;
        let key = host.observed().callback("command quit")?;
        let start = host.observed().transcript.len();
        assert_eq!(
            exports
                .invoke_command(key, String::new(), host.command_context("/work"))
                .await,
            Err(shutdown.to_owned())
        );
        assert!(host.observed().transcript[start..].is_empty());
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// A refused shutdown on either tool surfaces the message and keeps the updates delivered.
fn refused_tools(shutdown: &str) -> Result<(), String> {
    super::block_on(async {
        let (host, finish, deploy, exports) = started().await?;
        host.observe()
            .catalog
            .control(&json!({"stale": shutdown}).to_string());
        let start = host.observed().transcript.len();
        assert_eq!(
            run(
                &host,
                &exports,
                Invocation {
                    key: finish,
                    progress: true,
                    aborted: false,
                    cancel_on_progress: None,
                    params: json!({}),
                },
            )
            .await,
            Err(shutdown.to_owned())
        );
        assert!(host.observed().transcript[start..].is_empty());
        assert!(host.observed().updates.is_empty());
        let start = host.observed().transcript.len();
        assert_eq!(
            run(
                &host,
                &exports,
                Invocation {
                    key: deploy,
                    progress: true,
                    aborted: false,
                    cancel_on_progress: None,
                    params: json!({"environment": "staging"}),
                },
            )
            .await,
            Err(shutdown.to_owned())
        );
        assert_eq!(host.observed().updates.len(), 2);
        assert!(host.observed().transcript[start..].is_empty());
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// A refused progress update keeps the updates delivered so far and requests no shutdown.
fn refused_progress(position: usize, message: &str) -> Result<(), String> {
    super::block_on(async {
        let (host, _, deploy, exports) = started().await?;
        host.observe().refuse_update = Some((position, message.to_owned()));
        let start = host.observed().transcript.len();
        assert_eq!(
            run(
                &host,
                &exports,
                Invocation {
                    key: deploy,
                    progress: true,
                    aborted: false,
                    cancel_on_progress: None,
                    params: json!({"environment": "staging"}),
                },
            )
            .await,
            Err(message.to_owned())
        );
        assert_eq!(
            host.observed().updates.len(),
            position + 1,
            "the updates already delivered are kept"
        );
        assert!(
            host.observed().transcript[start..].is_empty(),
            "no shutdown is requested"
        );
        release_all(&host, &REGISTERED);
        Ok(())
    })
}

/// A refused shutdown or progress update surfaces the host's message and stops the call.
#[test]
fn shutdown_example_stops_on_callback_errors() -> Result<(), String> {
    let expected = corpus();
    let shutdown = expected["refusals"]["shutdown"]
        .as_str()
        .unwrap()
        .to_owned();
    let update = expected["refusals"]["update"].as_str().unwrap();
    refused_command(&shutdown)?;
    refused_tools(&shutdown)?;
    refused_progress(0, update)?;
    refused_progress(1, update)
}

/// A refused registration returns its message and stops the later ones.
#[test]
fn shutdown_example_stops_on_registration_error() -> Result<(), String> {
    super::block_on(async {
        let expected = corpus();
        let message = expected["refusals"]["registration"]
            .as_str()
            .unwrap()
            .to_owned();
        for row in expected["registrations"].as_array().unwrap() {
            let host = Controlled::default();
            let exports = Exports::new(host.clone());
            host.observe().registrations.refuse_at = Some((
                usize::try_from(row["refused"].as_u64().unwrap()).unwrap(),
                message.clone(),
            ));
            assert_eq!(
                exports.start::<Shutdown>().await,
                Err(message.clone()),
                "the registration at position {} is refused",
                row["refused"]
            );
            let admitted: Vec<&str> = row["admitted"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap())
                .collect();
            release_all(&host, &admitted);
        }
        Ok(())
    })
}

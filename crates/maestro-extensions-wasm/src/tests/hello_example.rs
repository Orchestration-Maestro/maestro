//! The shipped greeting factory exercised through registration and invocation.
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{invoke_tool_on_component, result_value, start};
use crate::component_adapter::{Exports, ToolCapabilities, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/hello.rs"]
mod hello;

/// The same factory exported by the shipped component.
struct Hello;

impl Extension for Hello {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        hello::factory(api)
    }
}

/// Recorded arguments, metadata and results consumed by these tests.
fn corpus() -> Value {
    serde_json::from_str(include_str!("hello_expected.json")).unwrap()
}

/// Assert the metadata and schema received by the actual host against a recorded document.
pub(super) fn assert_metadata(metadata: &[String], expected: &Value) {
    assert_eq!(metadata.len(), 1);
    let mut actual: Value = serde_json::from_str(&metadata[0]).unwrap();
    actual["parameters"] = serde_json::from_str(actual["parameters"].as_str().unwrap()).unwrap();
    assert_eq!(actual, *expected);
}

/// Starts the shipped factory on a fresh controlled host and returns its tool callback key.
async fn started() -> Result<(Controlled, u32, Exports<Controlled>), String> {
    let (host, exports) = start::<Hello>().await?;
    let key = host.observed().callback("tool hello")?;
    Ok((host, key, exports))
}

/// Releases the tool callback and asserts the host registered nothing else.
pub(super) fn released(host: &Controlled, key: u32) {
    release(key);
    assert_eq!(host.observed().live, 0);
}

/// Register once and assert the declared required string without optional metadata.
#[test]
fn hello_registers_the_greeting_schema() -> Result<(), String> {
    super::block_on(async {
        let (host, key, _exports) = started().await?;
        assert_metadata(&host.observed().tools, &corpus()["metadata"]);
        assert!(host.observed().callback("prepare hello").is_err());
        released(&host, key);
        Ok(())
    })
}

/// Every supplied string, including empty and whitespace-only names, reaches both outputs.
#[test]
fn hello_preserves_supplied_names() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = started().await?;
        for row in corpus()["rows"].as_array().unwrap() {
            let actual = exports
                .invoke_tool(
                    key,
                    crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                        tool_call_id: "controlled-call".into(),
                        params: row["input"].to_string(),
                    },
                    ToolCapabilities {
                        ctx: host.context("/work"),
                        signal: None,
                        update: None,
                    },
                )
                .await?;
            assert_eq!(result_value(&actual), row["result"]);
        }
        released(&host, key);
        Ok(())
    })
}

/// An already-aborted signal and a progress handle do not change this tool's answer.
#[test]
fn hello_ignores_unused_invocation_handles() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = started().await?;
        let flag = super::controlled::Flag::default();
        flag.abort();
        let actual = exports
            .invoke_tool(
                key,
                crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                    tool_call_id: "another-call".into(),
                    params: json!({"name":"Ada"}).to_string(),
                },
                ToolCapabilities {
                    ctx: host.context("/elsewhere"),
                    signal: Some(host.signal(&flag)),
                    update: Some(host.update()),
                },
            )
            .await?;
        assert_eq!(result_value(&actual), corpus()["rows"][0]["result"]);
        assert!(host.observed().updates.is_empty());
        released(&host, key);
        Ok(())
    })
}

/// The built single-copy example registers and executes across the component interface.
#[test]
fn hello_component_greets_through_the_host() -> Result<(), String> {
    let path = super::build::example_component("hello")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    let mut harness = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|error| error.to_string())?;
    assert_metadata(&harness.observed().tools, &corpus()["metadata"]);
    let key = harness.callback("tool hello").map_err(|e| e.to_string())?;
    for row in corpus()["rows"].as_array().unwrap() {
        let resources = super::host_family::guest::ToolCapabilities {
            ctx: harness
                .ordinary("/work")
                .map_err(|error| error.to_string())?,
            signal: None,
            update: None,
        };
        let encoded = invoke_tool_on_component(&mut harness, key, &row["input"], resources)?;
        assert_eq!(result_value(&encoded), row["result"]);
    }
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

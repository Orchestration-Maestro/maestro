//! The shipped duration factory exercised through registration and invocation.
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{registered, release_all, result_value};
use super::hello_example::assert_metadata;
use crate::component_adapter::{Exports, ToolCapabilities};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/with-deps/main.rs"]
mod with_deps;

/// The same factory exported by the shipped component.
struct WithDeps;

impl Extension for WithDeps {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        with_deps::factory(api)
    }
}

/// Recorded metadata and outcomes consumed by these tests.
fn corpus() -> Value {
    serde_json::from_str(include_str!("with_deps_expected.json")).unwrap()
}

/// The recorded rows of one group.
fn rows(group: &str) -> Vec<Value> {
    let recorded = corpus();
    recorded["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["group"] == group)
        .cloned()
        .collect()
}

/// The arguments of one recorded row, with or without an unread additional member.
fn params(row: &Value, extra: bool) -> Value {
    let mut params = json!({"duration": row["duration"]});
    if extra {
        params["note"] = json!("ignored");
    }
    params
}

/// Compare one recorded row's complete outcome with a caller-visible one.
fn assert_outcome(row: &Value, actual: &Result<Value, String>) {
    let input = row["duration"].as_str().unwrap();
    let expected = match row["outcome"].as_str() {
        Some("ok") => Ok(row["result"].clone()),
        _ => Err(row["error"].as_str().unwrap().to_owned()),
    };
    assert_eq!(actual, &expected, "input {input:?}");
}

/// One invocation of the registered callback through the in-process adapter.
async fn invoked(
    host: &Controlled,
    exports: &Exports<Controlled>,
    key: u32,
    params: Value,
) -> Result<Value, String> {
    exports
        .invoke_tool(
            key,
            crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                tool_call_id: "controlled-call".into(),
                params: params.to_string(),
            },
            ToolCapabilities {
                ctx: host.context("/work"),
                signal: None,
                update: None,
            },
        )
        .await
        .map(|encoded| result_value(&encoded))
}

/// Runs the recorded rows of one group through the in-process adapter.
fn run_group(group: &str, count: usize) -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = registered::<WithDeps>("tool parse_duration").await?;
        let recorded = rows(group);
        assert_eq!(recorded.len(), count, "{group} rows");
        for row in recorded {
            assert_outcome(
                &row,
                &invoked(&host, &exports, key, params(&row, false)).await,
            );
        }
        release_all(&host, &["tool parse_duration"]);
        Ok(())
    })
}

/// Register once and assert the declared required string without optional metadata.
#[test]
fn with_deps_registers_duration_schema() -> Result<(), String> {
    super::block_on(async {
        let (host, ..) = registered::<WithDeps>("tool parse_duration").await?;
        assert_metadata(&host.observed().tools, &corpus()["metadata"]);
        assert!(host.observed().callback("prepare parse_duration").is_err());
        release_all(&host, &["tool parse_duration"]);
        Ok(())
    })
}

/// The authored examples and every alias keep their exact number and text.
#[test]
fn with_deps_preserves_unit_results() -> Result<(), String> {
    run_group("authored", 3)?;
    run_group("unit_aliases", 31)
}

/// Every boundary row keeps its exact rendered number or its rejection.
#[test]
fn with_deps_preserves_numeric_boundaries() -> Result<(), String> {
    run_group("numeric_boundaries", 20)
}

/// Unsupported text keeps the literal input in its own error message.
#[test]
fn with_deps_rejects_invalid_duration_text() -> Result<(), String> {
    run_group("rejection_or_native_extension", 33)
}

/// Unread invocation handles and an additional member change nothing, and a call after a
/// rejected one still succeeds.
#[test]
fn with_deps_ignores_unused_invocation_handles() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = registered::<WithDeps>("tool parse_duration").await?;
        let flag = super::controlled::Flag::default();
        flag.abort();
        let row = rows("unused_member").pop().unwrap();
        let actual = exports
            .invoke_tool(
                key,
                crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                    tool_call_id: "another-call".into(),
                    params: params(&row, true).to_string(),
                },
                ToolCapabilities {
                    ctx: host.context("/elsewhere"),
                    signal: Some(host.signal(&flag)),
                    update: Some(host.update()),
                },
            )
            .await
            .map(|encoded| result_value(&encoded));
        assert_eq!(actual, Ok(row["result"].clone()));
        assert!(host.observed().updates.is_empty());
        let rejected = rows("rejection_or_native_extension").pop().unwrap();
        assert_outcome(
            &rejected,
            &invoked(&host, &exports, key, params(&rejected, false)).await,
        );
        assert_outcome(
            &row,
            &invoked(&host, &exports, key, params(&row, false)).await,
        );
        release_all(&host, &["tool parse_duration"]);
        Ok(())
    })
}

/// The built single-copy example registers and executes across the component interface.
#[test]
fn with_deps_component_uses_its_parsing_dependency() -> Result<(), String> {
    let mut harness = super::host::Harness::example("with_deps")?;
    let recorded = corpus();
    assert_metadata(&harness.observed().tools, &recorded["metadata"]);
    let key = harness
        .callback("tool parse_duration")
        .map_err(|error| error.to_string())?;
    for row in recorded["rows"].as_array().unwrap() {
        let invocation = super::host_family::guest::ToolInvocation {
            tool_call_id: "component-call".into(),
            params: params(row, false).to_string(),
        };
        let resources = super::host_family::guest::ToolCapabilities {
            ctx: harness
                .ordinary("/work")
                .map_err(|error| error.to_string())?,
            signal: None,
            update: None,
        };
        let exports = harness.exports.clone();
        let outcome = super::block_on(harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_tool(accessor, super::host::borrow(key), invocation, resources)
                .await
        }))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?
        .map(|encoded| result_value(&encoded));
        assert_outcome(row, &outcome);
    }
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

//! The shipped prompt-guidance factory exercised through registration and invocation.
use serde_json::Value;

use super::controlled::Controlled;
use super::documents::Answer;
use crate::component_adapter::{Capabilities, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/prompt_customizer.rs"]
mod prompt_customizer;

/// The same factory exported by the shipped component.
struct PromptCustomizer;

impl Extension for PromptCustomizer {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        prompt_customizer::prompt_customizer(api)
    }
}

/// Recorded events and results consumed by these tests.
fn corpus() -> Value {
    serde_json::from_str(include_str!("prompt_customizer_expected.json")).unwrap()
}

/// Releases the event callback and asserts the host registered nothing else.
fn released(host: &Controlled, key: u32) {
    release(key);
    assert_eq!(host.observed().live, 0);
}

/// Every recorded prompt option set produces its expected guidance without editing the event.
#[test]
fn prompt_guidance_uses_supplied_tools_and_skills() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<PromptCustomizer>().await?;
        let key = host.observed().callback("event before_agent_start")?;
        for row in corpus()["rows"].as_array().unwrap() {
            let event = &row["event"];
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
            assert_eq!(
                Answer::of(super::guest_family::delivery(outcome))?,
                Answer::returned(event, Some(row["result"].clone())),
                "{}",
                row["name"]
            );
        }
        released(&host, key);
        Ok(())
    })
}

/// The built single-copy example produces the same guidance across the component interface.
#[test]
fn prompt_guidance_component_matches_supplied_options() -> Result<(), String> {
    let mut harness = super::host::Harness::example("prompt_customizer")?;
    let key = harness
        .callback("event before_agent_start")
        .map_err(|error| error.to_string())?;
    for row in corpus()["rows"].as_array().unwrap() {
        let capabilities = super::host_family::guest::Capabilities {
            ctx: harness
                .ordinary("/controlled")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = harness.exports.clone();
        let event = row["event"].to_string();
        let run = super::block_on(harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), event, capabilities)
                .await
        }))
        .map_err(|error| error.to_string())?;
        let called = run.map_err(|error| error.to_string())?;
        let outcome = called?;
        assert_eq!(
            Answer::of(super::host_family::delivery(outcome))?,
            Answer::returned(&row["event"], Some(row["result"].clone())),
            "{}",
            row["name"]
        );
    }
    super::block_on(harness.release(key)).map_err(|error| error.to_string())?;
    assert_eq!(harness.observed().live, 0);
    Ok(())
}

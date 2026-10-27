//! Controlled execution of the shipped question tool through registered exports.
use super::{
    controlled::Controlled,
    example_support::{registered, result_value},
};
use crate::{
    Extension, ExtensionAPI, ExtensionFuture,
    component_adapter::{Exports, ToolCapabilities},
};
use serde_json::{Value, json};
#[path = "../../../../examples/extensions/question.rs"]
mod question;
/// The shipped factory, without an alternate implementation.
struct Question;
impl Extension for Question {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        question::factory(api)
    }
}
/// Invokes the registered tool with controlled context observations.
async fn ask(
    host: &Controlled,
    exports: &Exports<Controlled>,
    key: u32,
    params: Value,
) -> Result<Value, String> {
    let result = exports
        .invoke_tool(
            key,
            crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                tool_call_id: "question".into(),
                params: params.to_string(),
            },
            ToolCapabilities {
                ctx: host.context("/work"),
                signal: None,
                update: None,
            },
        )
        .await?;
    Ok(result_value(&result))
}
#[test]
fn question_headless_preserves_labels_and_null_answer() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = registered::<Question>("tool question").await?;
        assert_eq!(
            ask(
                &host,
                &exports,
                key,
                json!({"question":"Which?","options":[{"label":"Rust","description":"Fast"}]})
            )
            .await?,
            json!({"content":[{"type":"text","text":"Error: UI not available (running in non-interactive mode)"}],"details":{"question":"Which?","options":["Rust"],"answer":null}})
        );
        super::example_support::release_all(
            &host,
            &[
                "render-call question",
                "render-result question",
                "tool question",
            ],
        );
        Ok(())
    })
}

mod ui;

mod render;

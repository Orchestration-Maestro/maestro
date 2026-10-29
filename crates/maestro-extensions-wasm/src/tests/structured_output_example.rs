//! The shipped structured answer renderer through both registered callback adapters.
use super::scenario::{Driver, ToolRenderRun};
use super::theme_host::{Palette, Reply};
use crate::{Extension, ExtensionAPI, ExtensionFuture};
use serde_json::json;

#[path = "../../../../examples/extensions/structured_output.rs"]
mod structured_output;

/// The same factory exported by the shipped component.
struct StructuredOutput;
impl Extension for StructuredOutput {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        structured_output::factory(api)
    }
}

/// Literal host replies expose the requested theme roles and bold title.
fn palette() -> Palette {
    let mut replies: Vec<_> = [
        ("toolTitle", "<bold>Headline"),
        ("text", "Summary"),
        ("muted", "1. Review"),
        ("muted", "2.  Ship 雪 "),
    ]
    .into_iter()
    .map(|(role, text)| Reply {
        method: "fg".into(),
        operands: json!([role, text]),
        answer: json!({"ok":format!("<{role}>{text}")}),
    })
    .collect();
    replies.push(Reply {
        method: "bold".into(),
        operands: json!(["Headline"]),
        answer: json!({"ok":"<bold>Headline"}),
    });
    Palette {
        name: None,
        source_path: None,
        source_info: None,
        mode: "truecolor".into(),
        replies,
    }
}

/// Both expansion states keep the whole answer; missing details use only the first block.
async fn rendering(driver: &mut impl Driver) -> Result<(), String> {
    let details =
        json!({"headline":"Headline","summary":"Summary","actionItems":["Review"," Ship 雪 "]});
    for expanded in [false, true] {
        let info = json!({"toolCallId":"answer","cwd":"/work","args":{},"expanded":expanded});
        for (result, expected) in [
            (
                json!({"content":[],"details":details,"terminate":true}),
                "<toolTitle><bold>Headline\n<text>Summary\n\n<muted>1. Review\n<muted>2.  Ship 雪 ",
            ),
            (
                json!({"content":[{"type":"text","text":"fallback"}]}),
                "fallback",
            ),
            (json!({"content":[],"details":null}), ""),
            (
                json!({"content":[{"type":"text","text":"fallback"}],"details":false}),
                "fallback",
            ),
            (
                json!({"content":[{"type":"text","text":"fallback"}],"details":0}),
                "fallback",
            ),
            (
                json!({"content":[{"type":"text","text":"fallback"}],"details":-0.0}),
                "fallback",
            ),
            (
                json!({"content":[{"type":"text","text":"fallback"}],"details":""}),
                "fallback",
            ),
            (
                json!({"content":[{"type":"image","data":"AA==","mimeType":"image/png"},{"type":"text","text":"not first"}]}),
                "",
            ),
            (
                json!({"content":[],"details":{"headline":"Headline","summary":"Summary","actionItems":[]}}),
                "<toolTitle><bold>Headline\n<text>Summary\n",
            ),
        ] {
            let rows = driver
                .render_tool(
                    ToolRenderRun {
                        tool: "structured_output",
                        slot: "result",
                        info: &info,
                        result: Some(&result),
                    },
                    200,
                )
                .await?;
            assert_eq!(rows, [format!("text {expected} 0 0|200")]);
        }
    }
    driver.release_all().await;
    Ok(())
}

/// Registered in-process rendering uses the supplied host theme and result details.
#[test]
fn structured_output_renders_complete_answers_and_fallbacks() -> Result<(), String> {
    super::block_on(async {
        let mut driver = super::ControlledDriver::example::<StructuredOutput>().await?;
        driver.publish_theme(palette());
        rendering(&mut driver).await
    })
}

/// The built single-copy component renders the same collapsed and expanded rows.
#[test]
fn structured_output_component_renders_complete_answers_and_fallbacks() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "structured_output")?;
    super::block_on(async {
        let mut driver = super::ComponentDriver::new(path);
        driver.start().await?;
        driver.publish_theme(palette())?;
        rendering(&mut driver).await
    })
}

//! Registered question call/result renderers on both callback adapters.
use super::Question;
use crate::tests::{
    scenario::{Driver, ToolRenderRun},
    theme_host::{Palette, Reply},
};
use serde_json::{Value, json};
/// Literal style requests keep the semantic roles observable.
fn palette() -> Palette {
    let mut replies: Vec<_> = [
        ("toolTitle", "<bold>question "),
        ("muted", "Which?"),
        ("dim", "  Options: 1. Rust, 2. Go, 3. Type something."),
        ("warning", "Cancelled"),
        ("success", "✓ "),
        ("muted", "(wrote) "),
        ("accent", "snow"),
        ("accent", "1. Rust"),
        ("accent", "unknown"),
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
        operands: json!(["question "]),
        answer: json!({"ok":"<bold>question "}),
    });
    Palette {
        name: Some("dark".into()),
        source_path: None,
        source_info: None,
        mode: "truecolor".into(),
        replies,
    }
}
/// Renders through the callback that registration supplied to the host.
async fn paint(
    driver: &mut impl Driver,
    slot: &str,
    args: Value,
    result: &Value,
) -> Result<String, String> {
    let info = json!({"toolCallId":"question", "cwd":"/work", "args":args,"executionStarted":true,"argsComplete":true,"isPartial":false,"expanded":false,"showImages":false,"isError":false});
    let rows = driver
        .render_tool(
            ToolRenderRun {
                tool: "question",
                slot,
                info: &info,
                result: (slot == "result").then_some(result),
            },
            200,
        )
        .await?;
    assert_eq!(rows.len(), 1);
    Ok(rows[0].clone())
}
/// Complete and empty calls retain the title and supplied option order.
async fn calls(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        paint(
            driver,
            "call",
            json!({"question":"Which?","options":[{"label":"Rust"},{"label":"Go"}]}),
            &Value::Null
        )
        .await?,
        "text <toolTitle><bold>question <muted>Which?\n<dim>  Options: 1. Rust, 2. Go, 3. Type something. 0 0|200"
    );
    assert_eq!(
        paint(
            driver,
            "call",
            json!({"question":"Which?","options":[]}),
            &Value::Null
        )
        .await?,
        "text <toolTitle><bold>question <muted>Which? 0 0|200"
    );
    Ok(())
}
/// Answer kinds and missing details retain their authored display.
async fn exercise(driver: &mut impl Driver) -> Result<(), String> {
    calls(driver).await?;
    for (details, expected) in [
        (
            json!({"answer":null,"options":["Rust","Go"]}),
            "text <warning>Cancelled 0 0|200",
        ),
        (
            json!({"answer":"snow","options":["Rust"],"wasCustom":true}),
            "text <success>✓ <muted>(wrote) <accent>snow 0 0|200",
        ),
        (
            json!({"answer":"Rust","options":["Rust","Rust"],"wasCustom":false}),
            "text <success>✓ <accent>1. Rust 0 0|200",
        ),
        (
            json!({"answer":"unknown","options":["Rust"],"wasCustom":false}),
            "text <success>✓ <accent>unknown 0 0|200",
        ),
    ] {
        assert_eq!(
            paint(
                driver,
                "result",
                Value::Null,
                &json!({"content":[],"details":details})
            )
            .await?,
            expected
        );
    }
    fallback(driver).await?;
    driver.release_all().await;
    Ok(())
}
/// Absent and falsey details show the first text block rather than cancellation.
async fn fallback(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        paint(
            driver,
            "result",
            Value::Null,
            &json!({"content":[{"type":"text","text":"fallback"}]})
        )
        .await?,
        "text fallback 0 0|200"
    );
    assert_eq!(
        paint(driver, "result", Value::Null, &json!({"content":[]})).await?,
        "text  0 0|200"
    );
    for details in [Value::Null, json!(false), json!(0), json!("")] {
        assert_eq!(
            paint(
                driver,
                "result",
                Value::Null,
                &json!({"content":[{"type":"text","text":"fallback"}],"details":details})
            )
            .await?,
            "text fallback 0 0|200"
        );
    }
    Ok(())
}
#[test]
fn question_renders_calls_and_results() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = crate::tests::ControlledDriver::example::<Question>().await?;
        driver.publish_theme(palette());
        exercise(&mut driver).await
    })
}
#[test]
fn question_component_renders_calls_and_results() -> Result<(), String> {
    let path = crate::tests::build::example_component("question")?;
    crate::tests::block_on(async {
        let mut driver = crate::tests::ComponentDriver::new(path);
        driver.start().await?;
        driver.publish_theme(palette())?;
        exercise(&mut driver).await
    })
}

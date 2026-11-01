//! Exercises the shipped footer through the built component's command and UI interfaces.

use serde_json::json;

use super::ComponentDriver;
use crate::tests::scenario::Driver;

/// Starts the single shipped component on a fresh controlled host.
async fn start() -> Result<ComponentDriver, String> {
    let path = crate::tests::build::example_component("custom_footer")?;
    let mut driver = ComponentDriver::new(path);
    driver.start().await?;
    Ok(driver)
}

/// The toggle installs one branch listener and removes it when restoring the host footer.
#[test]
fn custom_footer_component_toggles_and_disposes() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        assert_eq!(
            driver.registrations().0,
            [(
                "footer".to_owned(),
                Some("Toggle custom footer".to_owned()),
                false
            )]
        );
        assert_eq!(driver.footer(80).await?, None);
        for _ in 0..2 {
            let from = driver.transcript().len();
            driver.run_plain_command("footer", "ignored").await?;
            assert_eq!(driver.branch_change().await?, [json!({"ok": null})]);
            let lines = driver.transcript();
            assert!(
                lines[from..]
                    .iter()
                    .any(|line| line == "writer requestRender false")
            );
            assert!(
                lines[from..]
                    .iter()
                    .any(|line| line.contains("Custom footer enabled"))
            );
            driver.run_plain_command("footer", "").await?;
            assert_eq!(driver.footer(80).await?, None);
            assert!(driver.branch_change().await?.is_empty());
            assert!(
                driver
                    .transcript()
                    .iter()
                    .any(|line| line.contains("Default footer restored"))
            );
        }
        assert_eq!(
            driver
                .harness
                .as_mut()
                .unwrap()
                .observed()
                .bus
                .removals
                .len(),
            2
        );
        driver.release_all().await;
        Ok(())
    })
}

/// Supplies one assistant entry whose cache counts must not enter the displayed token totals.
fn assistant(id: &str, input: u32, output: u32, total: f64) -> serde_json::Value {
    json!({"type":"message", "id":id, "parentId":null, "timestamp":"2026-01-01",
        "message":{"role":"assistant", "content":[], "api":"openai-responses",
            "provider":"controlled", "model":"old-model", "timestamp":0, "stopReason":"stop",
            "usage":{"input":input, "output":output, "cacheRead":999_999, "cacheWrite":888_888,
                "totalTokens":9_999_999, "cost":{"input":7, "output":8, "cacheRead":9,
                    "cacheWrite":10, "total":total}}}})
}

/// Changes the live session history without recreating the footer or its context.
fn history(driver: &mut ComponentDriver, entries: &serde_json::Value) {
    driver.harness.as_mut().unwrap().observed().readers.control(
        &json!({"bind":{"replies":[{"method":"getBranch", "args":[null], "value":entries}]}})
            .to_string(),
    );
}

/// Supplies a model through the context's captured model reader, including absent and empty IDs.
fn model(driver: &mut ComponentDriver, id: Option<&str>, operation: &str) {
    let value = id.map(|id| {
        json!({"id":id, "name":"Controlled", "api":"openai-responses",
        "provider":"controlled", "baseUrl":"https://invalid.example", "reasoning":false,
        "input":["text"], "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
        "contextWindow":10000, "maxTokens":1000})
    });
    driver.harness.as_mut().unwrap().observed().catalog.control(
        &json!({operation:{"replies":[{"method":"model","args":[],"value":value}]}}).to_string(),
    );
}

/// Independent expected strings and host replies for a single render.
struct Rendering<'a> {
    /// Plain token/cost text requested from the theme.
    left: &'a str,
    /// Plain model/branch text requested from the theme.
    right: &'a str,
    /// The host's supplied cell count for each styled side.
    measured: usize,
    /// Space between the sides, independently worked from the supplied width.
    padding: &'a str,
    /// Width the terminal supplies.
    width: u32,
    /// The clipping helper's supplied answer.
    clipped: &'a str,
}

/// Checks the text, padding and clipping operands the built footer sends to its host.
async fn rendered(driver: &mut ComponentDriver, case: Rendering<'_>) -> Result<(), String> {
    use crate::tests::theme_host::{Palette, Reply};
    let styled = |text| format!("\x1b[2m{text}\x1b[22m");
    let left = styled(case.left);
    let right = styled(case.right);
    let observed = driver.harness.as_mut().unwrap().observed();
    observed.themes.publish(
        "dark",
        Palette {
            name: None,
            source_path: None,
            source_info: None,
            mode: "truecolor".to_owned(),
            replies: [case.left, case.right]
                .into_iter()
                .map(|text| Reply {
                    method: "fg".to_owned(),
                    operands: json!(["dim", text]),
                    answer: json!({"ok":styled(text)}),
                })
                .collect(),
        },
    );
    observed.actions.value =
        json!({"visible_width":case.measured, "truncate_to_width":case.clipped});
    observed.helper_calls.clear();
    assert_eq!(
        driver.footer(case.width).await?,
        Some(vec![case.clipped.to_owned()])
    );
    let line = format!("{left}{}{right}", case.padding);
    assert_eq!(
        driver.harness.as_mut().unwrap().observed().helper_calls,
        [
            ("visible_width".to_owned(), json!([left]).to_string()),
            ("visible_width".to_owned(), json!([right]).to_string()),
            (
                "truncate_to_width".to_owned(),
                json!([line, case.width, "...", false]).to_string()
            ),
        ]
    );
    Ok(())
}

/// Publishes new branch metadata without replacing the retained footer.
fn branch(driver: &mut ComponentDriver, name: Option<&str>) {
    driver
        .harness
        .as_mut()
        .unwrap()
        .observed()
        .ui
        .control(&json!({"footer":{"branch":name}}).to_string());
}

/// Exercises nonempty history and terminal widths on the same retained footer.
async fn initial_rendering(driver: &mut ComponentDriver) -> Result<(), String> {
    history(
        driver,
        &json!([
            assistant("a",400,400,0.1),
            {"type":"message","id":"u","parentId":null,"timestamp":"2026-01-01",
                "message":{"role":"user","content":"ignore","timestamp":0,"usage":{"input":999_999}}},
            {"type":"model_change","id":"m","parentId":null,"timestamp":"2026-01-01",
                "provider":"controlled","modelId":"ignored"},
            assistant("b",599,600,0.0254)
        ]),
    );
    model(driver, Some("model-雪-x"), "model");
    branch(driver, Some("main"));
    driver.run_plain_command("footer", "").await?;
    rendered(
        driver,
        Rendering {
            left: "↑999 ↓1.0k $0.125",
            right: "model-雪-x (main)",
            measured: 17,
            padding: "      ",
            width: 40,
            clipped: "↑999 ↓1.0k $0.125      model-雪-x (main)",
        },
    )
    .await?;
    rendered(
        driver,
        Rendering {
            left: "↑999 ↓1.0k $0.125",
            right: "model-雪-x (main)",
            measured: 17,
            padding: " ",
            width: 10,
            clipped: "↑999 ↓1...",
        },
    )
    .await?;
    Ok(())
}

/// Empty history, missing and empty identities remain usable at zero and narrow widths.
async fn fallback_rendering(driver: &mut ComponentDriver) -> Result<(), String> {
    history(driver, &json!([]));
    for (id, name, width, clipped) in [(None, None, 0, ""), (Some(""), Some(""), 1, ".")] {
        model(driver, id, "model_update");
        branch(driver, name);
        rendered(
            driver,
            Rendering {
                left: "↑0 ↓0 $0.000",
                right: "no-model",
                measured: 12,
                padding: " ",
                width,
                clipped,
            },
        )
        .await?;
    }
    Ok(())
}

/// The retained footer rereads history, model and branch, and delegates cell-aware clipping.
#[test]
fn custom_footer_component_renders_live_history_and_clips() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        initial_rendering(&mut driver).await?;
        history(
            &mut driver,
            &json!([
                assistant("c", 1000, 1000, 0.1),
                assistant("d", 501, 999, 0.0254)
            ]),
        );
        branch(&mut driver, Some("topic"));
        rendered(
            &mut driver,
            Rendering {
                left: "↑1.5k ↓2.0k $0.125",
                right: "model-雪-x (topic)",
                measured: 18,
                padding: "    ",
                width: 40,
                clipped: "↑1.5k ↓2.0k $0.125    model-雪-x (topic)",
            },
        )
        .await?;
        fallback_rendering(&mut driver).await?;
        driver.run_plain_command("footer", "").await?;
        driver.release_all().await;
        Ok(())
    })
}

/// Renders one supplied usage record against an independently recorded decimal result.
async fn rounding(usage: serde_json::Value, left: &str) -> Result<(), String> {
    let mut driver = start().await?;
    history(&mut driver, &json!([usage]));
    driver.run_plain_command("footer", "").await?;
    rendered(
        &mut driver,
        Rendering {
            left,
            right: "no-model",
            measured: 30,
            padding: " ",
            width: 0,
            clipped: "",
        },
    )
    .await?;
    driver.run_plain_command("footer", "").await?;
    driver.release_all().await;
    Ok(())
}

/// An exact decimal tie rounds upward in the thousands label.
#[test]
fn custom_footer_component_rounds_token_ties_up() -> Result<(), String> {
    crate::tests::block_on(rounding(assistant("tie", 1250, 0, 0.0), "↑1.3k ↓0 $0.000"))
}

/// An exact decimal tie rounds upward in the dollar label.
#[test]
fn custom_footer_component_rounds_cost_ties_up() -> Result<(), String> {
    crate::tests::block_on(rounding(assistant("tie", 0, 0, 0.0625), "↑0 ↓0 $0.063"))
}

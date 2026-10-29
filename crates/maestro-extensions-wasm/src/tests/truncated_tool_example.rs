//! Registered search behavior with controlled process replies and host helper observations.
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::example_support::{registered, release_all, result_value};
use super::scenario::{Driver, ToolRenderRun};
use super::theme_host::{Palette, Reply};
use crate::component_adapter::{Exports, ToolCapabilities};
use crate::{Extension, ExtensionAPI, ExtensionFuture, child_process::Stdio};

#[path = "../../../../examples/extensions/truncated_tool.rs"]
mod truncated_tool;

/// The same factory that the shipped component exports.
struct TruncatedTool;
impl Extension for TruncatedTool {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        truncated_tool::factory(api)
    }
}

/// Invokes the registered tool using its host-supplied context.
async fn search(
    host: &Controlled,
    exports: &Exports<Controlled>,
    key: u32,
) -> Result<Value, String> {
    let encoded = exports
        .invoke_tool(
            key,
            crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                tool_call_id: "search".into(),
                params: json!({"pattern":"needle","path":"","glob":""}).to_string(),
            },
            ToolCapabilities {
                ctx: host.context("/work"),
                signal: None,
                update: None,
            },
        )
        .await?;
    Ok(result_value(&encoded))
}

/// Supplies the next search process's complete streams and exit status.
fn supply(host: &Controlled, stdout: &str, stderr: &str, code: i32) {
    let bank = &mut host.observe().native_io;
    bank.stdout = vec![stdout.as_bytes().to_vec()];
    bank.stderr = vec![stderr.as_bytes().to_vec()];
    bank.exit_code = Some(code);
}

#[test]
fn truncated_tool_handles_empty_and_failed_process_output() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = registered::<TruncatedTool>("tool rg").await?;
        host.observe().actions.value = json!({"is_whitespace_char":true});
        for (code, stdout) in [(1, ""), (0, ""), (0, " \u{feff}\n\t")] {
            supply(&host, stdout, "", code);
            assert_eq!(
                search(&host, &exports, key).await?,
                json!({
                "content":[{"type":"text","text":"No matches found"}],
                "details":{"pattern":"needle","path":"","glob":"","matchCount":0}})
            );
        }
        assert_eq!(host.observed().native_io.launches.len(), 3);
        for (code, stderr) in [(1, "partial failure"), (2, "bad pattern")] {
            supply(&host, "", stderr, code);
            assert_eq!(
                search(&host, &exports, key).await,
                Err(format!("ripgrep failed: {stderr}"))
            );
        }
        host.observe().native_io.error = Some("process refused".into());
        assert_eq!(
            search(&host, &exports, key).await,
            Err("ripgrep failed: process refused".into())
        );
        release_all(&host, &["render-call rg", "render-result rg", "tool rg"]);
        Ok(())
    })
}

/// Literal style replies make each requested semantic role observable.
fn palette() -> Palette {
    let mut replies: Vec<_> = [
        ("toolTitle", "<bold>rg "),
        ("accent", "\"needle\""),
        ("muted", " in src"),
        ("dim", " --glob *.rs"),
        ("warning", "Searching..."),
        ("dim", "No matches found"),
        ("success", "3 matches"),
        ("warning", " (truncated)"),
        ("dim", "one"),
        ("dim", "two"),
        ("muted", "... (use read tool to see full output)"),
        ("dim", "Full output: /tmp/maestro-rg-test/output.txt"),
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
        operands: json!(["rg "]),
        answer: json!({"ok":"<bold>rg "}),
    });
    for line in 1..=20 {
        replies.push(Reply {
            method: "fg".into(),
            operands: json!(["dim", format!("line {line}")]),
            answer: json!({"ok":format!("<dim>line {line}")}),
        });
    }
    Palette {
        name: None,
        source_path: None,
        source_info: None,
        mode: "truecolor".into(),
        replies,
    }
}

/// One row with independently supplied result and rendering options.
async fn paint(
    driver: &mut impl Driver,
    slot: &str,
    result: &Value,
    flags: [bool; 2],
) -> Result<String, String> {
    let [expanded, partial] = flags;
    let info = json!({"toolCallId":"search","cwd":"/work","args":{
        "pattern":"needle","path":"src","glob":"*.rs"},
        "executionStarted":true,"argsComplete":true,"isPartial":partial,
        "expanded":expanded,"showImages":false,"isError":false});
    let rows = driver
        .render_tool(
            ToolRenderRun {
                tool: "rg",
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

/// The recorded call and result displays, including the 20-line expansion limit.
async fn rendering(driver: &mut impl Driver) -> Result<(), String> {
    assert_eq!(
        paint(driver, "call", &Value::Null, [false; 2]).await?,
        "text <toolTitle><bold>rg <accent>\"needle\"<muted> in src<dim> --glob *.rs 0 0|200"
    );
    let empty = json!({"content":[]});
    assert_eq!(
        paint(driver, "result", &empty, [false, true]).await?,
        "text <warning>Searching... 0 0|200"
    );
    assert_eq!(
        paint(driver, "result", &empty, [false; 2]).await?,
        "text <dim>No matches found 0 0|200"
    );
    let result = json!({"content":[{"type":"text","text":"one\ntwo"}],"details":{"matchCount":3}});
    assert_eq!(
        paint(driver, "result", &result, [false; 2]).await?,
        "text <success>3 matches 0 0|200"
    );
    assert_eq!(
        paint(driver, "result", &result, [true, false]).await?,
        "text <success>3 matches\n<dim>one\n<dim>two 0 0|200"
    );
    truncated_rendering(driver).await?;
    driver.release_all().await;
    Ok(())
}

/// Collapsed and expanded views keep the warning, limit and artifact path.
async fn truncated_rendering(driver: &mut impl Driver) -> Result<(), String> {
    let content = (1..=21)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = json!({"content":[{"type":"text","text":content}],"details":{
        "matchCount":3,"truncation":{"truncated":true},"fullOutputPath":"/tmp/maestro-rg-test/output.txt"}});
    assert_eq!(
        paint(driver, "result", &result, [false; 2]).await?,
        "text <success>3 matches<warning> (truncated) 0 0|200"
    );
    let lines = (1..=20)
        .map(|line| format!("<dim>line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        paint(driver, "result", &result, [true, false]).await?,
        format!(
            "text <success>3 matches<warning> (truncated)\n{lines}\n<muted>... (use read tool to see full output)\n<dim>Full output: /tmp/maestro-rg-test/output.txt 0 0|200"
        )
    );
    Ok(())
}

#[test]
fn truncated_tool_renders_calls_and_results() -> Result<(), String> {
    super::block_on(async {
        let mut driver = super::ControlledDriver::example::<TruncatedTool>().await?;
        driver.publish_theme(palette());
        rendering(&mut driver).await
    })
}

#[test]
fn truncated_tool_component_renders_calls_and_results() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "truncated_tool")?;
    super::block_on(async {
        let mut driver = super::ComponentDriver::new(path);
        driver.start().await?;
        driver.publish_theme(palette())?;
        rendering(&mut driver).await
    })
}

#[test]
fn truncated_tool_counts_matches_and_registers_limits() -> Result<(), String> {
    super::block_on(async {
        let (host, key, exports) = registered::<TruncatedTool>("tool rg").await?;
        assert_eq!(
            super::example_support::registered_metadata(&host.observed().tools[0]),
            json!({
            "name":"rg","label":"ripgrep",
            "description":"Search file contents using ripgrep. Output is truncated to 2000 lines or 50.0KB (whichever is hit first). If truncated, full output is saved to a temp file.",
            "parameters":{"type":"object","required":["pattern"],"properties":{
                "pattern":{"type":"string","description":"Search pattern (regex)"},
                "path":{"type":"string","description":"Directory to search (default: current directory)"},
                "glob":{"type":"string","description":"File glob pattern, e.g. '*.ts'"}}}})
        );
        supply(&host, "1:x\n2:x\n", "", 0);
        host.observe().actions.value = json!({"is_whitespace_char":false,"tools.truncate_head":{
                "content":"1:x\n2:x\n","truncated":false,"truncatedBy":null,
                "totalLines":3,"totalBytes":8,"outputLines":3,"outputBytes":8,
                "lastLinePartial":false,"firstLineExceedsLimit":false,"maxLines":2000,"maxBytes":51200}});
        assert_eq!(
            search(&host, &exports, key).await?,
            json!({
            "content":[{"type":"text","text":"1:x\n2:x\n"}],
            "details":{"pattern":"needle","path":"","glob":"","matchCount":2}})
        );
        let observed = host.observed();
        let [(command, args, options)] = observed.native_io.launches.as_slice() else {
            panic!("one search process");
        };
        assert_eq!(command, "rg");
        assert_eq!(
            args,
            &["--line-number", "--color=never", "--", "needle", "."]
        );
        assert_eq!(options.cwd.as_deref(), Some("/work"));
        assert_eq!((&options.env, options.detached), (&None, false));
        assert_eq!(options.stdio, [Stdio::Ignore, Stdio::Pipe, Stdio::Pipe]);
        drop(observed);
        assert!(host.observed().updates.is_empty());
        release_all(&host, &["render-call rg", "render-result rg", "tool rg"]);
        Ok(())
    })
}

//! Sends status messages through the shipped component's command and draws each delivered
//! message with the renderer the component registered.

use serde_json::{Value, json};

use super::ComponentDriver;
use crate::tests::host::{Clock, Harness};
use crate::tests::renderer_host::{Call, Reply};
use crate::tests::scenario::Driver;
use crate::tests::theme_host::{self, Palette};

/// The wall-clock instant the command reads, in milliseconds since the Unix epoch.
const NOW: u64 = 1_767_323_045_678;

/// Width the transcript supplies to each rendered message.
const WIDTH: u32 = 40;

/// Starts the single shipped component with a supplied wall clock.
async fn start() -> Result<ComponentDriver, String> {
    let path = crate::tests::build::example_component("message_renderer")?;
    let (engine, component) =
        crate::tests::build::compile(&path).map_err(|error| error.to_string())?;
    let clock = Clock::default();
    clock.set(NOW);
    let harness = Harness::from_compiled_with_clock(&engine, &component, clock)
        .await
        .map_err(|error| error.to_string())?;
    let mut driver = ComponentDriver::new(path);
    driver.harness = Some(harness);
    Ok(driver)
}

/// The messages and options the command handed to the host, in order.
fn sent(driver: &ComponentDriver) -> Vec<Value> {
    driver
        .transcript()
        .iter()
        .filter_map(|line| line.strip_prefix("action sendMessage "))
        .map(|operands| serde_json::from_str(operands).unwrap())
        .collect()
}

/// The command registers with its usage, and the renderer under the sent custom type.
#[test]
fn message_renderer_registers_status_command_and_renderer() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        assert_eq!(
            driver.registrations().0,
            [(
                "status".to_owned(),
                Some("Post a status note: /status [warn|error] <text>".to_owned()),
                false
            )]
        );
        assert!(
            driver
                .transcript()
                .contains(&r#"renderer-register "status-update""#.to_owned())
        );
        driver.identity("renderer status-update")?;
        driver.release_all().await;
        Ok(())
    })
}

/// A leading `warn` or `error` word selects the level; anything else sends the trimmed text
/// at the `info` level, and each message records the clock's milliseconds.
#[test]
fn message_renderer_status_sends_level_defaults_and_timestamp() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        let cases = [
            ("  all   good \t", "info", "all   good"),
            ("warn  disk \n  low ", "warn", "disk low"),
            ("error", "error", "Status report"),
            ("\u{feff} error\u{a0}", "error", "Status report"),
            ("", "info", ""),
            ("warning x", "info", "warning x"),
            ("WARN x", "info", "WARN x"),
            ("warn\u{85}x", "info", "warn\u{85}x"),
        ];
        for (args, _, _) in cases {
            driver.run_plain_command("status", args).await?;
        }
        let expected: Vec<_> = cases
            .iter()
            .map(|(_, level, content)| {
                json!([
                    {"present":true, "value":{"customType":"status-update", "content":content,
                        "display":true, "details":{"level":level, "timestamp":NOW}}},
                    {"present":false}
                ])
            })
            .collect();
        assert_eq!(sent(&driver), expected);
        driver.release_all().await;
        Ok(())
    })
}

/// One message as the transcript hands it to the renderer, and the expected drawing.
struct Drawing<'a> {
    /// The message document the renderer receives.
    message: Value,
    /// Whether the transcript shows the message expanded.
    expanded: bool,
    /// The level role and label the theme colors.
    level: (&'a str, &'a str),
    /// The text after the colored level.
    content: &'a str,
    /// The instant passed to the host's local time helper and its answer, when one is shown.
    time: Option<(i64, &'a str)>,
}

/// Wraps text as the controlled theme paints it with `role`.
fn painted(role: &str, text: &str) -> String {
    format!("<{role}>{text}")
}

/// Publishes the theme and local time answers one drawing needs and returns the text the
/// renderer must hand to its text widget.
fn publish(driver: &mut ComponentDriver, drawing: &Drawing<'_>) -> String {
    let (role, label) = drawing.level;
    let row = format!("box 1 1|{WIDTH}");
    let mut replies = vec![
        ("fg", json!([role, label]), painted(role, label)),
        ("bg", json!(["customMessageBg", row]), painted("bg", &row)),
    ];
    let mut text = format!("{} {}", painted(role, label), drawing.content);
    if let Some((_, time)) = drawing.time {
        let line = format!("  sent {time}");
        text = format!("{text}\n{}", painted("dim", &line));
        replies.push(("fg", json!(["dim", line]), painted("dim", &line)));
    }
    let observed = driver.harness.as_mut().unwrap().observed();
    observed.themes.publish(
        "dark",
        Palette {
            name: None,
            source_path: None,
            source_info: None,
            mode: "truecolor".to_owned(),
            replies: replies
                .into_iter()
                .map(|(method, operands, answer)| theme_host::Reply {
                    method: method.to_owned(),
                    operands,
                    answer: json!({ "ok": answer }),
                })
                .collect(),
        },
    );
    observed.actions.value = json!({"time.local_time_string": drawing.time.map(|(_, time)| time)});
    observed.helper_calls.clear();
    text
}

/// Renders one delivered message through the registered renderer and checks every row and
/// host request.
async fn drawn(driver: &mut ComponentDriver, drawing: Drawing<'_>) -> Result<(), String> {
    let text = publish(driver, &drawing);
    let renderer = driver.identity("renderer status-update")?;
    let view = driver
        .renderer(
            renderer,
            &drawing.message.to_string(),
            drawing.expanded,
            "dark",
        )
        .await?
        .ok_or("the renderer drew no component")?;
    assert_eq!(
        driver.component(view, Call::Render(WIDTH)).await?,
        Reply::Lines(vec![
            painted("bg", &format!("box 1 1|{WIDTH}")),
            format!("text {text} 0 0|{WIDTH}"),
        ])
    );
    let calls: Vec<_> = drawing
        .time
        .map(|(instant, _)| {
            (
                "time.local_time_string".to_owned(),
                json!([instant]).to_string(),
            )
        })
        .into_iter()
        .collect();
    assert_eq!(
        driver.harness.as_mut().unwrap().observed().helper_calls,
        calls
    );
    driver.drop_component(view).await
}

/// A message the command delivered draws its level and text, and adds the local time it was
/// sent only when expanded.
#[test]
fn message_renderer_draws_delivered_messages_collapsed_and_expanded() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        for (args, level, content) in [
            ("warn disk  low", ("warning", "[WARN]"), "disk low"),
            ("error", ("error", "[ERROR]"), "Status report"),
            (" all good ", ("success", "[INFO]"), "all good"),
        ] {
            driver.run_plain_command("status", args).await?;
            let mut message = sent(&driver).last().unwrap()[0]["value"].clone();
            message["role"] = json!("custom");
            message["timestamp"] = json!(NOW);
            for (expanded, time) in [
                (false, None),
                (true, Some((1_767_323_045_678, "3:04:05 AM"))),
            ] {
                let message = message.clone();
                let drawing = Drawing {
                    message,
                    expanded,
                    level,
                    content,
                    time,
                };
                drawn(&mut driver, drawing).await?;
            }
        }
        driver.release_all().await;
        Ok(())
    })
}

/// A stored status message with the supplied content and, unless null, details.
fn stored(details: Value, content: &Value) -> Value {
    let mut message = json!({"role":"custom", "customType":"status-update",
        "content":content, "display":true, "timestamp":NOW});
    if !details.is_null() {
        message["details"] = details;
    }
    message
}

/// Expanded drawings of stored messages whose details the command would not write.
fn stored_drawings() -> Vec<Drawing<'static>> {
    let blocks = json!([
        {"type":"text", "text":"first"},
        {"type":"image", "data":"AA==", "mimeType":"image/png"},
        {"type":"text", "text":"second"}
    ]);
    [
        (
            json!(null),
            blocks,
            ("success", "[INFO]"),
            "first\nsecond",
            None,
        ),
        (
            json!({"level":"error", "timestamp":0}),
            json!("x"),
            ("error", "[ERROR]"),
            "x",
            None,
        ),
        (
            json!({"level":7, "timestamp":5}),
            json!("y"),
            ("success", "[INFO]"),
            "y",
            Some((5, "12:00:00 AM")),
        ),
        (
            json!({"level":""}),
            json!("z"),
            ("success", "[]"),
            "z",
            None,
        ),
        (
            json!({"level":"straße"}),
            json!(""),
            ("success", "[STRASSE]"),
            "",
            None,
        ),
    ]
    .into_iter()
    .map(|(details, content, level, text, time)| Drawing {
        message: stored(details, &content),
        expanded: true,
        level,
        content: text,
        time,
    })
    .collect()
}

/// Stored messages without a text level draw as `info`, text blocks join by line, and a
/// missing or zero timestamp shows no time even when expanded.
#[test]
fn message_renderer_defaults_level_and_skips_absent_timestamps() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        for drawing in stored_drawings() {
            drawn(&mut driver, drawing).await?;
        }
        driver.release_all().await;
        Ok(())
    })
}

/// Any nonzero numeric timestamp shows a time: its milliseconds truncate toward zero, and a
/// value past every instant reaches the host as one beyond the calendar.
#[test]
fn message_renderer_truncates_numeric_timestamps() -> Result<(), String> {
    crate::tests::block_on(async {
        let mut driver = start().await?;
        for (timestamp, time) in [
            (
                json!(1_767_323_045_678.0),
                (1_767_323_045_678, "3:04:05 AM"),
            ),
            (
                json!(1_767_323_045_678.9),
                (1_767_323_045_678, "3:04:05 AM"),
            ),
            (json!(-1.5), (-1, "11:59:59 PM")),
            (json!(0.5), (0, "12:00:00 AM")),
            (json!(1e300), (i64::MAX, "Invalid Date")),
        ] {
            let drawing = Drawing {
                message: stored(json!({ "timestamp": timestamp }), &json!("x")),
                expanded: true,
                level: ("success", "[INFO]"),
                content: "x",
                time: Some(time),
            };
            drawn(&mut driver, drawing).await?;
        }
        driver.release_all().await;
        Ok(())
    })
}

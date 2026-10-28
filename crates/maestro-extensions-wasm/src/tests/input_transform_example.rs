//! Input decisions and feedback through the shipped factory's native and component callbacks.
use serde_json::{Value, json};

use super::documents::{Answer, Ended};
use super::example_support::{Observation, observations, release_all, start};
use crate::component_adapter::Capabilities;
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/input_transform.rs"]
mod input_transform;

/// The shipped factory, loaded through the author interface.
struct InputTransformExample;

impl Extension for InputTransformExample {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        input_transform::factory(api)
    }
}

/// One supplied input and its independent expected decision and feedback.
struct Case {
    /// The event, including attachments that must not be mutated.
    event: Value,
    /// Exact callback termination, including host rejections.
    ended: Ended,
    /// The requested notification, if any.
    notification: Option<(&'static str, &'static str)>,
    /// Supplied host clock presentation, never read from the test machine.
    clock: &'static str,
    /// Optional notification rejection.
    ui_error: Option<&'static str>,
    /// Optional helper rejection.
    helper_error: Option<&'static str>,
}

/// Records an input with an attachment and an explicit expected action.
fn case(source: &str, text: &str, result: Value) -> Case {
    Case {
        event: json!({"type":"input","source":source,"text":text,
            "images":[{"type":"image","data":"YQ==","mimeType":"image/png"}]}),
        ended: Ended::Returned(Some(result)),
        notification: None,
        clock: "10/27/2026, 9:25:07 AM",
        ui_error: None,
        helper_error: None,
    }
}

/// Supplies whitespace answers for literal boundary characters, not a trimming implementation.
fn replies(clock: &str) -> Value {
    let mut values = json!({"is_whitespace_char":false,"time.local_date_time_string":clock});
    for ch in [' ', '\t', '\n', '\r', '\u{feff}', '\u{a0}', '\u{2028}'] {
        values[format!("is_whitespace_char {}", json!([ch.to_string()]))] = json!(true);
    }
    values
}

/// Checks exact result, unmodified input and notification contents and severity.
fn assert_case(row: &Case, answer: &Answer, lines: &[String]) {
    assert_eq!(answer.event.as_ref(), Some(&row.event));
    assert_eq!(answer.ended, row.ended, "{}", row.event);
    let expected: Vec<_> = row
        .notification
        .iter()
        .map(|(message, kind)| Observation::Notify {
            message: (*message).into(),
            kind: Some((*kind).into()),
        })
        .collect();
    assert_eq!(observations(lines), expected, "{}", row.event);
}

/// Replays each row through both existing adapters, checking resource balance after each call.
fn check(rows: &[Case]) -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = start::<InputTransformExample>().await?;
        let key = host.observed().callback("event input")?;
        for row in rows {
            host.observe().actions.value = replies(row.clock);
            host.observe().actions.error = row.helper_error.map(str::to_owned);
            host.observe()
                .ui
                .control(&json!({"error":row.ui_error}).to_string());
            let from = host.observed().transcript.len();
            let lent = host.observed().live;
            let outcome = exports
                .invoke_event(
                    key,
                    row.event.to_string(),
                    Capabilities {
                        ctx: host.context("/controlled"),
                        signal: None,
                    },
                )
                .await?;
            assert_case(
                row,
                &Answer::of(super::guest_family::delivery(outcome))?,
                &host.observed().transcript[from..],
            );
            assert_eq!(host.observed().live, lent);
        }
        release_all(&host, &["event input"]);
        Ok::<_, String>(())
    })?;
    check_component(rows)
}

/// Runs the identical decisions in the compiled shipped component with supplied host replies.
fn check_component(rows: &[Case]) -> Result<(), String> {
    let path = super::build::example_component("input_transform")?;
    let (engine, component) = super::build::compile(&path).map_err(|e| e.to_string())?;
    let mut host = super::block_on(super::host::Harness::from_compiled(&engine, &component))
        .map_err(|e| e.to_string())?;
    let key = host.callback("event input").map_err(|e| e.to_string())?;
    for row in rows {
        host.observed().actions.value = replies(row.clock);
        host.observed().actions.error = row.helper_error.map(str::to_owned);
        host.observed()
            .ui
            .control(&json!({"error":row.ui_error}).to_string());
        let from = host.observed().transcript.len();
        let lent = host.observed().live;
        let capabilities = super::host_family::guest::Capabilities {
            ctx: host.ordinary("/controlled").map_err(|e| e.to_string())?,
            signal: None,
        };
        let exports = host.exports.clone();
        let event = row.event.to_string();
        let outcome = super::block_on(host.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), event, capabilities)
                .await
        }))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())??;
        let lines = host.observed().transcript[from..].to_vec();
        assert_case(
            row,
            &Answer::of(super::host_family::delivery(outcome))?,
            &lines,
        );
        assert_eq!(host.observed().live, lent);
    }
    super::block_on(host.release(key)).map_err(|e| e.to_string())?;
    assert_eq!(host.observed().live, 0);
    Ok(())
}

/// Injected input bypasses every local action; ordinary near misses continue without trimming.
#[test]
fn input_transform_bypasses_extension_and_continues_other_input() -> Result<(), String> {
    let mut rows = Vec::new();
    for text in ["?quick explain", "?quick ", "PING", "time"] {
        let mut row = case("extension", text, json!({"action":"continue"}));
        row.helper_error = Some("must not request a helper");
        row.ui_error = Some("must not notify");
        rows.push(row);
    }
    for source in ["interactive", "rpc"] {
        for text in [
            "",
            "hello",
            "?quick",
            "?Quick question",
            " ping",
            "ping ",
            "time\n",
        ] {
            rows.push(case(source, text, json!({"action":"continue"})));
        }
    }
    check(&rows)
}

/// Only the literal prefix rewrites; shared whitespace trims the edges but preserves interiors.
#[test]
fn input_transform_rewrites_quick_questions() -> Result<(), String> {
    let mut rows = Vec::new();
    for source in ["interactive", "rpc"] {
        for (text, expected) in [
            (
                "?quick What is Rust?",
                "Respond briefly in 1-2 sentences: What is Rust?",
            ),
            (
                "?quick \u{feff}\t A  😀\nB \u{a0}\u{2028}",
                "Respond briefly in 1-2 sentences: A  😀\nB",
            ),
            (
                "?quick \u{85} question \u{85}",
                "Respond briefly in 1-2 sentences: \u{85} question \u{85}",
            ),
        ] {
            rows.push(case(
                source,
                text,
                json!({"action":"transform","text":expected}),
            ));
        }
    }
    let mut rejected = case("rpc", "?quick question", Value::Null);
    rejected.helper_error = Some("whitespace unavailable");
    rejected.ended = Ended::Failed("whitespace unavailable".into());
    rows.push(rejected);
    check(&rows)
}

/// Empty quick input warns and stops rather than producing an empty model request.
#[test]
fn input_transform_handles_empty_quick_input() -> Result<(), String> {
    let mut rows = Vec::new();
    for text in ["?quick ", "?quick \t\u{feff}\n\u{a0}\u{2028}"] {
        let mut row = case("interactive", text, json!({"action":"handled"}));
        row.notification = Some(("Usage: ?quick <question>", "warning"));
        rows.push(row);
    }
    let mut rejected = case("rpc", "?quick ", Value::Null);
    rejected.ui_error = Some("notification unavailable");
    rejected.notification = Some(("Usage: ?quick <question>", "warning"));
    rejected.ended = Ended::Failed("notification unavailable".into());
    rows.push(rejected);
    check(&rows)
}

/// Ping ignores ASCII case and requests one local informational notification.
#[test]
fn input_transform_handles_ping_locally() -> Result<(), String> {
    let mut rows = Vec::new();
    for text in ["ping", "PING", "PiNg"] {
        let mut row = case("rpc", text, json!({"action":"handled"}));
        row.notification = Some(("pong", "info"));
        rows.push(row);
    }
    let mut rejected = case("interactive", "ping", Value::Null);
    rejected.ui_error = Some("notification unavailable");
    rejected.notification = Some(("pong", "info"));
    rejected.ended = Ended::Failed("notification unavailable".into());
    rows.push(rejected);
    check(&rows)
}

/// Time reads each supplied host clock presentation and propagates helper and UI failures.
#[test]
fn input_transform_handles_time_from_the_host_clock() -> Result<(), String> {
    let mut rows = Vec::new();
    for (text, clock) in [
        ("time", "1/2/2026, 12:05:07 AM"),
        ("TIME", "1/2/2026, 12:05:07 PM"),
    ] {
        let mut row = case("interactive", text, json!({"action":"handled"}));
        row.clock = clock;
        row.notification = Some((clock, "info"));
        rows.push(row);
    }
    for error in ["clock unavailable", "notification unavailable"] {
        let mut row = case("rpc", "TiMe", Value::Null);
        if error == "clock unavailable" {
            row.helper_error = Some(error);
        } else {
            row.ui_error = Some(error);
            row.notification = Some((row.clock, "info"));
        }
        row.ended = Ended::Failed(error.into());
        rows.push(row);
    }
    check(&rows)
}

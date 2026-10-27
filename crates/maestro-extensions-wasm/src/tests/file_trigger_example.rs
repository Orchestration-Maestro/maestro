//! The shipped file trigger through registered callbacks and controlled watch delivery.
#![cfg(target_os = "linux")]
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::documents::Answer;
use super::example_support::{release_all, start};
use super::file_trigger_path::Trigger;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/file_trigger.rs"]
mod file_trigger;

/// The single shipped factory, also exported by the component.
struct FileTrigger;
impl Extension for FileTrigger {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        file_trigger::factory(api)
    }
}

/// Delivers a host event through its registered public callback.
fn deliver(host: &Controlled, exports: &Exports<Controlled>, kind: &str) -> Result<Answer, String> {
    let event = if kind == "session_start" {
        json!({"type":kind,"reason":"startup"})
    } else {
        json!({"type":kind,"reason":"quit"})
    };
    let key = host.observed().callback(&format!("event {kind}"))?;
    let result = super::block_on(exports.invoke_event(
        key,
        event.to_string(),
        Capabilities {
            ctx: host.context("/controlled"),
            signal: None,
        },
    ))?;
    Answer::of(super::guest_family::delivery(result))
}

/// The exact custom messages and options the host admitted, without transport wrappers.
fn messages(host: &Controlled) -> Vec<Value> {
    host.observed()
        .transcript
        .iter()
        .filter_map(|line| {
            let value: Value =
                serde_json::from_str(line.strip_prefix("action sendMessage ")?).unwrap();
            Some(json!([value[0]["value"], value[1]["value"]]))
        })
        .collect()
}

#[test]
fn file_trigger_delivers_nonempty_contents_and_releases_watches() -> Result<(), String> {
    let mut trigger = Trigger::new();
    let (host, exports) = super::block_on(start::<FileTrigger>())?;
    host.observe().catalog.data.has_ui = true;
    deliver(&host, &exports, "session_start")?;
    assert_eq!(
        host.observed().native_watch.paths,
        ["/tmp/agent-trigger.txt"]
    );
    let notifications = super::ui_host::receipts(&host.observed().transcript);
    assert_eq!(notifications.len(), 1);
    assert_eq!(
        notifications[0].operands,
        json!(["Watching /tmp/agent-trigger.txt", "Info"])
    );
    trigger.write(" \u{feff}\nRun the tests Ω\n\u{feff} ");
    host.notify_watch(0)?;
    assert_eq!(
        messages(&host),
        [json!([
            {"customType":"file-trigger", "content":"External trigger: Run the tests Ω", "display":true},
            {"triggerTurn":true}
        ])]
    );
    assert_eq!(
        std::fs::read_to_string("/tmp/agent-trigger.txt").unwrap(),
        ""
    );
    for text in ["", " \r\n\t", "\u{2003}\u{a0}", "\u{feff}", " \u{feff}\t"] {
        trigger.write(text);
        host.notify_watch(0)?;
        assert_eq!(messages(&host).len(), 1, "empty content stays silent");
    }
    host.observe().catalog.data.has_ui = false;
    deliver(&host, &exports, "session_start")?;
    assert!(host.observed().native_watch.watches[0].upgrade().is_none());
    trigger.write("\u{85}second\u{85}");
    host.notify_watch(0)?;
    assert_eq!(messages(&host).len(), 1, "retired watch stays silent");
    host.notify_watch(1)?;
    assert_eq!(messages(&host).len(), 2);
    assert_eq!(
        messages(&host)[1],
        json!([
            {"customType":"file-trigger", "content":"External trigger: \u{85}second\u{85}", "display":true},
            {"triggerTurn":true}
        ])
    );
    deliver(&host, &exports, "session_shutdown")?;
    deliver(&host, &exports, "session_shutdown")?;
    assert!(host.observed().native_watch.watches[1].upgrade().is_none());
    release_all(&host, &["event session_start", "event session_shutdown"]);
    Ok(())
}

#[test]
fn file_trigger_ignores_transient_errors_without_clearing_rejected_text() -> Result<(), String> {
    let trigger = Trigger::new();
    let (host, exports) = super::block_on(start::<FileTrigger>())?;
    deliver(&host, &exports, "session_start")?;
    drop(trigger);
    let missing = host.notify_watch(0);
    let mut trigger = Trigger::new();
    trigger.write("retry me");
    host.observe().actions.error = Some("delivery refused".into());
    let rejected = host.notify_watch(0);
    let retained = std::fs::read_to_string("/tmp/agent-trigger.txt").unwrap();
    assert!(messages(&host).is_empty());
    host.observe().actions.error = None;
    let retry = host.notify_watch(0);
    std::fs::write("/tmp/agent-trigger.txt", b"invalid \xff UTF-8").unwrap();
    let decoded = host.notify_watch(0);
    deliver(&host, &exports, "session_shutdown")?;
    release_all(&host, &["event session_start", "event session_shutdown"]);
    assert_eq!(
        missing,
        Ok(()),
        "a missing file is a transient read failure"
    );
    assert_eq!(rejected, Ok(()), "a rejected delivery stays catchable");
    assert_eq!(retained, "retry me", "rejected text must not be lost");
    assert_eq!(retry, Ok(()));
    assert_eq!(decoded, Ok(()));
    assert_eq!(
        messages(&host),
        [
            json!([{"customType":"file-trigger","content":"External trigger: retry me","display":true},{"triggerTurn":true}]),
            json!([{"customType":"file-trigger","content":"External trigger: invalid \u{fffd} UTF-8","display":true},{"triggerTurn":true}]),
        ]
    );
    Ok(())
}

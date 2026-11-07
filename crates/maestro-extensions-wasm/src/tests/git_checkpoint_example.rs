//! The shipped checkpoint factory at the controlled event, process and UI interfaces.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde_json::{Value, json};

use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

/// The single shipped factory, also exported by the built component.
#[path = "../../../../examples/extensions/git_checkpoint.rs"]
mod git_checkpoint;

/// Loads the same factory exported by the shipped component.
struct Checkpoint;
impl Extension for Checkpoint {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        git_checkpoint::factory(api)
    }
}

/// Delivers a document to its registered event callback and decodes the public outcome.
async fn fire(host: &Controlled, exports: &Exports<Controlled>, event: Value) -> Ended {
    let key = host
        .observed()
        .callback(&format!("event {}", event["type"].as_str().unwrap()))
        .unwrap();
    let outcome = exports
        .invoke_event(
            key,
            event.to_string(),
            Capabilities {
                ctx: host.context("/controlled"),
                signal: None,
            },
        )
        .await
        .unwrap();
    Answer::of(super::guest_family::delivery(outcome))
        .unwrap()
        .ended
}

/// Supplies one leaf response without implementing a session reader.
fn leaf(host: &Controlled, value: &Value) {
    host.observe().readers.control(
        &json!({"bind":{"replies":[{
            "method":"getLeafEntry", "args":[], "value":value
        }]}})
        .to_string(),
    );
}

/// Supplies a successfully completed execution result.
fn stdout(host: &Controlled, value: &str) {
    host.observe().actions.value = json!({"stdout":value,"stderr":"controlled","code":0,"killed":false,"is_whitespace_char":false});
}

/// The literal turn-start document accepted by the registered handler.
fn turn() -> Value {
    json!({"type":"turn_start","turnIndex":0,"timestamp":0})
}

/// The literal tool-result document accepted by the registered handler.
fn result() -> Value {
    json!({"type":"tool_result","toolCallId":"read","toolName":"read","input":{},"content":[],"isError":false})
}

/// The literal before-fork document for the selected entry.
fn fork(id: &str) -> Value {
    json!({"type":"session_before_fork","entryId":id,"position":"at"})
}

/// Successful event callbacks supply no override.
fn returned(ended: &Ended) {
    assert_eq!(*ended, Ended::Returned(None));
}

/// Releases every registration and proves the factory lent no retained resource.
fn release(host: &Controlled) {
    super::example_support::release_all(
        host,
        &[
            "event tool_result",
            "event turn_start",
            "event session_before_fork",
            "event agent_end",
        ],
    );
}

/// Every leaf kind retains its checkpoint and honors the host whitespace decision.
#[test]
fn git_checkpoint_factory_preserves_commands_whitespace_and_entry_kinds() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<Checkpoint>().await?;
        let corpus: Value = serde_json::from_str(include_str!("bookmark_expected.json")).unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        for entry in corpus["rows"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row["entries"].as_array().unwrap())
        {
            if !kinds.insert(entry["type"].as_str().unwrap()) {
                continue;
            }
            leaf(&host, entry);
            returned(&fire(&host, &exports, result()).await);
            stdout(&host, "object-id");
            returned(&fire(&host, &exports, turn()).await);
            // A missing leaf keeps the last identity, and empty output keeps its checkpoint.
            leaf(&host, &Value::Null);
            returned(&fire(&host, &exports, result()).await);
            stdout(&host, "\u{feff} \n");
            host.observe().actions.value["is_whitespace_char"] = json!(true);
            returned(&fire(&host, &exports, turn()).await);
            host.observe().catalog.data.has_ui = true;
            host.observe()
                .ui
                .control(&json!({"answer":"Yes, restore code to that point"}).to_string());
            let mark = host.observed().transcript.len();
            returned(&fire(&host, &exports, fork(entry["id"].as_str().unwrap())).await);
            let lines = host.observed().transcript[mark..].to_vec();
            assert!(
                lines.contains(
                    &r#"action exec ["git",["stash","apply","object-id"],null]"#.to_owned()
                ),
                "{lines:?}"
            );
            let receipts = super::ui_host::receipts(&lines);
            assert_eq!(receipts.len(), 2);
            assert_eq!(
                receipts[1].operands,
                json!(["Code restored to checkpoint", "Info"])
            );
        }
        assert_eq!(kinds.len(), 9);
        assert!(
            host.observed()
                .transcript
                .contains(&r#"action exec ["git",["stash","create"],null]"#.to_owned())
        );
        returned(&fire(&host, &exports, json!({"type":"agent_end","messages":[]})).await);
        let mark = host.observed().transcript.len();
        returned(&fire(&host, &exports, fork("assistant")).await);
        assert!(super::ui_host::receipts(&host.observed().transcript[mark..]).is_empty());
        release(&host);
        Ok(())
    })
}

/// Nonzero and terminated applications fail without announcing a successful restore.
#[test]
fn git_checkpoint_factory_rejects_failed_restores() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<Checkpoint>().await?;
        leaf(
            &host,
            &json!({"type":"custom","id":"entry","parentId":null,"timestamp":"now","customType":"state"}),
        );
        returned(&fire(&host, &exports, result()).await);
        stdout(&host, "object-id");
        returned(&fire(&host, &exports, turn()).await);
        host.observe().catalog.data.has_ui = true;
        for (code, killed) in [(7, false), (0, true), (7, true)] {
            host.observe().actions.value["code"] = json!(code);
            host.observe().actions.value["killed"] = json!(killed);
            host.observe()
                .ui
                .control(&json!({"answer":"Yes"}).to_string());
            let mark = host.observed().transcript.len();
            assert_eq!(
                fire(&host, &exports, fork("entry")).await,
                Ended::Failed(format!(
                    "Could not restore checkpoint (exit code {code}, killed: {killed}): controlled"
                ))
            );
            let lines = host.observed().transcript[mark..].to_vec();
            assert!(
                lines.contains(
                    &r#"action exec ["git",["stash","apply","object-id"],null]"#.to_owned()
                )
            );
            let receipts = super::ui_host::receipts(&lines);
            assert_eq!(receipts.len(), 1, "no success notification: {lines:?}");
        }
        release(&host);
        Ok(())
    })
}

/// Host failures end the callback before any later notification is sent.
#[test]
fn git_checkpoint_factory_propagates_host_failures() -> Result<(), String> {
    super::block_on(async {
        let (host, exports) = super::example_support::start::<Checkpoint>().await?;
        host.observe().actions.error = Some("cannot start git".into());
        assert_eq!(
            fire(&host, &exports, turn()).await,
            Ended::Failed("cannot start git".into())
        );
        host.observe().actions.error = None;
        leaf(
            &host,
            &json!({"type":"custom","id":"entry","parentId":null,"timestamp":"now","customType":"state"}),
        );
        returned(&fire(&host, &exports, result()).await);
        stdout(&host, "ref\u{85}");
        returned(&fire(&host, &exports, turn()).await);
        host.observe().catalog.data.has_ui = true;
        host.observe()
            .ui
            .control(&json!({"answer":"Yes"}).to_string());
        host.observe().actions.error = Some("cannot apply".into());
        assert_eq!(
            fire(&host, &exports, fork("entry")).await,
            Ended::Failed("cannot apply".into())
        );
        assert_eq!(
            super::ui_host::receipts(&host.observed().transcript).len(),
            1,
            "no notification after host failure"
        );
        host.observe().actions.error = None;
        host.observe()
            .ui
            .control(&json!({"answer":"Yes"}).to_string());
        returned(&fire(&host, &exports, fork("entry")).await);
        assert!(host.observed().transcript.contains(&format!(
            "action exec {}",
            json!(["git", ["stash", "apply", "ref\u{85}"], null])
        )));
        host.observe()
            .ui
            .control(&json!({"error":"selector unavailable"}).to_string());
        assert_eq!(
            fire(&host, &exports, fork("entry")).await,
            Ended::Failed("selector unavailable".into())
        );
        release(&host);
        Ok(())
    })
}

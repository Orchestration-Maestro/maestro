//! Artifact history replay and sandbox storage access through the public panel.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::artifact_presentation_support::Recorder;
use crate::support::run;
use maestro_agent::{Agent, AgentMessage, AgentOptions};
use maestro_app::SharedHandle;
use maestro_app::browser::components::messages::CustomAgentMessages;
use maestro_app::browser::components::sandbox::{
    artifacts_runtime_provider::{AgentLike, ArtifactsRuntimeProvider},
    sandbox_runtime_provider::SandboxRuntimeProvider,
};
use maestro_app::browser::tools::artifacts::ArtifactsPanel;
use maestro_app::browser::tools::prompts::{
    ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO, ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// Caller-defined record standing in for roles outside the browser conversation.
#[derive(Serialize, Deserialize)]
struct Foreign {
    /// Role used by the caller.
    role: String,
    /// Remaining recorded fields.
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}
impl maestro_agent::CustomAgentMessages for Foreign {
    fn role(&self) -> &str {
        &self.role
    }
}
/// Browser conversation entries with an open caller-defined branch.
type Message = AgentMessage<CustomAgentMessages<Foreign>>;

/// Recorded corpus with every reference correction already applied.
fn corpus() -> Value {
    serde_json::from_str(include_str!("fixtures/artifact_runtime_access.json")).unwrap()
}
/// Typed conversation entries from recorded rows.
fn messages(rows: &Value) -> Vec<Message> {
    serde_json::from_value(rows.clone()).unwrap()
}
/// A panel holding `files` in order, then cleared effects.
fn panel(files: &Value) -> (Arc<Recorder>, ArtifactsPanel) {
    let recorder = Arc::new(Recorder::default());
    let panel = ArtifactsPanel::new(recorder.clone());
    let tool = panel.tool();
    for file in files.as_array().unwrap() {
        // A user empties a file by replacing its whole content.
        let empty = file[1] == "";
        let content = if empty { json!("x") } else { file[1].clone() };
        let args = json!({"command":"create","filename":file[0],"content":content});
        run((tool.execute)("setup".into(), args, None, None)).unwrap();
        if empty {
            let args = json!({"command":"update","filename":file[0],"old_str":"x","new_str":""});
            run((tool.execute)("setup".into(), args, None, None)).unwrap();
        }
    }
    recorder.take();
    *recorder.clock.lock().unwrap() = 0;
    (recorder, panel)
}
/// Ordered filename and content pairs.
fn files(panel: &ArtifactsPanel) -> Value {
    let artifacts = panel.artifacts();
    let artifacts = artifacts.read().unwrap();
    artifacts
        .iter()
        .map(|(name, record)| json!([name, record.read().unwrap().content]))
        .collect()
}
/// The conversation slot of a thread-safe agent, as the provider retains it.
fn conversation(
    entries: Vec<Message>,
) -> (
    Agent<CustomAgentMessages<Foreign>>,
    SharedHandle<dyn AgentLike>,
) {
    let agent: Agent<CustomAgentMessages<Foreign>> = Agent::new(AgentOptions::default());
    agent.state().write().unwrap().set_messages(entries);
    let slot: SharedHandle<dyn AgentLike> = agent.state().clone();
    (agent, slot)
}
/// Serialized history entries of the agent's current conversation.
fn history(agent: &Agent<CustomAgentMessages<Foreign>>) -> Value {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    messages
        .iter()
        .map(|message| match message {
            AgentMessage::Custom(custom) => match &*custom.read().unwrap() {
                CustomAgentMessages::Artifact(record) => serde_json::to_value(record).unwrap(),
                _ => panic!("unexpected history entry"),
            },
            _ => panic!("unexpected history entry"),
        })
        .collect()
}
/// Deliver one sandbox message and collect its replies and returned failure.
fn deliver(
    provider: &dyn SandboxRuntimeProvider,
    message: Value,
    fail: bool,
) -> (Vec<Value>, Option<String>) {
    let replies = Arc::new(Mutex::new(Vec::new()));
    let output = Arc::clone(&replies);
    let result = run(provider.handle_message(
        message,
        Arc::new(move |value| {
            output.lock().unwrap().push(value);
            if fail {
                return Err(maestro_app::DiagnosticErrorInfo::error("reply failed"));
            }
            Ok(())
        }),
    ));
    let replies = replies.lock().unwrap().clone();
    (replies, result.err().map(|error| error.message))
}
/// Reconstruct rows owned by the corrected-behaviour tests, with the test that asserts them.
const RECONSTRUCT_ROWS: [(&str, &[&str]); 3] = [
    (
        "history_reconstruction_keeps_emptied_files",
        &["erase_all_content", "update_[\"old\",\"\"]"],
    ),
    (
        "history_reconstruction_replaces_edit_operands_as_literal_text",
        &[
            "update_[\"old\",\"new\"]",
            "update_[\"old\",\"$&-$`-$'-$ $\"]",
        ],
    ),
    (
        "history_reconstruction_skips_commands_the_panel_rejects",
        &[
            "missing_update_file",
            "update_[null,\"new\"]",
            "update_[\"old\",null]",
            "update_[\"missing\",\"new\"]",
            "update_[\"\",\"prefix\"]",
            "failed_duplicate_tool_result",
        ],
    ),
];
/// Recorded rows of one group that no corrected-behaviour test owns.
fn unowned_rows(group: &str, owned: &[(&str, &[&str])]) -> Vec<Value> {
    corpus()[group]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            let id = row["id"].as_str().unwrap();
            !owned.iter().any(|(_, ids)| ids.contains(&id))
        })
        .cloned()
        .collect()
}
/// Recorded rows of one group owned by the named test.
fn owned_rows(group: &str, owned: &[(&str, &[&str])], test: &str) -> Vec<Value> {
    let ids = owned.iter().find(|(name, _)| *name == test).map_or_else(
        || panic!("no recorded rows owned by {test}"),
        |(_, ids)| *ids,
    );
    named_rows(group, ids)
}
/// Recorded rows of one group named by `ids`.
fn named_rows(group: &str, ids: &[&str]) -> Vec<Value> {
    let rows = corpus()[group].as_array().unwrap().clone();
    ids.iter()
        .map(|id| {
            rows.iter()
                .find(|row| row["id"] == *id)
                .unwrap_or_else(|| panic!("no recorded row {id}"))
                .clone()
        })
        .collect()
}
/// Replay every supplied row through the public panel.
fn assert_reconstructed_rows(rows: Vec<Value>) {
    for row in rows {
        assert_reconstructed(&row);
    }
}
/// Deliver every supplied host row and assert its replies, history, files and effects.
fn assert_host_rows(rows: Vec<Value>) {
    for row in rows {
        assert_host_row(&row);
    }
}
/// Replay one recorded row and assert its files, selection, effects and emptied alias.
fn assert_reconstructed(row: &Value) {
    let (recorder, panel) = panel(&row["initial"]);
    let previous = panel.artifacts();
    run(panel.reconstruct_from_messages(&messages(&row["messages"])));
    let id = &row["id"];
    assert_eq!(files(&panel), row["files"], "{id} files");
    assert_eq!(json!(panel.active_filename()), row["active"], "{id} active");
    assert_eq!(json!(recorder.take()), row["effects"], "{id} effects");
    assert_eq!(
        json!(previous.read().unwrap().is_empty()),
        row["previous_alias_cleared"],
        "{id} previous alias"
    );
}

#[test]
fn history_reconstruction_replays_ordered_messages() {
    assert_reconstructed_rows(unowned_rows("reconstruct", &RECONSTRUCT_ROWS));
}

#[test]
fn history_reconstruction_keeps_emptied_files() {
    let rows = owned_rows(
        "reconstruct",
        &RECONSTRUCT_ROWS,
        "history_reconstruction_keeps_emptied_files",
    );
    assert_reconstructed_rows(rows);
}

#[test]
fn history_reconstruction_replaces_edit_operands_as_literal_text() {
    let rows = owned_rows(
        "reconstruct",
        &RECONSTRUCT_ROWS,
        "history_reconstruction_replaces_edit_operands_as_literal_text",
    );
    assert_reconstructed_rows(rows);
}

#[test]
fn history_reconstruction_skips_commands_the_panel_rejects() {
    let rows = owned_rows(
        "reconstruct",
        &RECONSTRUCT_ROWS,
        "history_reconstruction_skips_commands_the_panel_rejects",
    );
    assert_reconstructed_rows(rows);
}

#[test]
fn html_providers_snapshot_current_history() {
    for row in corpus()["html"].as_array().unwrap() {
        let (recorder, panel) = panel(&json!([]));
        let agent = (row["agent"] == true).then(|| conversation(messages(&row["messages"])));
        panel.set_agent(agent.as_ref().map(|(_, slot)| slot));
        let html = json!({"command":"create","filename":"view.html","content":"<p>"});
        run((panel.tool().execute)("c".into(), html, None, None)).unwrap();
        let observed = recorder.providers.lock().unwrap().clone();
        assert_eq!(
            observed,
            [row["providers"].clone(), row["providers"].clone()]
        );
        if let Some((agent, _)) = &agent {
            agent
                .state()
                .write()
                .unwrap()
                .set_messages(messages(&row["replacement"]));
            let rewrite = json!({"command":"rewrite","filename":"view.html","content":"<b>"});
            run((panel.tool().execute)("r".into(), rewrite, None, None)).unwrap();
            let observed = recorder.providers.lock().unwrap()[2..].to_vec();
            let after = row["providers_after_replacement"].clone();
            assert_eq!(observed, [after.clone(), after]);
        }
    }
}

#[test]
fn artifact_snapshots_read_live_store_and_detach() {
    for row in corpus()["snapshots"].as_array().unwrap() {
        let (_, panel) = panel(&row["entries"]);
        let provider = ArtifactsRuntimeProvider::new(&panel, None, true);
        let snapshot = SandboxRuntimeProvider::<Value>::get_data(&provider).unwrap();
        let later = json!({"command":"create","filename":"later.txt","content":"later"});
        run((panel.tool().execute)("l".into(), later, None, None)).unwrap();
        let next = SandboxRuntimeProvider::<Value>::get_data(&provider).unwrap();
        let keys: Vec<_> = snapshot["artifacts"].as_object().unwrap().keys().collect();
        assert_eq!(json!(keys), row["keys"]);
        for entry in row["entries"].as_array().unwrap() {
            assert_eq!(snapshot["artifacts"][entry[0].as_str().unwrap()], entry[1]);
        }
        assert_eq!(
            json!(snapshot["artifacts"].get("later.txt").is_none()),
            row["later_entry_detached"],
            "later entry detached"
        );
        assert_eq!(next["artifacts"]["later.txt"], "later");
    }
}

#[test]
fn artifact_guidance_selects_access_mode() {
    let (_, panel) = panel(&json!([]));
    for row in corpus()["descriptions"].as_array().unwrap() {
        let read_write = row["read_write"].as_bool().unwrap();
        let provider = ArtifactsRuntimeProvider::new(&panel, None, read_write);
        let description = SandboxRuntimeProvider::<Value>::get_description(&provider).unwrap();
        assert_eq!(description, row["description"]);
        let constant = if read_write {
            ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RW
        } else {
            ARTIFACTS_RUNTIME_PROVIDER_DESCRIPTION_RO
        };
        assert_eq!(description, constant);
    }
}
/// Host rows owned by the corrected-behaviour tests, with the test that asserts them.
const HOST_ROWS: [(&str, &[&str]); 4] = [
    (
        "artifact_read_only_access_refuses_writes",
        &[
            "create_false",
            "rewrite_false",
            "delete_false",
            "empty_false",
            "missing_delete_false",
            "list_false",
            "get_false",
        ],
    ),
    (
        "artifact_failed_commands_report_failures_without_history",
        &["empty_true", "missing_delete_true"],
    ),
    ("artifact_list_keeps_map_order", &["list_order"]),
    (
        "artifact_history_tracks_the_current_message_slot",
        &["history_slot_replacement"],
    ),
];
/// Deliver one recorded host row and assert its replies, history, files and effects.
fn assert_host_row(row: &Value) {
    let id = &row["id"];
    let (recorder, panel) = panel(&json!([["a.txt", "A"]]));
    let (agent, slot) = conversation(Vec::new());
    let slot = (row["agent"] == true).then_some(slot);
    let read_write = row["read_write"].as_bool().unwrap();
    let provider = ArtifactsRuntimeProvider::new(&panel, slot, read_write);
    let (replies, error) = deliver(
        &provider,
        row["message"].clone(),
        row["reply_fails"] == true,
    );
    assert_eq!(json!(replies), row["replies"], "{id} replies");
    assert_eq!(json!(error), row["error"], "{id} error");
    if row["agent"] == true {
        assert_eq!(history(&agent), row["history"], "{id} history");
    }
    assert_eq!(files(&panel), row["files"], "{id} files");
    assert_eq!(json!(recorder.take()), row["effects"], "{id} effects");
}

#[test]
fn artifact_host_operations_preserve_mutation_and_history_phase() {
    assert_host_rows(unowned_rows("host", &HOST_ROWS));
}

#[test]
fn artifact_read_only_access_refuses_writes() {
    let rows = owned_rows(
        "host",
        &HOST_ROWS,
        "artifact_read_only_access_refuses_writes",
    );
    assert_host_rows(rows);
}

#[test]
fn artifact_failed_commands_report_failures_without_history() {
    let rows = owned_rows(
        "host",
        &HOST_ROWS,
        "artifact_failed_commands_report_failures_without_history",
    );
    assert_host_rows(rows);
}

#[test]
fn artifact_list_keeps_map_order() {
    let rows = owned_rows("host", &HOST_ROWS, "artifact_list_keeps_map_order");
    let row = &rows[0];
    let (_, panel) = panel(&json!([["b.txt", "B"], ["a.txt", "A"], ["2", "two"]]));
    let provider = ArtifactsRuntimeProvider::new(&panel, None, true);
    let (replies, _) = deliver(
        &provider,
        json!({"type":"artifact-operation","action":"list"}),
        false,
    );
    assert_eq!(json!(replies), row["replies"]);
}

#[test]
fn artifact_history_tracks_the_current_message_slot() {
    let rows = owned_rows(
        "host",
        &HOST_ROWS,
        "artifact_history_tracks_the_current_message_slot",
    );
    let row = &rows[0];
    let (_, panel) = panel(&json!([]));
    let (agent, slot) = conversation(Vec::new());
    let provider = ArtifactsRuntimeProvider::new(&panel, Some(slot), true);
    let write = |content| json!({"type":"artifact-operation","action":"createOrUpdate","filename":"a.txt","content":content});
    let (first, _) = deliver(&provider, write("first"), false);
    let old = history(&agent);
    let replaced = std::mem::take(&mut *agent.state().read().unwrap().messages().write().unwrap());
    agent.state().write().unwrap().set_messages(Vec::new());
    let (second, _) = deliver(&provider, write("second"), false);
    assert_eq!(old, row["old"]);
    assert_eq!(history(&agent), row["current"]);
    assert_eq!(json!([first, second].concat()), row["replies"]);
    let current = std::mem::take(&mut *agent.state().read().unwrap().messages().write().unwrap());
    let restored = ArtifactsPanel::new(Arc::new(Recorder::default()));
    run(restored.reconstruct_from_messages(&[replaced, current].concat()));
    assert_eq!(files(&restored), row["replayed"]);
}

#[test]
fn dropping_the_panel_and_agent_frees_both() {
    let (_, panel) = panel(&json!([]));
    let (agent, slot) = conversation(Vec::new());
    let state = agent.state().clone();
    let freed = Arc::downgrade(&state);
    let store = Arc::downgrade(&panel.artifacts());
    panel.set_agent(Some(&slot));
    let tool = panel.tool();
    state
        .write()
        .unwrap()
        .set_tools(vec![Arc::new(RwLock::new(tool))]);
    drop(state);
    drop(agent);
    drop(slot);
    drop(panel);
    assert!(
        freed.upgrade().is_none(),
        "the retained panel and tool keep the conversation alive"
    );
    assert!(
        store.upgrade().is_none(),
        "the retained tool keeps the panel artifacts alive"
    );
}

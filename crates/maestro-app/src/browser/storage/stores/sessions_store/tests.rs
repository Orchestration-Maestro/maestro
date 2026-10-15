//! Deterministic preparation-phase clock witnesses.
use super::prepare;
use crate::browser::storage::{SessionData, SessionMetadata};
use maestro_agent::{Agent, AgentInitialState, AgentOptions, CustomAgentMessages};
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, RwLock};
/// Complete open message data used by these preparation inputs.
#[derive(Deserialize)]
struct Open {
    /// Caller role.
    role: String,
}
impl CustomAgentMessages for Open {
    fn role(&self) -> &str {
        &self.role
    }
}
/// Strict oracle envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Independently recorded clock sequence.
    clock: Vec<String>,
    /// All recorded operations.
    rows: Vec<Row>,
}
/// Strict recorded operation envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Owning public behavior.
    test: String,
    /// Operation operands.
    input: Value,
    /// Complete observations.
    expected: Value,
}
#[test]
fn session_alias_preparation_reads_clock_at_each_source_phase() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/browser_sessions.json"
    ))
    .unwrap();
    let mut consumed = 0;
    for row in corpus
        .rows
        .iter()
        .filter(|r| r.expected["result"]["result"].get("clockReads").is_some())
    {
        assert!(row.test.starts_with("session_alias_"));
        let agent = agent_from(&row.input["state"]);
        let metadata: Option<SessionMetadata> = row
            .input
            .get("metadata")
            .map(|v| serde_json::from_value(v.clone()).unwrap());
        let mut reads = 0;
        let (data, metadata) = {
            let state = agent.state().read().unwrap();
            prepare(
                row.input["id"].as_str().unwrap(),
                &state,
                metadata.as_ref(),
                row.input.get("title").and_then(Value::as_str),
                || {
                    let now = corpus.clock[reads].clone();
                    reads += 1;
                    now
                },
            )
        };
        assert_eq!(
            reads,
            usize::try_from(
                row.expected["result"]["result"]["clockReads"]
                    .as_u64()
                    .unwrap()
            )
            .unwrap()
        );
        let trace = row.expected["result"]["trace"].as_array().unwrap();
        let expected_data = &trace
            .iter()
            .find(|v| v[0] == "tx.set" && v[1] == "sessions")
            .unwrap()[3];
        let expected_metadata = &trace
            .iter()
            .find(|v| v[0] == "tx.set" && v[1] == "sessions-metadata")
            .unwrap()[3];
        check_prepared(&data, &metadata, expected_data, expected_metadata);
        let original = agent.state().read().unwrap().messages().clone();
        agent.state().write().unwrap().set_messages(Vec::new());
        assert!(Arc::ptr_eq(&data.messages, &original));
        assert!(Arc::ptr_eq(
            &data.model,
            &agent.state().read().unwrap().model
        ));
        consumed += 1;
    }
    assert_eq!(consumed, 41);
}
/// Observe every prepared field before any backend operation.
fn check_prepared(
    data: &SessionData<Open>,
    metadata: &SessionMetadata,
    expected_data: &Value,
    expected_metadata: &Value,
) {
    assert_eq!(data.id, expected_data["id"].as_str().unwrap());
    assert_eq!(data.title, expected_data["title"].as_str().unwrap());
    assert_eq!(
        data.created_at,
        expected_data["createdAt"].as_str().unwrap()
    );
    assert_eq!(
        data.last_modified,
        expected_data["lastModified"].as_str().unwrap()
    );
    assert_eq!(
        serde_json::to_value(&data.thinking_level).unwrap(),
        expected_data["thinkingLevel"]
    );
    let expected: SessionMetadata = serde_json::from_value(expected_metadata.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(metadata).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

/// Construct the preparation input through the actual state owner.
fn agent_from(value: &Value) -> Agent<Open> {
    Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(
                serde_json::from_value(value["model"].clone()).unwrap(),
            ))),
            messages: serde_json::from_value(value["messages"].clone()).unwrap(),
            thinking_level: Some(serde_json::from_value(value["thinkingLevel"].clone()).unwrap()),
            ..Default::default()
        },
        ..Default::default()
    })
}

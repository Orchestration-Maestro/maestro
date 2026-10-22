//! Application messages crossing between stored records and typed agent history.
#![cfg(test)]
use maestro_agent::AgentMessage;
use maestro_json::compact_json;
use maestro_session::messages::CustomAgentMessages;
use serde_json::json;

/// The typed history the session agent holds.
type History = Vec<AgentMessage<CustomAgentMessages>>;

#[test]
fn application_messages_round_trip_through_records() {
    let records = json!([
        {"role": "user", "content": "hello", "timestamp": 1},
        {"role": "bashExecution", "command": "ls", "output": "a", "exitCode": 0,
         "cancelled": false, "truncated": false, "timestamp": 2},
        {"role": "branchSummary", "summary": "left", "fromId": "e1", "timestamp": 3},
        {"role": "compactionSummary", "summary": "short", "tokensBefore": 40, "timestamp": 4},
        {"role": "custom", "customType": "note", "content": [{"type": "text", "text": "n"}],
         "display": true, "details": {"k": 1}, "timestamp": 5}
    ]);
    let history: History = serde_json::from_value(records.clone()).unwrap();
    let roles: Vec<&str> = history
        .iter()
        .map(|message| match message {
            AgentMessage::User(_) => "user",
            AgentMessage::Custom(custom) => match &*custom.read().unwrap() {
                CustomAgentMessages::BashExecution(_) => "bashExecution",
                CustomAgentMessages::BranchSummary(_) => "branchSummary",
                CustomAgentMessages::CompactionSummary(_) => "compactionSummary",
                CustomAgentMessages::Custom(_) => "custom",
                CustomAgentMessages::Other(never) => match *never {},
            },
            _ => "other",
        })
        .collect();
    assert_eq!(
        roles,
        [
            "user",
            "bashExecution",
            "branchSummary",
            "compactionSummary",
            "custom"
        ]
    );
    let written = serde_json::to_value(&history).unwrap();
    assert_eq!(
        compact_json(&written).unwrap(),
        compact_json(&records).unwrap()
    );
}

#[test]
fn unknown_application_role_is_refused() {
    let record = json!({"role": "notice", "text": "x", "timestamp": 1});
    let Err(error) = serde_json::from_value::<CustomAgentMessages>(record.clone()) else {
        panic!("an unknown role must not read");
    };
    assert_eq!(error.to_string(), "unknown message role `notice`");
    assert!(serde_json::from_value::<AgentMessage<CustomAgentMessages>>(record).is_err());
}

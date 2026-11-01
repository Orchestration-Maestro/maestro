//! Shared application records through guest public paths.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    Presence,
    messages::{
        BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage, CustomMessage,
    },
};
use serde_json::{Value, json};

#[test]
fn guest_reexports_preserve_message_wire_and_presence() {
    let shell: maestro_request::messages::BashExecutionMessage = BashExecutionMessage {
        command: "command".into(),
        output: "output".into(),
        exit_code: Presence::Missing,
        cancelled: false,
        truncated: true,
        full_output_path: Presence::Null,
        timestamp: 1.0,
        exclude_from_context: Presence::Present(false),
    };
    assert_eq!(
        serde_json::to_value(shell).unwrap(),
        json!({"role":"bashExecution","command":"command","output":"output","cancelled":false,"truncated":true,"fullOutputPath":null,"timestamp":1.0,"excludeFromContext":false})
    );
    let custom: maestro_request::messages::CustomMessage<crate::JsonText> = CustomMessage {
        custom_type: "kind".into(),
        content: crate::UserContent::Text("text".into()),
        display: false,
        details: Presence::Present(json!("details").to_string().into()),
        timestamp: 2.0,
    };
    assert_eq!(
        serde_json::to_value(custom).unwrap(),
        json!({"role":"custom","customType":"kind","content":"text","display":false,"details":"details","timestamp":2.0})
    );
    let branch: maestro_request::messages::BranchSummaryMessage = BranchSummaryMessage {
        summary: "branch".into(),
        from_id: "origin".into(),
        timestamp: 3.0,
    };
    assert_eq!(
        serde_json::to_value(branch).unwrap(),
        json!({"role":"branchSummary","summary":"branch","fromId":"origin","timestamp":3.0})
    );
    let compaction: maestro_request::messages::CompactionSummaryMessage =
        CompactionSummaryMessage {
            summary: "compact".into(),
            tokens_before: 4.5,
            timestamp: 4.0,
        };
    assert_eq!(
        serde_json::to_value(compaction).unwrap(),
        json!({"role":"compactionSummary","summary":"compact","tokensBefore":4.5,"timestamp":4.0})
    );
    assert_eq!(
        serde_json::to_string(&Presence::<String>::Missing)
            .unwrap_err()
            .to_string(),
        "missing value outside a property"
    );
    direct_record_decoding();
    generic_payload_decoding();
}

/// Standalone record decoding retains its existing serde policy.
fn direct_record_decoding() {
    let base = json!({"customType":"kind","content":"text","display":false,"timestamp":2.0});
    for role in [
        None,
        Some(json!("different")),
        Some(json!({"unexpected":true})),
    ] {
        let mut input = base.clone();
        if let Some(role) = role {
            input["role"] = role;
        }
        let record: CustomMessage = serde_json::from_value(input).unwrap();
        assert!(record.details.is_missing());
        assert_eq!(record.custom_type, "kind");
    }
    for (details, expected) in [
        (Value::Null, Presence::Null),
        (
            json!("value"),
            Presence::Present(json!("value").to_string().into()),
        ),
    ] {
        let mut input = base.clone();
        input["details"] = details;
        let record: CustomMessage = serde_json::from_value(input).unwrap();
        assert_eq!(record.details, expected);
    }
    let positional: CustomMessage =
        serde_json::from_value(json!(["kind", "text", false, "value", 2.0])).unwrap();
    assert_eq!(
        positional.details,
        Presence::Present(json!("value").to_string().into())
    );
    let mut input = base.clone();
    let mut nested = json!("unread");
    for _ in 0..150 {
        nested = json!({"nested":nested});
    }
    input["unknown"] = nested;
    let record: CustomMessage = serde_json::from_str(&input.to_string()).unwrap();
    assert_eq!(record.custom_type, "kind");
    assert!(serde_json::from_str::<CustomMessage>(r#"{"customType":"kind","customType":"duplicate","content":"text","display":false,"timestamp":2}"#).is_err());
    let mut input = base;
    input["content"] = json!(true);
    assert!(serde_json::from_value::<CustomMessage>(input).is_err());
}

/// Typed details do not require Default during decoding.
fn generic_payload_decoding() {
    /// An intentionally non-Default record.
    #[derive(serde::Deserialize)]
    struct Payload {
        /// Selected field.
        value: u8,
    }
    let base = json!({"customType":"typed","content":"text","display":false,"timestamp":0});
    let absent: maestro_request::messages::CustomMessage<Payload> =
        serde_json::from_value(base.clone()).unwrap();
    assert!(absent.details.is_missing());
    let mut input = base;
    input["details"] = json!({"value":7});
    let present: maestro_request::messages::CustomMessage<Payload> =
        serde_json::from_value(input).unwrap();
    assert!(matches!(
        present.details,
        Presence::Present(Payload { value: 7 })
    ));
}

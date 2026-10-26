//! Recorded browser messages through public conversion boundaries.
#![cfg(test)]
use crate::support;
use maestro_app::browser::components::messages::{
    ArtifactAction, ArtifactMessage, CustomAgentMessages, UserMessageWithAttachments,
    convert_attachments, default_convert_to_llm, is_artifact_message,
    is_user_message_with_attachments,
};
use maestro_app::browser::utils::attachment_utils::SharedAttachment;
use maestro_app::{AgentMessage, UserContent};
use maestro_models::Message;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};
use support::attachments::{corpus, equal};
use support::browser_records::{AttachmentRecord, attachments};

/// Caller-defined role accessor.
struct Foreign(String);
impl maestro_app::CustomAgentMessages for Foreign {
    fn role(&self) -> &str {
        &self.0
    }
}
/// Complete browser or ordinary recorded input.
#[derive(Deserialize)]
#[serde(tag = "role", deny_unknown_fields)]
enum RecordedMessage {
    /// Standard user.
    #[serde(rename = "user")]
    User(maestro_models::UserMessage),
    /// Standard assistant.
    #[serde(rename = "assistant")]
    Assistant(maestro_models::AssistantMessage),
    /// Standard tool result.
    #[serde(rename = "toolResult")]
    ToolResult(maestro_models::ToolResultMessage),
    /// Browser user.
    #[serde(rename = "user-with-attachments")]
    Browser {
        /// Text or blocks.
        content: UserContent,
        /// Recorded time.
        timestamp: f64,
        /// Optional attachments.
        attachments: Option<Vec<AttachmentRecord>>,
    },
    /// Browser artifact.
    #[serde(rename = "artifact")]
    Artifact {
        /// Action literal.
        action: String,
        /// Resource name.
        filename: String,
        /// Optional data.
        content: Option<String>,
        /// Optional title.
        title: Option<String>,
        /// Time spelling.
        timestamp: String,
    },
    /// Unknown role.
    #[serde(rename = "foreign")]
    Foreign {},
}
/// Construct complete typed input and prove no supplied field was discarded.
fn message(value: &Value) -> AgentMessage<CustomAgentMessages<Foreign>> {
    let input: RecordedMessage = serde_json::from_value(value.clone()).unwrap();
    let result = match input {
        RecordedMessage::User(value) => AgentMessage::User(Arc::new(RwLock::new(value))),
        RecordedMessage::Assistant(value) => AgentMessage::Assistant(Arc::new(RwLock::new(value))),
        RecordedMessage::ToolResult(value) => {
            AgentMessage::ToolResult(Arc::new(RwLock::new(value)))
        }
        RecordedMessage::Browser {
            content,
            timestamp,
            attachments,
        } => AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::UserWithAttachments(UserMessageWithAttachments {
                content,
                timestamp,
                attachments: attachments.map(|items| {
                    items
                        .into_iter()
                        .map(|value| Arc::new(RwLock::new(value.into_attachment())))
                        .collect()
                }),
            }),
        ))),
        RecordedMessage::Artifact {
            action,
            filename,
            content,
            title,
            timestamp,
        } => AgentMessage::Custom(Arc::new(RwLock::new(CustomAgentMessages::Artifact(
            ArtifactMessage {
                action: match action.as_str() {
                    "create" => ArtifactAction::Create,
                    "update" => ArtifactAction::Update,
                    "delete" => ArtifactAction::Delete,
                    _ => panic!("unexpected action"),
                },
                filename,
                content,
                title,
                timestamp,
            },
        )))),
        RecordedMessage::Foreign {} => AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::Other(Foreign("foreign".into())),
        ))),
    };
    equal(&serialized(&result), value);
    result
}
/// Serialize after constructing without changing the supplied fields.
fn serialized(message: &AgentMessage<CustomAgentMessages<Foreign>>) -> Value {
    if let Some(value) = message.to_message() {
        return serde_json::to_value(value).unwrap();
    }
    let AgentMessage::Custom(value) = message else {
        unreachable!()
    };
    let value = value.read().unwrap();
    match &*value {
        CustomAgentMessages::UserWithAttachments(value) => serde_json::to_value(value).unwrap(),
        CustomAgentMessages::Artifact(value) => serde_json::to_value(value).unwrap(),
        CustomAgentMessages::Other(value) => json!({"role":value.0}),
    }
}
/// Ordered history input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    /// Complete messages.
    messages: Vec<Value>,
}
/// Replay selected history rows.
fn history_rows(select: impl Fn(usize, &str) -> bool) {
    for (_, row) in corpus()
        .into_iter()
        .enumerate()
        .filter(|(index, row)| select(*index, &row.suite))
    {
        let input: History = serde_json::from_value(row.input).unwrap();
        let messages: Vec<_> = input.messages.iter().map(message).collect();
        let output = if row.suite == "agent_projection" {
            messages
                .iter()
                .filter_map(AgentMessage::to_message)
                .collect()
        } else {
            default_convert_to_llm(&messages)
        };
        equal(&serde_json::to_value(output).unwrap(), &row.expected);
    }
}
#[test]
fn attachment_blocks_keep_order_duplicates_and_document_text() {
    /// Complete conversion operands.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        /// Original records.
        attachments: Value,
    }
    for row in corpus()
        .into_iter()
        .filter(|row| row.suite == "attachment_blocks")
    {
        let input: Input = serde_json::from_value(row.input).unwrap();
        let values: Vec<_> = attachments(input.attachments.clone())
            .into_iter()
            .map(|value| Arc::new(RwLock::new(value)))
            .collect();
        equal(&serde_json::to_value(&values).unwrap(), &input.attachments);
        equal(
            &serde_json::to_value(convert_attachments(&values)).unwrap(),
            &row.expected,
        );
    }
}
#[test]
fn browser_record_serialization_keeps_roles_actions_and_optionality() {
    for row in corpus()
        .into_iter()
        .filter(|row| row.suite == "artifact_wire" || row.suite == "user_wire")
    {
        equal(&serialized(&message(&row.input)), &row.expected);
    }
}
#[test]
fn browser_history_preserves_standard_messages_and_filters_ui_entries() {
    history_rows(|index, suite| {
        suite == "agent_projection" || (suite == "history" && [79, 80, 81, 87].contains(&index))
    });
}
#[test]
fn browser_user_blocks_precede_attachment_blocks() {
    history_rows(|index, suite| suite == "history" && (82..=86).contains(&index));
}
#[test]
fn browser_message_predicates_compare_exact_roles() {
    /// Role-only guard operand.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        /// Exact role spelling.
        role: String,
    }
    let messages: Vec<_> = corpus()
        .into_iter()
        .filter(|row| row.suite == "history")
        .flat_map(|row| {
            serde_json::from_value::<History>(row.input)
                .unwrap()
                .messages
        })
        .collect();
    for role in ["user", "assistant", "toolResult"] {
        let value = message(messages.iter().find(|value| value["role"] == role).unwrap());
        assert!(matches!(
            (&value, role),
            (AgentMessage::User(_), "user")
                | (AgentMessage::Assistant(_), "assistant")
                | (AgentMessage::ToolResult(_), "toolResult")
        ));
        assert!(!is_user_message_with_attachments(&value));
        assert!(!is_artifact_message(&value));
    }
    for row in corpus().into_iter().filter(|row| row.suite == "guards") {
        let input: Input = serde_json::from_value(row.input).unwrap();
        let value = AgentMessage::Custom(Arc::new(RwLock::new(Foreign(input.role))));
        equal(
            &json!({"user":is_user_message_with_attachments(&value),"artifact":is_artifact_message(&value)}),
            &row.expected,
        );
    }
}
#[test]
fn browser_projection_reads_retained_state_without_mutating_it() {
    let retained = Arc::new(RwLock::new(maestro_models::UserMessage {
        content: UserContent::Text("before".into()),
        timestamp: 11.0,
    }));
    let weak = Arc::downgrade(&retained);
    let history: Vec<AgentMessage<CustomAgentMessages>> =
        vec![AgentMessage::User(retained.clone())];
    retained.write().unwrap().content = UserContent::Text("edited".into());
    let output = default_convert_to_llm(&history);
    assert!(
        matches!(&output[0],Message::User(value) if value.content == UserContent::Text("edited".into()))
    );
    assert_eq!(
        retained.read().unwrap().content,
        UserContent::Text("edited".into())
    );
    drop(history);
    drop(retained);
    assert!(weak.upgrade().is_none());
}

#[test]
fn browser_attachment_handles_survive_parent_removal_and_stay_live() {
    use maestro_app::browser::utils::attachment_utils::Attachment;
    let original = Arc::new(RwLock::new(Attachment {
        id: "original".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Image,
        file_name: "before.png".into(),
        mime_type: "image/png".into(),
        size: 3,
        content: "YWJj".into(),
        extracted_text: None,
        preview: None,
    }));
    let mut user = UserMessageWithAttachments {
        content: UserContent::Text(" untouched ".into()),
        timestamp: 1.,
        attachments: Some(vec![Arc::clone(&original), Arc::clone(&original)]),
    };
    let retained = user.attachments.as_ref().unwrap().clone();
    user.attachments = None;
    {
        let mut value = original.write().unwrap();
        value.file_name = "after.png".into();
        value.content = "ZGVm".into();
    }
    for handle in &retained {
        assert!(Arc::ptr_eq(handle, &original));
        assert_eq!(handle.read().unwrap().file_name, "after.png");
    }
    assert_eq!(
        serde_json::to_value(convert_attachments(&retained)).unwrap(),
        json!([
            {"type":"image", "data":"ZGVm", "mimeType":"image/png"},
            {"type":"image", "data":"ZGVm", "mimeType":"image/png"}
        ])
    );
}

#[test]
fn browser_shared_attachments_keep_plain_serialization_and_projection() {
    /// Complete recorded conversion cases.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Corpus {
        /// Independently recorded cases.
        rows: Vec<Row>,
    }
    /// One complete input and its observable outputs.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Row {
        /// Case name.
        id: String,
        /// Original browser record.
        input: Value,
        /// Serialized and projected results.
        expected: Expected,
    }
    /// Boundary observations from the source.
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        /// Saved plain objects.
        persisted: Value,
        /// Ordered attachment blocks.
        blocks: Value,
        /// Complete model input.
        projected: Value,
    }
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/attachments/shared_records.json")).unwrap();
    assert_eq!(corpus.rows.len(), 9);
    for row in corpus.rows {
        let _: RecordedMessage = serde_json::from_value(row.input.clone()).unwrap();
        let input: UserMessageWithAttachments = serde_json::from_value(row.input).unwrap();
        let retained: Vec<SharedAttachment> =
            input.attachments.iter().flatten().map(Arc::clone).collect();
        let expected = if row.id == "null" {
            let decoded: UserMessageWithAttachments =
                serde_json::from_value(row.expected.persisted).unwrap();
            serde_json::to_value(decoded).unwrap()
        } else {
            row.expected.persisted
        };
        equal(&serde_json::to_value(&input).unwrap(), &expected);
        equal(
            &serde_json::to_value(convert_attachments(
                input.attachments.as_deref().unwrap_or_default(),
            ))
            .unwrap(),
            &row.expected.blocks,
        );
        let history: [AgentMessage<CustomAgentMessages>; 1] = [AgentMessage::Custom(Arc::new(
            RwLock::new(CustomAgentMessages::UserWithAttachments(input)),
        ))];
        equal(
            &serde_json::to_value(default_convert_to_llm(&history)).unwrap(),
            &row.expected.projected,
        );
        assert_same_members(&history[0], &retained);
    }
}
/// The projected record still holds the handles a view retained: serialization and projection
/// read the shared members in place and copy none of them.
fn assert_same_members(message: &AgentMessage<CustomAgentMessages>, retained: &[SharedAttachment]) {
    let AgentMessage::Custom(record) = message else {
        panic!("expected the browser record");
    };
    let record = record.read().unwrap();
    let CustomAgentMessages::UserWithAttachments(user) = &*record else {
        panic!("expected an attachment-bearing user record");
    };
    let members = user.attachments.as_deref().unwrap_or_default();
    assert_eq!(members.len(), retained.len());
    assert!(
        members
            .iter()
            .zip(retained)
            .all(|(member, handle)| Arc::ptr_eq(member, handle))
    );
}

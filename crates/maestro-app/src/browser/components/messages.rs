//! Browser conversation records and model projection.
use crate::browser::utils::attachment_utils::{AttachmentType, SharedAttachment};
use crate::utils::lock::read;
use crate::{ImageContent, Message, TextContent, UserBlock, UserContent};
use maestro_agent::{AgentMessage, CustomAgentMessages as CustomMessage};

/// Convert images and nonempty document text into ordered model blocks.
#[must_use]
pub fn convert_attachments(attachments: &[SharedAttachment]) -> Vec<UserBlock> {
    attachments
        .iter()
        .filter_map(|attachment| {
            let attachment = read(attachment);
            match attachment.r#type {
                AttachmentType::Image => Some(UserBlock::Image(ImageContent {
                    data: attachment.content.clone(),
                    mime_type: attachment.mime_type.clone(),
                })),
                AttachmentType::Document => attachment
                    .extracted_text
                    .as_ref()
                    .filter(|text| !text.is_empty())
                    .map(|text| {
                        UserBlock::Text(TextContent::new(format!(
                            "\n\n[Document: {}]\n{text}",
                            attachment.file_name
                        )))
                    }),
            }
        })
        .collect()
}

/// Browser user input with optional attachments.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role", rename = "user-with-attachments")]
pub struct UserMessageWithAttachments {
    /// Text or existing model blocks.
    pub content: UserContent,
    /// Recorded milliseconds.
    pub timestamp: f64,
    /// Optional original attachments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<SharedAttachment>>,
}
impl CustomMessage for UserMessageWithAttachments {
    fn role(&self) -> &'static str {
        "user-with-attachments"
    }
}
/// Artifact edit classification.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactAction {
    /// Create an artifact.
    Create,
    /// Update an artifact.
    Update,
    /// Delete an artifact.
    Delete,
}
/// Browser artifact history entry, excluded from model input.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role", rename = "artifact")]
pub struct ArtifactMessage {
    /// Edit classification.
    pub action: ArtifactAction,
    /// Authored filename.
    pub filename: String,
    /// Optional artifact data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Optional display title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Recorded timestamp spelling.
    pub timestamp: String,
}
impl CustomMessage for ArtifactMessage {
    fn role(&self) -> &'static str {
        "artifact"
    }
}

/// Browser records with an open caller-defined message branch.
#[derive(serde::Deserialize)]
#[serde(tag = "role")]
pub enum CustomAgentMessages<C: CustomMessage = maestro_agent::NoCustomMessages> {
    /// User input with attachments.
    #[serde(rename = "user-with-attachments")]
    UserWithAttachments(UserMessageWithAttachments),
    /// Artifact history.
    #[serde(rename = "artifact")]
    Artifact(ArtifactMessage),
    /// Caller-defined entry.
    #[serde(untagged)]
    Other(C),
}
impl<C: CustomMessage + serde::Serialize> serde::Serialize for CustomAgentMessages<C> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::UserWithAttachments(value) => value.serialize(serializer),
            Self::Artifact(value) => value.serialize(serializer),
            Self::Other(value) => value.serialize(serializer),
        }
    }
}
impl<C: CustomMessage> CustomMessage for CustomAgentMessages<C> {
    fn role(&self) -> &str {
        match self {
            Self::UserWithAttachments(value) => value.role(),
            Self::Artifact(value) => value.role(),
            Self::Other(value) => value.role(),
        }
    }
}
/// Compare the exact attachment-user role.
#[must_use]
pub fn is_user_message_with_attachments<C: CustomMessage>(message: &AgentMessage<C>) -> bool {
    has_role(message, "user-with-attachments")
}
/// Compare the exact artifact role.
#[must_use]
pub fn is_artifact_message<C: CustomMessage>(message: &AgentMessage<C>) -> bool {
    has_role(message, "artifact")
}
/// Inspect custom role accessors without caching the value.
fn has_role<C: CustomMessage>(message: &AgentMessage<C>, role: &str) -> bool {
    match message {
        AgentMessage::Custom(value) => read(value).role() == role,
        _ => false,
    }
}
/// Expand browser users and snapshot standard messages in input order.
#[must_use]
pub fn default_convert_to_llm<C: CustomMessage>(
    messages: &[AgentMessage<CustomAgentMessages<C>>],
) -> Vec<Message> {
    messages
        .iter()
        .filter_map(|message| {
            let AgentMessage::Custom(value) = message else {
                return message.to_message();
            };
            let value = read(value);
            if let CustomAgentMessages::UserWithAttachments(user) = &*value {
                let mut content = match &user.content {
                    UserContent::Text(text) => {
                        vec![UserBlock::Text(TextContent::new(text.clone()))]
                    }
                    UserContent::Blocks(blocks) => blocks.clone(),
                };
                if let Some(attachments) = &user.attachments {
                    content.extend(convert_attachments(attachments));
                }
                return Some(Message::User(maestro_models::UserMessage {
                    content: UserContent::Blocks(content),
                    timestamp: user.timestamp,
                }));
            }
            None
        })
        .collect()
}

//! Owned user operands retain their original attachment handles.
#![cfg(test)]
use super::UserMessage;
use maestro_app::{AgentMessage, browser::components::messages::CustomAgentMessages};
use std::sync::{Arc, RwLock};

#[test]
fn user_owned_operand_keeps_content_and_attachment_handles() {
    let attachment = Arc::new(RwLock::new(crate::Attachment {
        id: "original".into(),
        r#type: maestro_app::browser::utils::attachment_utils::AttachmentType::Document,
        file_name: "note.txt".into(),
        mime_type: "text/plain".into(),
        size: 1,
        content: "YQ==".into(),
        extracted_text: None,
        preview: None,
    }));
    let message = AgentMessage::Custom(Arc::new(RwLock::new(
        CustomAgentMessages::UserWithAttachments(
            maestro_app::browser::components::messages::UserMessageWithAttachments {
                content: maestro_app::UserContent::Text("  kept text  ".into()),
                timestamp: 0.,
                attachments: Some(vec![Arc::clone(&attachment), Arc::clone(&attachment)]),
            },
        ),
    )));
    let mut row = UserMessage::<maestro_app::NoCustomMessages>::default();
    assert!(row.operand().is_err());
    row.message = Some(message);
    let operand = row.operand().unwrap();
    drop(row);
    assert_eq!(operand.text, "  kept text  ");
    assert_eq!(operand.attachments.len(), 2);
    assert!(
        operand
            .attachments
            .iter()
            .all(|handle| Arc::ptr_eq(handle, &attachment))
    );
    attachment.write().unwrap().file_name = "edited.txt".into();
    assert_eq!(
        operand.attachments[1].read().unwrap().file_name,
        "edited.txt"
    );
}

#[test]
fn assistant_preparation_has_linear_admission_and_no_tool_name_search() {
    use super::AssistantMessage;
    use maestro_app::{AssistantContent, ToolCall};
    use std::{cell::Cell, collections::HashSet, rc::Rc};
    for count in [8, 16, 32] {
        let chunks: Vec<_> = (0..count)
            .map(|index| {
                AssistantContent::ToolCall(ToolCall {
                    id: index.to_string(),
                    name: "selected".into(),
                    arguments: serde_json::json!({"index": index}),
                    thought_signature: None,
                })
            })
            .collect();
        let tools = Rc::new(RwLock::new(Vec::new()));
        let row = AssistantMessage::<serde_json::Value> {
            tools: Some(Rc::clone(&tools)),
            ..AssistantMessage::default()
        };
        // An unused collection must not be read merely to recover the call's existing name.
        let _exclusive = tools.write().unwrap();
        let visited = Cell::new(0);
        let emitted: Vec<_> = row
            .parts(chunks.iter().inspect(|_| visited.set(visited.get() + 1)))
            .collect();
        assert_eq!(visited.get(), count);
        assert_eq!(emitted.len(), count);
        assert!(
            emitted
                .iter()
                .zip(&chunks)
                .all(|(part, source)| std::ptr::eq(*part, source))
        );
        let repeated = Cell::new(0);
        for wanted in &chunks {
            assert!(
                chunks
                    .iter()
                    .inspect(|_| repeated.set(repeated.get() + 1))
                    .any(|chunk| std::ptr::eq(chunk, wanted))
            );
        }
        assert!(repeated.get() > count * 2);
        let pending = Rc::new(
            chunks
                .iter()
                .filter_map(|chunk| match chunk {
                    AssistantContent::ToolCall(call) => Some(call.id.clone()),
                    _ => None,
                })
                .collect::<HashSet<_>>(),
        );
        let row = AssistantMessage::<serde_json::Value> {
            pending_tool_calls: Some(pending),
            hide_pending_tool_calls: true,
            ..AssistantMessage::default()
        };
        assert_eq!(row.parts(chunks.iter()).count(), 0);
    }
}

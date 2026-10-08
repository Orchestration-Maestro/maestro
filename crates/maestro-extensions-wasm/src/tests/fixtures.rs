//! Typed values of the generated records, instantiated once per binding family.
//!
//! The component host and the author side generate the same records separately, so each
//! family module imports its own `events`, `models` and `session` modules and expands
//! [`define_fixtures`] to build values against them.
/// Defines the fixture builders against the `events`, `models` and `session` modules in
/// scope at the call site.
macro_rules! define_fixtures {
    () => {
        /// Compaction data for an agent that read a file and answered once.
        pub fn compact_data() -> events::SessionBeforeCompactEventData {
            events::SessionBeforeCompactEventData {
                preparation: session::CompactionPreparation {
                    first_kept_entry_id: "e2".to_owned(),
                    messages_to_summarize: vec![models::AgentMessage::User(models::UserMessage {
                        content: models::UserContent::Text("fix the build".to_owned()),
                        timestamp: 1_700_000_000_000.0,
                    })],
                    turn_prefix_messages: Vec::new(),
                    is_split_turn: false,
                    tokens_before: 1234.0,
                    previous_summary: None,
                    file_ops: session::FileOperations {
                        read: Vec::new(),
                        written: Vec::new(),
                        edited: Vec::new(),
                    },
                    settings: session::CompactionSettings {
                        enabled: true,
                        reserve_tokens: 16384.0,
                        keep_recent_tokens: 20000.0,
                    },
                },
                branch_entries: Vec::new(),
                custom_instructions: None,
            }
        }

        /// The text of content blocks, each followed by its signature when it has one.
        pub fn describe(content: &[models::ContentBlock]) -> String {
            let blocks: Vec<String> = content
                .iter()
                .map(|block| match block {
                    models::ContentBlock::Text(text) => match &text.text_signature {
                        Some(signature) => format!("{}/{signature}", text.text),
                        None => text.text.clone(),
                    },
                    models::ContentBlock::Image(image) => format!("image:{}", image.mime_type),
                })
                .collect();
            blocks.join(",")
        }

        /// A session info entry with the given identifier and no parent.
        pub fn info_entry(id: &str) -> session::SessionEntry {
            session::SessionEntry::SessionInfo(session::SessionInfoEntry {
                base: session::SessionEntryBase {
                    id: id.to_owned(),
                    parent_id: None,
                    timestamp: "2026-10-08T00:00:00Z".to_owned(),
                },
                name: Some(format!("session {id}")),
            })
        }

        /// A session context with no messages and no model.
        pub fn empty_session_context() -> session::SessionContext {
            session::SessionContext {
                messages: Vec::new(),
                thinking_level: "off".to_owned(),
                model: None,
            }
        }

        /// Credentials with neither key nor headers.
        pub fn empty_credentials() -> session::ResolvedRequestAuth {
            session::ResolvedRequestAuth::Ok(session::RequestCredentials {
                api_key: None,
                headers: None,
            })
        }

        /// A session start event for a fresh startup.
        pub fn session_start() -> events::SessionStartEvent {
            events::SessionStartEvent {
                reason: events::SessionStartReason::Startup,
                previous_session_file: None,
            }
        }

        /// An interactive input event.
        pub fn input(text: &str) -> events::InputEvent {
            events::InputEvent {
                text: text.to_owned(),
                images: None,
                source: events::InputSource::Interactive,
            }
        }

        /// A bash tool call of `command`.
        pub fn bash_call(command: &str) -> events::ToolCallEvent {
            events::ToolCallEvent::Bash(events::BashToolCallEvent {
                tool_call_id: "t1".to_owned(),
                input: events::BashToolInput {
                    command: command.to_owned(),
                    timeout: None,
                },
            })
        }
    };
}

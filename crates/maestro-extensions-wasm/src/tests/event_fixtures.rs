//! Typed values of every event family, built with every field set to a distinguishable value.
//!
//! Like the basic fixtures, they are expanded once per binding family. Text uses non-ASCII
//! characters, JSON text keeps present nulls, and optional fields are set so that dropping a
//! field anywhere along the way shows.

/// Defines the event fixture builders against the `events`, `models` and `session` modules in
/// scope at the call site.
macro_rules! define_event_fixtures {
    () => {
        /// Text content with a provider signature.
        pub fn text_block(text: &str) -> models::ContentBlock {
            models::ContentBlock::Text(models::TextContent {
                text: text.to_owned(),
                text_signature: Some(format!("sig-{text}")),
            })
        }

        /// A base64 image block.
        pub fn image_block() -> models::ContentBlock {
            models::ContentBlock::Image(models::ImageContent {
                data: "AAECAw==".to_owned(),
                mime_type: "image/png".to_owned(),
            })
        }

        /// An image as attached to prompts and inputs.
        pub fn image() -> models::ImageContent {
            models::ImageContent {
                data: "AQID".to_owned(),
                mime_type: "image/webp".to_owned(),
            }
        }

        /// A cost with every part distinct.
        pub fn cost() -> models::Cost {
            models::Cost {
                input: 0.5,
                output: 1.5,
                cache_read: 0.25,
                cache_write: 0.75,
                total: 3.0,
            }
        }

        /// Usage with every counter distinct.
        pub fn usage() -> models::Usage {
            models::Usage {
                input: 10.0,
                output: 20.0,
                cache_read: 3.0,
                cache_write: 4.0,
                total_tokens: 37.0,
                cost: cost(),
            }
        }

        /// An assistant message with every kind of content and every optional field.
        pub fn assistant() -> models::AssistantMessage {
            models::AssistantMessage {
                content: vec![
                    models::AssistantBlock::Text(models::TextContent {
                        text: "héllo \u{1f680}".to_owned(),
                        text_signature: Some("text-sig".to_owned()),
                    }),
                    models::AssistantBlock::Thinking(models::ThinkingContent {
                        thinking: "hm".to_owned(),
                        thinking_signature: Some("think-sig".to_owned()),
                        redacted: Some(true),
                    }),
                    models::AssistantBlock::ToolCall(tool_call()),
                ],
                api: "anthropic-messages".to_owned(),
                provider: "anthropic".to_owned(),
                model: "claude-x".to_owned(),
                response_model: Some("claude-x-2".to_owned()),
                response_id: Some("resp-1".to_owned()),
                diagnostics: Some(vec![models::AssistantMessageDiagnostic {
                    type_: "retry".to_owned(),
                    timestamp: 12.5,
                    error: Some(models::DiagnosticErrorInfo {
                        name: Some("Error".to_owned()),
                        message: "overloaded".to_owned(),
                        stack: Some("at x".to_owned()),
                        code: Some(models::DiagnosticCode::Number(529.0)),
                    }),
                    details: Some(r#"{"attempt":2,"note":null}"#.to_owned()),
                }]),
                usage: usage(),
                stop_reason: models::StopReason::ToolUse,
                error_message: Some("late warning".to_owned()),
                timestamp: 1_700_000_000_123.0,
            }
        }

        /// A tool call block with its signature.
        pub fn tool_call() -> models::ToolCall {
            models::ToolCall {
                id: "call-1".to_owned(),
                name: "read".to_owned(),
                arguments: r#"{"path":"é.txt","offset":0}"#.to_owned(),
                thought_signature: Some("thought".to_owned()),
            }
        }

        /// A tool result message.
        pub fn tool_result_message() -> models::ToolResultMessage {
            models::ToolResultMessage {
                tool_call_id: "call-1".to_owned(),
                tool_name: "read".to_owned(),
                content: vec![text_block("contents"), image_block()],
                details: Some("null".to_owned()),
                is_error: false,
                timestamp: 1_700_000_000_456.0,
            }
        }

        /// One message of every kind.
        pub fn messages() -> Vec<models::AgentMessage> {
            vec![
                models::AgentMessage::User(models::UserMessage {
                    content: models::UserContent::Blocks(vec![text_block("hi"), image_block()]),
                    timestamp: 1.0,
                }),
                models::AgentMessage::Assistant(assistant()),
                models::AgentMessage::ToolResult(tool_result_message()),
                models::AgentMessage::BashExecution(models::BashExecutionMessage {
                    command: "ls".to_owned(),
                    output: "a\nb".to_owned(),
                    exit_code: Some(-1),
                    cancelled: true,
                    truncated: true,
                    full_output_path: Some("/tmp/out".to_owned()),
                    timestamp: 2.0,
                    exclude_from_context: Some(false),
                }),
                models::AgentMessage::Custom(models::CustomMessage {
                    custom_type: "note".to_owned(),
                    content: models::UserContent::Text("n".to_owned()),
                    display: false,
                    details: Some("[]".to_owned()),
                    timestamp: 3.0,
                }),
                models::AgentMessage::BranchSummary(models::BranchSummaryMessage {
                    summary: "branch".to_owned(),
                    from_id: "e1".to_owned(),
                    timestamp: 4.0,
                }),
                models::AgentMessage::CompactionSummary(models::CompactionSummaryMessage {
                    summary: "compact".to_owned(),
                    tokens_before: 99.0,
                    timestamp: 5.0,
                }),
            ]
        }

        /// A model with every optional field, a completions compatibility record and routing.
        pub fn model() -> models::Model {
            models::Model {
                id: "m-1".to_owned(),
                name: "Model \u{e9}".to_owned(),
                api: "openai-completions".to_owned(),
                provider: "custom-provider".to_owned(),
                base_url: "https://example.test/v1".to_owned(),
                reasoning: true,
                thinking_level_map: Some(models::ThinkingLevelMap {
                    off: Some(models::LevelValue::Unsupported),
                    minimal: None,
                    low: Some(models::LevelValue::Mapped("l".to_owned())),
                    medium: None,
                    high: Some(models::LevelValue::Mapped("h".to_owned())),
                    xhigh: Some(models::LevelValue::Unsupported),
                }),
                input: vec![models::InputKind::Text, models::InputKind::Image],
                cost: models::ModelCost {
                    input: 1.0,
                    output: 2.0,
                    cache_read: 0.1,
                    cache_write: 0.2,
                },
                context_window: 200_000.0,
                max_tokens: 8192.5,
                headers: Some(vec![
                    ("x-a".to_owned(), "1".to_owned()),
                    ("x-b".to_owned(), String::new()),
                ]),
                compat: Some(models::ModelCompat::OpenaiCompletions(completions_compat())),
            }
        }

        /// A completions compatibility record with every field set.
        pub fn completions_compat() -> models::OpenaiCompletionsCompat {
            models::OpenaiCompletionsCompat {
                supports_store: Some(false),
                supports_developer_role: Some(true),
                supports_reasoning_effort: Some(false),
                supports_usage_in_streaming: Some(true),
                max_tokens_field: Some(models::MaxTokensField::MaxCompletionTokens),
                requires_tool_result_name: Some(true),
                requires_assistant_after_tool_result: Some(false),
                requires_thinking_as_text: Some(true),
                requires_reasoning_content_on_assistant_messages: Some(false),
                thinking_format: Some(models::ThinkingFormat::QwenChatTemplate),
                open_router_routing: Some(models::OpenRouterRouting {
                    allow_fallbacks: Some(false),
                    require_parameters: Some(true),
                    data_collection: Some(models::DataCollection::Deny),
                    zdr: Some(true),
                    enforce_distillable_text: Some(false),
                    order: Some(vec!["a".to_owned(), "b".to_owned()]),
                    only: Some(Vec::new()),
                    ignore: Some(vec!["c".to_owned()]),
                    quantizations: Some(vec!["fp8".to_owned()]),
                    sort: Some(models::SortPreference::Detail(models::SortDetail {
                        by: Some("price".to_owned()),
                        partition: Some(models::NullableString::Null),
                    })),
                    max_price: Some(models::MaxPrice {
                        prompt: Some(models::Price::Amount(0.5)),
                        completion: Some(models::Price::Text("1.5".to_owned())),
                        image: None,
                        audio: None,
                        request: Some(models::Price::Amount(0.0)),
                    }),
                    preferred_min_throughput: Some(models::PercentilePreference::Value(30.0)),
                    preferred_max_latency: Some(models::PercentilePreference::ByPercentile(
                        models::Percentiles {
                            p50: Some(1.0),
                            p75: None,
                            p90: Some(3.0),
                            p99: None,
                        },
                    )),
                }),
                vercel_gateway_routing: Some(models::VercelGatewayRouting {
                    only: Some(vec!["bedrock".to_owned()]),
                    order: None,
                }),
                zai_tool_stream: Some(true),
                supports_strict_mode: Some(false),
                cache_control_format: Some(models::CacheControlFormat::Anthropic),
                send_session_affinity_headers: Some(true),
                supports_long_cache_retention: Some(false),
            }
        }

        /// Source provenance with every field.
        pub fn source_info() -> session::SourceInfo {
            session::SourceInfo {
                path: "/p/skills/x".to_owned(),
                source: "npm:pkg".to_owned(),
                scope: session::SourceScope::Project,
                origin: session::SourceOrigin::Package,
                base_dir: Some("/p".to_owned()),
            }
        }

        /// One entry of every kind of the session tree, parented in a chain.
        pub fn entries() -> Vec<session::SessionEntry> {
            let base = |id: &str, parent: Option<&str>| session::SessionEntryBase {
                id: id.to_owned(),
                parent_id: parent.map(str::to_owned),
                timestamp: format!("2026-10-08T00:00:0{id}Z"),
            };
            vec![
                session::SessionEntry::Message(session::SessionMessageEntry {
                    base: base("1", None),
                    message: models::AgentMessage::Assistant(assistant()),
                }),
                session::SessionEntry::ThinkingLevelChange(session::ThinkingLevelChangeEntry {
                    base: base("2", Some("1")),
                    thinking_level: "high".to_owned(),
                }),
                session::SessionEntry::ModelChange(session::ModelChangeEntry {
                    base: base("3", Some("2")),
                    provider: "p".to_owned(),
                    model_id: "m".to_owned(),
                }),
                session::SessionEntry::Compaction(compaction_entry()),
                session::SessionEntry::BranchSummary(branch_summary_entry()),
                session::SessionEntry::Custom(session::CustomEntry {
                    base: base("6", Some("5")),
                    custom_type: "state".to_owned(),
                    data: Some("null".to_owned()),
                }),
                session::SessionEntry::CustomMessage(session::CustomMessageEntry {
                    base: base("7", Some("6")),
                    custom_type: "note".to_owned(),
                    content: models::UserContent::Text("body".to_owned()),
                    details: None,
                    display: true,
                }),
                session::SessionEntry::Label(session::LabelEntry {
                    base: base("8", Some("7")),
                    target_id: "1".to_owned(),
                    label: None,
                }),
                session::SessionEntry::SessionInfo(session::SessionInfoEntry {
                    base: base("9", Some("8")),
                    name: Some("named \u{e9}".to_owned()),
                }),
            ]
        }

        /// A compaction entry that an extension produced.
        pub fn compaction_entry() -> session::CompactionEntry {
            session::CompactionEntry {
                base: session::SessionEntryBase {
                    id: "4".to_owned(),
                    parent_id: Some("3".to_owned()),
                    timestamp: "2026-10-08T00:00:04Z".to_owned(),
                },
                summary: "summary \u{1f4dd}".to_owned(),
                first_kept_entry_id: "2".to_owned(),
                tokens_before: 12_345.5,
                details: Some(r#"{"files":[]}"#.to_owned()),
                from_hook: Some(true),
            }
        }

        /// A branch summary entry.
        pub fn branch_summary_entry() -> session::BranchSummaryEntry {
            session::BranchSummaryEntry {
                base: session::SessionEntryBase {
                    id: "5".to_owned(),
                    parent_id: Some("4".to_owned()),
                    timestamp: "2026-10-08T00:00:05Z".to_owned(),
                },
                from_id: "1".to_owned(),
                summary: "branch summary".to_owned(),
                details: None,
                from_hook: Some(false),
            }
        }

        /// System prompt options with every field.
        pub fn prompt_options() -> session::BuildSystemPromptOptions {
            session::BuildSystemPromptOptions {
                custom_prompt: Some("custom".to_owned()),
                selected_tools: Some(vec!["read".to_owned(), "bash".to_owned()]),
                tool_snippets: Some(vec![("read".to_owned(), "reads files".to_owned())]),
                prompt_guidelines: Some(vec!["be brief".to_owned()]),
                append_system_prompt: Some("appended".to_owned()),
                cwd: "/work".to_owned(),
                context_files: Some(vec![session::ContextFile {
                    path: "AGENTS.md".to_owned(),
                    content: "rules \u{e9}".to_owned(),
                }]),
                skills: Some(vec![session::Skill {
                    name: "skill".to_owned(),
                    description: "does things".to_owned(),
                    file_path: "/p/skills/x/SKILL.md".to_owned(),
                    base_dir: "/p/skills/x".to_owned(),
                    source_info: source_info(),
                    disable_model_invocation: true,
                }]),
            }
        }

        /// Bash tool truncation details.
        pub fn truncation() -> events::TruncationResult {
            events::TruncationResult {
                content: "cut".to_owned(),
                truncated: true,
                truncated_by: Some(events::TruncatedBy::Bytes),
                total_lines: 5_000,
                total_bytes: 5_000_000,
                output_lines: 2_000,
                output_bytes: 51_200,
                last_line_partial: true,
                first_line_exceeds_limit: false,
                max_lines: 2_000,
                max_bytes: 51_200,
            }
        }

        /// A resources discovery event.
        pub fn resources_discover(
            reason: events::ResourcesDiscoverReason,
        ) -> events::ResourcesDiscoverEvent {
            events::ResourcesDiscoverEvent {
                cwd: "/w\u{e9}".to_owned(),
                reason,
            }
        }

        /// A session start after a fork.
        pub fn forked_start() -> events::SessionStartEvent {
            events::SessionStartEvent {
                reason: events::SessionStartReason::Fork,
                previous_session_file: Some("/s/prev.jsonl".to_owned()),
            }
        }

        /// A switch event with an empty target.
        pub fn switch_to_empty() -> events::SessionBeforeSwitchEvent {
            events::SessionBeforeSwitchEvent {
                reason: events::SwitchReason::Resume,
                target_session_file: Some(String::new()),
            }
        }

        /// A fork event at an entry.
        pub fn fork_at() -> events::SessionBeforeForkEvent {
            events::SessionBeforeForkEvent {
                entry_id: "e9".to_owned(),
                position: session::ForkPosition::At,
            }
        }

        /// A compaction event carrying the persisted entry.
        pub fn session_compact() -> events::SessionCompactEvent {
            events::SessionCompactEvent {
                compaction_entry: compaction_entry(),
                from_extension: true,
            }
        }

        /// A shutdown caused by a session replacement.
        pub fn shutdown_for_new() -> events::SessionShutdownEvent {
            events::SessionShutdownEvent {
                reason: events::ShutdownReason::New,
                target_session_file: Some("/s/next.jsonl".to_owned()),
            }
        }

        /// Tree navigation preparation with every field.
        pub fn tree_preparation() -> events::TreePreparation {
            events::TreePreparation {
                target_id: "t".to_owned(),
                old_leaf_id: None,
                common_ancestor_id: Some("c".to_owned()),
                entries_to_summarize: entries(),
                user_wants_summary: true,
                custom_instructions: Some(String::new()),
                replace_instructions: Some(false),
                label: Some("mark".to_owned()),
            }
        }

        /// A tree navigation event with a summary entry.
        pub fn session_tree() -> events::SessionTreeEvent {
            events::SessionTreeEvent {
                new_leaf_id: Some("n".to_owned()),
                old_leaf_id: None,
                summary_entry: Some(branch_summary_entry()),
                from_extension: Some(false),
            }
        }

        /// The messages about to be sent to the model.
        pub fn context_event() -> events::ContextEvent {
            events::ContextEvent {
                messages: messages(),
            }
        }

        /// A provider request with a falsy payload.
        pub fn provider_request() -> events::BeforeProviderRequestEvent {
            events::BeforeProviderRequestEvent {
                payload: r#"{"stream":false,"n":0,"x":null}"#.to_owned(),
            }
        }

        /// A provider response with ordered headers.
        pub fn provider_response() -> events::AfterProviderResponseEvent {
            events::AfterProviderResponseEvent {
                status: 429,
                headers: vec![
                    ("retry-after".to_owned(), "1".to_owned()),
                    ("a".to_owned(), String::new()),
                ],
            }
        }

        /// A prompt about to start the agent.
        pub fn before_agent_start() -> events::BeforeAgentStartEvent {
            events::BeforeAgentStartEvent {
                prompt: "do it \u{1f680}".to_owned(),
                images: Some(vec![image()]),
                system_prompt: "system".to_owned(),
                system_prompt_options: prompt_options(),
            }
        }

        /// The end of an agent loop.
        pub fn agent_end() -> events::AgentEndEvent {
            events::AgentEndEvent {
                messages: messages(),
            }
        }

        /// The start of a turn.
        pub fn turn_start() -> events::TurnStartEvent {
            events::TurnStartEvent {
                turn_index: 4_000_000_000,
                timestamp: -1.5,
            }
        }

        /// The end of a turn.
        pub fn turn_end() -> events::TurnEndEvent {
            events::TurnEndEvent {
                turn_index: 2,
                message: models::AgentMessage::Assistant(assistant()),
                tool_results: vec![tool_result_message()],
            }
        }

        /// A message starting.
        pub fn message_start() -> events::MessageStartEvent {
            events::MessageStartEvent {
                message: models::AgentMessage::Assistant(assistant()),
            }
        }

        /// A message ending.
        pub fn message_end() -> events::MessageEndEvent {
            events::MessageEndEvent {
                message: models::AgentMessage::Custom(models::CustomMessage {
                    custom_type: "end".to_owned(),
                    content: models::UserContent::Text(String::new()),
                    display: true,
                    details: Some("false".to_owned()),
                    timestamp: 0.0,
                }),
            }
        }

        /// One update event of every kind of the assistant message stream.
        pub fn stream_events() -> Vec<models::AssistantMessageEvent> {
            let start = || models::ContentStart {
                content_index: 1,
                partial: assistant(),
            };
            let delta = |delta: &str| models::ContentDelta {
                content_index: 1,
                delta: delta.to_owned(),
                partial: assistant(),
            };
            let end = |content: &str| models::ContentEnd {
                content_index: 1,
                content: content.to_owned(),
                partial: assistant(),
            };
            vec![
                models::AssistantMessageEvent::Start(assistant()),
                models::AssistantMessageEvent::TextStart(start()),
                models::AssistantMessageEvent::TextDelta(delta("t\u{e9}")),
                models::AssistantMessageEvent::TextEnd(end("text")),
                models::AssistantMessageEvent::ThinkingStart(start()),
                models::AssistantMessageEvent::ThinkingDelta(delta("th")),
                models::AssistantMessageEvent::ThinkingEnd(end("thought")),
                models::AssistantMessageEvent::ToolcallStart(start()),
                models::AssistantMessageEvent::ToolcallDelta(delta("{\"p")),
                models::AssistantMessageEvent::ToolcallEnd(models::ToolCallEnd {
                    content_index: 2,
                    tool_call: tool_call(),
                    partial: assistant(),
                }),
                models::AssistantMessageEvent::Done(models::DoneEvent {
                    reason: models::DoneReason::Length,
                    message: assistant(),
                }),
                models::AssistantMessageEvent::Error(models::ErrorEvent {
                    reason: models::ErrorReason::Aborted,
                    error: assistant(),
                }),
            ]
        }

        /// A message update carrying one stream event.
        pub fn message_update(event: models::AssistantMessageEvent) -> events::MessageUpdateEvent {
            events::MessageUpdateEvent {
                message: models::AgentMessage::Assistant(assistant()),
                assistant_message_event: event,
            }
        }

        /// A tool starting.
        pub fn execution_start() -> events::ToolExecutionStartEvent {
            events::ToolExecutionStartEvent {
                tool_call_id: "x1".to_owned(),
                tool_name: "bash".to_owned(),
                args: r#"{"command":"ls"}"#.to_owned(),
            }
        }

        /// A tool reporting progress.
        pub fn execution_update() -> events::ToolExecutionUpdateEvent {
            events::ToolExecutionUpdateEvent {
                tool_call_id: "x1".to_owned(),
                tool_name: "bash".to_owned(),
                args: "null".to_owned(),
                partial_result: r#"{"content":[]}"#.to_owned(),
            }
        }

        /// A tool finishing with an error.
        pub fn execution_end() -> events::ToolExecutionEndEvent {
            events::ToolExecutionEndEvent {
                tool_call_id: "x1".to_owned(),
                tool_name: "bash".to_owned(),
                result: "0".to_owned(),
                is_error: true,
            }
        }

        /// A model selection restoring a previous one.
        pub fn model_select() -> events::ModelSelectEvent {
            events::ModelSelectEvent {
                model: model(),
                previous_model: None,
                source: events::ModelSelectSource::Restore,
            }
        }

        /// A thinking level change.
        pub fn thinking_select() -> events::ThinkingLevelSelectEvent {
            events::ThinkingLevelSelectEvent {
                level: models::ThinkingLevel::Xhigh,
                previous_level: models::ThinkingLevel::Off,
            }
        }

        /// A user bash command excluded from the context.
        pub fn user_bash() -> events::UserBashEvent {
            events::UserBashEvent {
                command: "echo \u{e9}".to_owned(),
                exclude_from_context: true,
                cwd: "/w".to_owned(),
            }
        }

        /// Input with images and a non-interactive source.
        pub fn rpc_input() -> events::InputEvent {
            events::InputEvent {
                text: String::new(),
                images: Some(Vec::new()),
                source: events::InputSource::Rpc,
            }
        }

        /// One call of every built-in tool plus a custom one.
        pub fn tool_calls() -> Vec<events::ToolCallEvent> {
            let mut calls = shell_calls();
            calls.extend(search_calls());
            calls.push(events::ToolCallEvent::Custom(events::CustomToolCallEvent {
                tool_call_id: "c8".to_owned(),
                tool_name: "mine".to_owned(),
                input: r#"{"nested":{"k":[1,null,false]}}"#.to_owned(),
            }));
            calls
        }

        /// Calls of the bash, read, edit and write tools.
        fn shell_calls() -> Vec<events::ToolCallEvent> {
            vec![
                events::ToolCallEvent::Bash(events::BashToolCallEvent {
                    tool_call_id: "c1".to_owned(),
                    input: events::BashToolInput {
                        command: "ls".to_owned(),
                        timeout: Some(0.5),
                    },
                }),
                events::ToolCallEvent::Read(events::ReadToolCallEvent {
                    tool_call_id: "c2".to_owned(),
                    input: events::ReadToolInput {
                        path: "a.txt".to_owned(),
                        offset: Some(-1.5),
                        limit: Some(10.0),
                    },
                }),
                events::ToolCallEvent::Edit(events::EditToolCallEvent {
                    tool_call_id: "c3".to_owned(),
                    input: events::EditToolInput {
                        path: "b".to_owned(),
                        edits: vec![events::ReplaceEdit {
                            old_text: "o".to_owned(),
                            new_text: String::new(),
                        }],
                    },
                }),
                events::ToolCallEvent::Write(events::WriteToolCallEvent {
                    tool_call_id: "c4".to_owned(),
                    input: events::WriteToolInput {
                        path: "c".to_owned(),
                        content: "x\u{85}y".to_owned(),
                    },
                }),
            ]
        }

        /// Calls of the grep, find and ls tools.
        fn search_calls() -> Vec<events::ToolCallEvent> {
            vec![
                events::ToolCallEvent::Grep(events::GrepToolCallEvent {
                    tool_call_id: "c5".to_owned(),
                    input: events::GrepToolInput {
                        pattern: "p".to_owned(),
                        path: Some(".".to_owned()),
                        glob: Some("*.rs".to_owned()),
                        ignore_case: Some(false),
                        literal: Some(true),
                        context: Some(2.0),
                        limit: Some(100.0),
                    },
                }),
                events::ToolCallEvent::Find(events::FindToolCallEvent {
                    tool_call_id: "c6".to_owned(),
                    input: events::FindToolInput {
                        pattern: "*".to_owned(),
                        path: None,
                        limit: Some(0.0),
                    },
                }),
                events::ToolCallEvent::Ls(events::LsToolCallEvent {
                    tool_call_id: "c7".to_owned(),
                    input: events::LsToolInput {
                        path: Some("/".to_owned()),
                        limit: None,
                    },
                }),
            ]
        }

        /// Fields every tool result shares.
        pub fn result_base(id: &str) -> events::ToolResultBase {
            events::ToolResultBase {
                tool_call_id: id.to_owned(),
                input: r#"{"a":"\u{e9}"}"#.to_owned(),
                content: vec![text_block("out"), image_block()],
                is_error: true,
            }
        }

        /// One result of every built-in tool plus a custom one.
        pub fn tool_results() -> Vec<events::ToolResultEvent> {
            let mut results = shell_results();
            results.extend(search_results());
            results.push(events::ToolResultEvent::Custom(
                events::CustomToolResultEvent {
                    base: result_base("r8"),
                    tool_name: "mine".to_owned(),
                    details: Some(r#"{"deep":{"x":null}}"#.to_owned()),
                },
            ));
            results
        }

        /// Results of the bash, read, edit and write tools.
        fn shell_results() -> Vec<events::ToolResultEvent> {
            vec![
                events::ToolResultEvent::Bash(events::BashToolResultEvent {
                    base: result_base("r1"),
                    details: Some(events::BashToolDetails {
                        truncation: Some(truncation()),
                        full_output_path: Some("/tmp/full".to_owned()),
                    }),
                }),
                events::ToolResultEvent::Read(events::ReadToolResultEvent {
                    base: result_base("r2"),
                    details: Some(events::ReadToolDetails { truncation: None }),
                }),
                events::ToolResultEvent::Edit(events::EditToolResultEvent {
                    base: result_base("r3"),
                    details: Some(events::EditToolDetails {
                        diff: "-a\n+b".to_owned(),
                        first_changed_line: Some(7),
                    }),
                }),
                events::ToolResultEvent::Write(result_base("r4")),
            ]
        }

        /// Results of the grep, find and ls tools.
        fn search_results() -> Vec<events::ToolResultEvent> {
            vec![
                events::ToolResultEvent::Grep(events::GrepToolResultEvent {
                    base: result_base("r5"),
                    details: Some(events::GrepToolDetails {
                        truncation: Some(truncation()),
                        match_limit_reached: Some(100.0),
                        lines_truncated: Some(false),
                    }),
                }),
                events::ToolResultEvent::Find(events::FindToolResultEvent {
                    base: result_base("r6"),
                    details: Some(events::FindToolDetails {
                        truncation: None,
                        result_limit_reached: Some(1000.0),
                    }),
                }),
                events::ToolResultEvent::Ls(events::LsToolResultEvent {
                    base: result_base("r7"),
                    details: None,
                }),
            ]
        }

        /// An event under a name the host does not define.
        pub fn custom_event() -> events::CustomEvent {
            events::CustomEvent {
                name: "my_event".to_owned(),
                payload: Some("null".to_owned()),
            }
        }
    };
}

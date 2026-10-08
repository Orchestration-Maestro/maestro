//! Typed events and results through the author facade.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentMessage, BashOperations, BashOperationsPort, ExtensionEvent, ExtensionEventResult,
    ExtensionHandler, ExtensionResult, SessionBeforeCompactEvent, SessionBeforeTreeEvent,
    SessionEvent, ToolCallEvent, ToolResultEvent, UserBashEventResult, UserContent,
};
use serde_json::{Value, json};
use support::controlled::{ControlledHost, Flag, Log};
use support::guest_family::{self as typed, events, models, session};
use support::{block_on, compaction_example};

/// Lines a handler wrote about the events it saw.
type Seen = Rc<RefCell<Vec<String>>>;

/// What a test handler decides for an event.
type Decision = ExtensionResult<Option<ExtensionEventResult>>;

/// The name of a unit variant: its debug text without the type path.
fn variant(value: &impl std::fmt::Debug) -> String {
    let text = format!("{value:?}");
    text.rsplit("::").next().unwrap_or_default().to_owned()
}

/// A host with a handler registered under `name`; the handler runs `body`.
fn host_with(
    name: &str,
    body: impl Fn(&mut ExtensionEvent) -> Decision + 'static,
) -> ControlledHost {
    let host = ControlledHost::new(Log::default());
    let handler: ExtensionHandler = Rc::new(move |event, _ctx| {
        let outcome = body(event);
        Box::pin(async move { outcome })
    });
    host.api().on(name, handler).unwrap();
    host
}

/// Delivers an event and returns the event as the handler left it with the wire result.
fn deliver_edited(
    host: &ControlledHost,
    name: &str,
    mut event: ExtensionEvent,
) -> (
    ExtensionEvent,
    ExtensionResult<Option<events::ExtensionEventResult>>,
) {
    let outcome = block_on(host.dispatch(name, &mut event));
    (
        event,
        outcome.map(|result| result.map(|result| result.into_wire().0)),
    )
}

/// Delivers an event and returns the wire result.
fn deliver(
    host: &ControlledHost,
    name: &str,
    event: ExtensionEvent,
) -> ExtensionResult<Option<events::ExtensionEventResult>> {
    deliver_edited(host, name, event).1
}

/// The event as a session event of the given kind.
fn session_event(event: SessionEvent) -> ExtensionEvent {
    ExtensionEvent::Session(event)
}

#[test]
fn maestro_resource_events_keep_paths_and_reasons() {
    let host = host_with("resources_discover", |event| {
        let ExtensionEvent::ResourcesDiscover(discover) = event else {
            return Ok(None);
        };
        let reason = variant(&discover.reason);
        Ok(Some(ExtensionEventResult::ResourcesDiscover(
            events::ResourcesDiscoverResult {
                skill_paths: Some(vec![format!("{}/{reason}", discover.cwd)]),
                prompt_paths: Some(Vec::new()),
                theme_paths: None,
            },
        )))
    });
    for (reason, suffix) in [
        (events::ResourcesDiscoverReason::Startup, "Startup"),
        (events::ResourcesDiscoverReason::Reload, "Reload"),
    ] {
        let event = ExtensionEvent::ResourcesDiscover(typed::resources_discover(reason));
        assert_eq!(
            deliver(&host, "resources_discover", event),
            Ok(Some(events::ExtensionEventResult::ResourcesDiscover(
                events::ResourcesDiscoverResult {
                    skill_paths: Some(vec![format!("/w\u{e9}/{suffix}")]),
                    prompt_paths: Some(Vec::new()),
                    theme_paths: None,
                }
            ))),
            "populated, empty and absent path lists stay distinct"
        );
    }
}

/// Notes a session event and returns the verdict a handler gives it.
fn judge_session(seen: &Seen, event: &SessionEvent) -> Option<ExtensionEventResult> {
    match event {
        SessionEvent::Start(start) => {
            seen.borrow_mut().push(format!(
                "start {} {:?}",
                variant(&start.reason),
                start.previous_session_file
            ));
            None
        }
        SessionEvent::BeforeSwitch(switch) => {
            seen.borrow_mut().push(format!(
                "switch {} {:?}",
                variant(&switch.reason),
                switch.target_session_file
            ));
            Some(ExtensionEventResult::SessionBeforeSwitch(
                events::SessionBeforeSwitchResult { cancel: Some(true) },
            ))
        }
        SessionEvent::BeforeFork(fork) => {
            seen.borrow_mut().push(format!(
                "fork {} {}",
                fork.entry_id,
                variant(&fork.position)
            ));
            Some(ExtensionEventResult::SessionBeforeFork(
                events::SessionBeforeForkResult {
                    cancel: Some(false),
                    skip_conversation_restore: Some(true),
                },
            ))
        }
        SessionEvent::Shutdown(shutdown) => {
            seen.borrow_mut().push(format!(
                "shutdown {} {:?}",
                variant(&shutdown.reason),
                shutdown.target_session_file
            ));
            None
        }
        _ => None,
    }
}

#[test]
fn maestro_session_events_keep_reasons_and_optional_paths() {
    let seen = Seen::default();
    let record = Rc::clone(&seen);
    let host = host_with("session", move |event| match event {
        ExtensionEvent::Session(inner) => Ok(judge_session(&record, inner)),
        _ => Ok(None),
    });
    let start = session_event(SessionEvent::Start(typed::forked_start()));
    assert_eq!(deliver(&host, "session", start), Ok(None));
    let switch = session_event(SessionEvent::BeforeSwitch(typed::switch_to_empty()));
    assert_eq!(
        deliver(&host, "session", switch),
        Ok(Some(events::ExtensionEventResult::SessionBeforeSwitch(
            events::SessionBeforeSwitchResult { cancel: Some(true) }
        )))
    );
    let fork = session_event(SessionEvent::BeforeFork(typed::fork_at()));
    assert_eq!(
        deliver(&host, "session", fork),
        Ok(Some(events::ExtensionEventResult::SessionBeforeFork(
            events::SessionBeforeForkResult {
                cancel: Some(false),
                skip_conversation_restore: Some(true),
            }
        ))),
        "an explicit false is not omission"
    );
    let shutdown = session_event(SessionEvent::Shutdown(typed::shutdown_for_new()));
    assert_eq!(deliver(&host, "session", shutdown), Ok(None));
    assert_eq!(
        *seen.borrow(),
        [
            r#"start Fork Some("/s/prev.jsonl")"#,
            r#"switch Resume Some("")"#,
            "fork e9 At",
            r#"shutdown New Some("/s/next.jsonl")"#,
        ]
    );
}

/// The user message the compaction tests summarize.
fn user_text(text: &str) -> AgentMessage {
    AgentMessage::User(models::UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 1.0,
    })
}

/// A host running the compaction example against a scripted session and catalog.
fn compaction_host() -> ControlledHost {
    let host = ControlledHost::new(Log::default());
    compaction_example::register_compaction_example(&host.api()).unwrap();
    host.script.reply("get_entries", Ok(typed::entries()));
    host.script
        .reply("find", Ok(None::<maestro_extensions_wasm::Model>));
    let reader = maestro_extensions_wasm::ReadonlySessionManager::new(Rc::new(
        support::controlled::Writer(host.script.clone()),
    ));
    let registry = maestro_extensions_wasm::ModelRegistry::new(Rc::new(
        support::controlled::Catalog(host.script.clone()),
    ));
    host.script.reply("session_manager", Ok(reader));
    host.script.reply("model_registry", Ok(registry));
    host
}

#[test]
fn maestro_compaction_handler_uses_preparation_and_returns_content() {
    let host = compaction_host();
    let mut data = typed::compact_data();
    data.preparation.messages_to_summarize = vec![user_text("first"), user_text("second \u{e9}")];
    data.preparation.tokens_before = 4321.5;
    data.custom_instructions = Some("focus".to_owned());
    let flag = Flag::default();
    let wrapper = SessionBeforeCompactEvent {
        data,
        signal: flag.signal(),
    };
    let mut event = session_event(SessionEvent::BeforeCompact(wrapper));
    let handler = host.handler("session_before_compact").unwrap();
    let result = block_on(handler(
        &mut event,
        ControlledHost::context_with(&host.script),
    ))
    .unwrap();
    let Some(ExtensionEventResult::SessionBeforeCompact(verdict)) = result else {
        panic!("the handler returned no compaction verdict");
    };
    assert_eq!(
        verdict.cancel, None,
        "no verdict to cancel is distinct from cancel: false"
    );
    let compaction = verdict.compaction.unwrap();
    assert_eq!(
        compaction.summary,
        "User requests:\n- first\n- second \u{e9}"
    );
    assert_eq!(compaction.first_kept_entry_id, "e2");
    assert!((compaction.tokens_before - 4321.5).abs() < f64::EPSILON);
    assert_eq!(
        compaction.details.as_deref(),
        Some(r#"{"entries":9,"model":null}"#)
    );
    let calls = host.script_calls();
    assert!(calls.iter().any(|call| call == "get_entries()"));
    assert!(calls.iter().any(|call| call.starts_with("find(")));
    assert!(!flag.signal().aborted());
}

#[test]
fn maestro_compact_event_exposes_persisted_entry() {
    let host = compaction_host();
    for from_extension in [true, false] {
        let mut compact = typed::session_compact();
        compact.from_extension = from_extension;
        let outcome = deliver(
            &host,
            "session_compact",
            session_event(SessionEvent::Compact(compact)),
        );
        assert_eq!(
            outcome,
            Err(format!("summary \u{1f4dd} 12345.5 {from_extension}")),
            "the handler read the persisted entry and the extension-origin flag"
        );
    }
    let entry = typed::compaction_entry();
    assert_eq!(entry.base.parent_id.as_deref(), Some("3"));
    assert_eq!(entry.first_kept_entry_id, "2");
    assert_eq!(entry.details.as_deref(), Some(r#"{"files":[]}"#));
    assert_eq!(entry.from_hook, Some(true));
}

#[test]
fn maestro_tree_events_keep_nullable_ids_and_summary() {
    let verdict = events::SessionBeforeTreeResult {
        cancel: Some(true),
        summary: Some(events::TreeSummary {
            summary: "kept".into(),
            details: Some("null".into()),
        }),
        custom_instructions: Some(String::new()),
        replace_instructions: Some(false),
        label: Some("new label".into()),
    };
    let answer = events::SessionBeforeTreeResult { ..verdict.clone() };
    let host = host_with("session_before_tree", move |event| {
        let ExtensionEvent::Session(SessionEvent::BeforeTree(tree)) = event else {
            return Ok(None);
        };
        tree.preparation.label = None;
        let cancel = Some(tree.signal.aborted());
        let answer = events::SessionBeforeTreeResult {
            cancel,
            ..answer.clone()
        };
        Ok(Some(ExtensionEventResult::SessionBeforeTree(answer)))
    });
    let flag = Flag::default();
    flag.abort();
    let data = events::SessionBeforeTreeEventData {
        preparation: typed::tree_preparation(),
    };
    let wrapper = SessionBeforeTreeEvent {
        data,
        signal: flag.signal(),
    };
    let (event, result) = deliver_edited(
        &host,
        "session_before_tree",
        session_event(SessionEvent::BeforeTree(wrapper)),
    );
    let ExtensionEvent::Session(SessionEvent::BeforeTree(tree)) = &event else {
        panic!("not a tree event")
    };
    assert_eq!(tree.preparation.old_leaf_id, None, "a null leaf stays null");
    assert_eq!(tree.preparation.common_ancestor_id.as_deref(), Some("c"));
    assert_eq!(tree.preparation.entries_to_summarize.len(), 9);
    assert_eq!(
        tree.preparation.label, None,
        "an edit through the wrapper reaches the data"
    );
    assert!(tree.signal.aborted(), "the signal stays with the event");
    assert_eq!(
        result,
        Ok(Some(events::ExtensionEventResult::SessionBeforeTree(
            verdict
        )))
    );
    let after = typed::session_tree();
    assert_eq!(after.old_leaf_id, None);
    assert_eq!(
        after.summary_entry.map(|entry| entry.from_hook),
        Some(Some(false))
    );
}

/// Notes an agent event and returns the result a handler gives it.
fn judge_agent(seen: &Seen, event: &mut ExtensionEvent) -> Option<ExtensionEventResult> {
    match event {
        ExtensionEvent::Context(context) => {
            seen.borrow_mut()
                .push(format!("context {}", context.messages.len()));
            let messages = context.messages.drain(..2).collect();
            Some(ExtensionEventResult::Context(events::ContextEventResult {
                messages: Some(messages),
            }))
        }
        ExtensionEvent::BeforeAgentStart(start) => {
            let options = &start.system_prompt_options;
            let files = options
                .context_files
                .as_ref()
                .map(|files| files[0].content.clone());
            let skill = options.skills.as_ref().map(|skills| skills[0].name.clone());
            seen.borrow_mut().push(format!(
                "start {} {} {:?} {:?} {files:?} {skill:?}",
                start.prompt,
                start.images.as_ref().map_or(0, Vec::len),
                options.selected_tools,
                options.tool_snippets,
            ));
            Some(ExtensionEventResult::BeforeAgentStart(
                events::BeforeAgentStartEventResult {
                    message: Some(models::CustomMessageInput {
                        custom_type: "note".into(),
                        content: UserContent::Text("hi".into()),
                        display: false,
                        details: None,
                    }),
                    system_prompt: Some(String::new()),
                },
            ))
        }
        ExtensionEvent::MessageEnd(end) => {
            seen.borrow_mut().push("message-end".into());
            Some(ExtensionEventResult::MessageEnd(
                events::MessageEndEventResult {
                    message: Some(end.message.clone()),
                },
            ))
        }
        other => {
            seen.borrow_mut().push(note_agent(other));
            None
        }
    }
}

/// A line describing an agent event that carries no result.
fn note_agent(event: &ExtensionEvent) -> String {
    match event {
        ExtensionEvent::AgentEnd(end) => format!("end {}", end.messages.len()),
        ExtensionEvent::TurnStart(turn) => format!("turn {} {}", turn.turn_index, turn.timestamp),
        ExtensionEvent::TurnEnd(turn) => {
            format!("turn-end {} {}", turn.turn_index, turn.tool_results.len())
        }
        _ => "noted".to_owned(),
    }
}

/// Delivers the agent events that carry no result and the message end that replaces a message.
fn deliver_notifications(host: &ControlledHost) {
    for event in [
        ExtensionEvent::AgentStart,
        ExtensionEvent::AgentEnd(typed::agent_end()),
        ExtensionEvent::TurnStart(typed::turn_start()),
        ExtensionEvent::TurnEnd(typed::turn_end()),
        ExtensionEvent::MessageStart(typed::message_start()),
    ] {
        assert_eq!(deliver(host, "agent", event), Ok(None));
    }
    let ended = deliver(
        host,
        "agent",
        ExtensionEvent::MessageEnd(typed::message_end()),
    );
    let Ok(Some(events::ExtensionEventResult::MessageEnd(ended))) = ended else {
        panic!("no message end result")
    };
    assert!(matches!(
        ended.message,
        Some(models::AgentMessage::Custom(_))
    ));
}

#[test]
fn maestro_agent_events_keep_messages_and_system_prompt_options() {
    let seen = Seen::default();
    let record = Rc::clone(&seen);
    let host = host_with("agent", move |event| Ok(judge_agent(&record, event)));
    let (event, result) = deliver_edited(
        &host,
        "agent",
        ExtensionEvent::Context(typed::context_event()),
    );
    let ExtensionEvent::Context(context) = event else {
        panic!("not a context event")
    };
    assert_eq!(
        context.messages.len(),
        5,
        "the handler's edit of the message list is visible"
    );
    let Ok(Some(events::ExtensionEventResult::Context(replaced))) = result else {
        panic!("no context result")
    };
    assert_eq!(replaced.messages.map(|messages| messages.len()), Some(2));
    let start = deliver(
        &host,
        "agent",
        ExtensionEvent::BeforeAgentStart(Box::new(typed::before_agent_start())),
    );
    let Ok(Some(events::ExtensionEventResult::BeforeAgentStart(start))) = start else {
        panic!("no start result")
    };
    assert_eq!(
        start.system_prompt.as_deref(),
        Some(""),
        "an empty replacement prompt is present"
    );
    assert_eq!(start.message.map(|message| message.display), Some(false));
    deliver_notifications(&host);
    assert_eq!(
        *seen.borrow(),
        [
            "context 7",
            "start do it \u{1f680} 1 Some([\"read\", \"bash\"]) Some([(\"read\", \"reads files\")]) \
             Some(\"rules \u{e9}\") Some(\"skill\")",
            "noted",
            "end 7",
            "turn 4000000000 -1.5",
            "turn-end 2 1",
            "noted",
            "message-end",
        ]
    );
}

/// A line describing one event of the assistant message stream.
fn stream_label(event: &models::AssistantMessageEvent) -> String {
    use models::AssistantMessageEvent as Stream;
    match event {
        Stream::Start(_) => "start".to_owned(),
        Stream::TextStart(e) | Stream::ThinkingStart(e) | Stream::ToolcallStart(e) => {
            format!("start@{}", e.content_index)
        }
        Stream::TextDelta(e) | Stream::ThinkingDelta(e) | Stream::ToolcallDelta(e) => {
            format!("delta {}", e.delta)
        }
        Stream::TextEnd(e) | Stream::ThinkingEnd(e) => format!("end {}", e.content),
        Stream::ToolcallEnd(e) => format!(
            "tool {} {:?}",
            e.tool_call.arguments, e.tool_call.thought_signature
        ),
        Stream::Done(e) => format!(
            "done {} {:?}",
            variant(&e.reason),
            e.message.diagnostics.as_ref().map(Vec::len)
        ),
        Stream::Error(e) => format!("error {} {:?}", variant(&e.reason), e.error.response_model),
    }
}

#[test]
fn maestro_stream_events_keep_all_variants_and_signatures() {
    let seen = Seen::default();
    let record = Rc::clone(&seen);
    let host = host_with("stream", move |event| {
        let line = match event {
            ExtensionEvent::MessageUpdate(update) => stream_label(&update.assistant_message_event),
            ExtensionEvent::ToolExecutionStart(e) => format!("exec {} {}", e.tool_name, e.args),
            ExtensionEvent::ToolExecutionUpdate(e) => {
                format!("progress {} {}", e.args, e.partial_result)
            }
            ExtensionEvent::ToolExecutionEnd(e) => format!("finished {} {}", e.result, e.is_error),
            _ => return Ok(None),
        };
        record.borrow_mut().push(line);
        Ok(None)
    });
    for stream in typed::stream_events() {
        let event = ExtensionEvent::MessageUpdate(Box::new(typed::message_update(stream)));
        assert_eq!(deliver(&host, "stream", event), Ok(None));
    }
    for event in [
        ExtensionEvent::ToolExecutionStart(typed::execution_start()),
        ExtensionEvent::ToolExecutionUpdate(typed::execution_update()),
        ExtensionEvent::ToolExecutionEnd(typed::execution_end()),
    ] {
        assert_eq!(deliver(&host, "stream", event), Ok(None));
    }
    assert_eq!(
        *seen.borrow(),
        [
            "start",
            "start@1",
            "delta t\u{e9}",
            "end text",
            "start@1",
            "delta th",
            "end thought",
            "start@1",
            "delta {\"p",
            "tool {\"path\":\"\u{e9}.txt\",\"offset\":0} Some(\"thought\")",
            "done Length Some(1)",
            "error Aborted Some(\"claude-x-2\")",
            "exec bash {\"command\":\"ls\"}",
            "progress null {\"content\":[]}",
            "finished 0 true",
        ]
    );
    let assistant = typed::assistant();
    assert!(
        matches!(&assistant.content[0], models::AssistantBlock::Text(t) if t.text_signature.as_deref() == Some("text-sig"))
    );
    assert!(
        matches!(&assistant.content[1], models::AssistantBlock::Thinking(t) if t.redacted == Some(true))
    );
    assert_eq!(assistant.usage.cost.total.to_bits(), 3.0_f64.to_bits());
}

/// Checks the routing preferences of the model fixture.
fn assert_routing(routing: &models::OpenRouterRouting) {
    let detail = models::SortDetail {
        by: Some("price".into()),
        partition: Some(models::NullableString::Null),
    };
    assert_eq!(
        routing.sort,
        Some(models::SortPreference::Detail(detail)),
        "a present null partition is distinct from an omitted one"
    );
    let price = routing.max_price.as_ref().unwrap();
    assert_eq!(price.prompt, Some(models::Price::Amount(0.5)));
    assert_eq!(price.completion, Some(models::Price::Text("1.5".into())));
    assert_eq!(routing.only, Some(Vec::new()));
    let percentiles = models::Percentiles {
        p50: Some(1.0),
        p75: None,
        p90: Some(3.0),
        p99: None,
    };
    assert_eq!(
        routing.preferred_max_latency,
        Some(models::PercentilePreference::ByPercentile(percentiles))
    );
}

/// Checks the model fixture's thinking levels, headers and compatibility record.
fn assert_model(model: &models::Model) {
    let map = model.thinking_level_map.as_ref().unwrap();
    assert_eq!(map.off, Some(models::LevelValue::Unsupported));
    assert_eq!(
        map.minimal, None,
        "an omitted level uses the provider default"
    );
    assert_eq!(map.low, Some(models::LevelValue::Mapped("l".into())));
    assert_eq!(
        model.headers,
        Some(vec![
            ("x-a".into(), "1".into()),
            ("x-b".into(), String::new())
        ])
    );
    assert!((model.context_window - 200_000.0).abs() < f64::EPSILON);
    assert!(
        (model.max_tokens - 8192.5).abs() < f64::EPSILON,
        "fractional limits pass through"
    );
    let Some(models::ModelCompat::OpenaiCompletions(compat)) = &model.compat else {
        panic!("compat changed kind")
    };
    assert_eq!(
        compat.thinking_format,
        Some(models::ThinkingFormat::QwenChatTemplate)
    );
    assert_eq!(
        compat.max_tokens_field,
        Some(models::MaxTokensField::MaxCompletionTokens)
    );
    assert_routing(compat.open_router_routing.as_ref().unwrap());
}

#[test]
fn maestro_selection_events_keep_model_and_thinking_metadata() {
    let model: Rc<RefCell<Option<models::Model>>> = Rc::default();
    let levels: Rc<RefCell<Vec<(models::ThinkingLevel, models::ThinkingLevel)>>> = Rc::default();
    let (record_model, record_levels) = (Rc::clone(&model), Rc::clone(&levels));
    let host = host_with("select", move |event| {
        match event {
            ExtensionEvent::ModelSelect(select) => {
                assert_eq!(select.source, events::ModelSelectSource::Restore);
                assert_eq!(
                    select.previous_model, None,
                    "an absent previous model stays absent"
                );
                *record_model.borrow_mut() = Some(select.model.clone());
            }
            ExtensionEvent::ThinkingLevelSelect(select) => {
                record_levels
                    .borrow_mut()
                    .push((select.level, select.previous_level));
            }
            _ => {}
        }
        Ok(None)
    });
    assert_eq!(
        deliver(
            &host,
            "select",
            ExtensionEvent::ModelSelect(Box::new(typed::model_select()))
        ),
        Ok(None)
    );
    assert_eq!(
        deliver(
            &host,
            "select",
            ExtensionEvent::ThinkingLevelSelect(typed::thinking_select())
        ),
        Ok(None)
    );
    assert_model(model.borrow().as_ref().unwrap());
    assert_eq!(
        *levels.borrow(),
        [(models::ThinkingLevel::Xhigh, models::ThinkingLevel::Off)]
    );
}

#[test]
fn maestro_input_transforms_keep_source_and_image_presence() {
    let host = host_with("input", |event| {
        let ExtensionEvent::Input(input) = event else {
            return Ok(None);
        };
        Ok(Some(ExtensionEventResult::Input(match input.source {
            events::InputSource::Interactive => events::InputEventResult::Continue,
            events::InputSource::Rpc => {
                events::InputEventResult::Transform(events::InputTransform {
                    text: String::new(),
                    images: input.images.clone(),
                })
            }
            events::InputSource::Extension => events::InputEventResult::Handled,
        })))
    });
    let input = |result| Ok(Some(events::ExtensionEventResult::Input(result)));
    let interactive = deliver(&host, "input", ExtensionEvent::Input(typed::input("hi")));
    assert_eq!(interactive, input(events::InputEventResult::Continue));
    let empty = events::InputTransform {
        text: String::new(),
        images: Some(Vec::new()),
    };
    let rpc = deliver(&host, "input", ExtensionEvent::Input(typed::rpc_input()));
    assert_eq!(
        rpc,
        input(events::InputEventResult::Transform(empty)),
        "an empty text and an empty image list survive"
    );
    let mut absent = typed::rpc_input();
    absent.images = None;
    absent.source = events::InputSource::Extension;
    assert_eq!(
        deliver(&host, "input", ExtensionEvent::Input(absent)),
        input(events::InputEventResult::Handled)
    );
    let mut with_image = typed::rpc_input();
    with_image.images = Some(vec![typed::image()]);
    let transformed = deliver(&host, "input", ExtensionEvent::Input(with_image));
    let expected = events::InputTransform {
        text: String::new(),
        images: Some(vec![typed::image()]),
    };
    assert_eq!(
        transformed,
        input(events::InputEventResult::Transform(expected))
    );
}

/// Operations a user bash replacement supplies.
struct Operations;

impl BashOperationsPort for Operations {}

/// The variant-specific part of a tool result, as one line.
fn tool_result_summary(result: &ToolResultEvent) -> String {
    match result {
        ToolResultEvent::Bash(r) => {
            let by = r
                .details
                .as_ref()
                .and_then(|d| d.truncation.as_ref())
                .and_then(|t| t.truncated_by);
            let full = r.details.as_ref().and_then(|d| d.full_output_path.clone());
            format!("truncated={:?} full={full:?}", by.map(|by| variant(&by)))
        }
        ToolResultEvent::Edit(r) => {
            let details = r.details.as_ref();
            format!(
                "diff={} line={:?}",
                details.map_or("", |d| d.diff.as_str()),
                details.and_then(|d| d.first_changed_line)
            )
        }
        ToolResultEvent::Write(_) => "details=none".to_owned(),
        ToolResultEvent::Grep(r) => {
            let details = r.details.as_ref();
            format!(
                "match_limit={:?} lines_truncated={:?}",
                details.and_then(|d| d.match_limit_reached),
                details.and_then(|d| d.lines_truncated)
            )
        }
        ToolResultEvent::Custom(r) => format!("custom details={:?}", r.details),
        ToolResultEvent::Read(r) => format!("details={:?}", r.details),
        ToolResultEvent::Find(r) => format!("details={:?}", r.details),
        ToolResultEvent::Ls(r) => format!("details={:?}", r.details),
    }
}

/// Checks the calls a handler recorded against the fixtures.
fn assert_calls(calls: &[ToolCallEvent]) {
    let names: Vec<&str> = calls.iter().map(ToolCallEvent::tool_name).collect();
    assert_eq!(
        names,
        [
            "bash", "read", "edit", "write", "grep", "find", "ls", "mine"
        ]
    );
    assert_eq!(calls, typed::tool_calls());
    let ToolCallEvent::Read(read) = &calls[1] else {
        panic!("not a read call")
    };
    assert_eq!(
        (read.input.offset, read.input.limit),
        (Some(-1.5), Some(10.0)),
        "fractional and negative numbers pass through"
    );
    let ToolCallEvent::Find(find) = &calls[5] else {
        panic!("not a find call")
    };
    assert_eq!(
        (find.input.path.as_deref(), find.input.limit),
        (None, Some(0.0)),
        "zero is a value, absence is not"
    );
    let ToolCallEvent::Custom(custom) = &calls[7] else {
        panic!("not a custom call")
    };
    assert_eq!(custom.input, r#"{"nested":{"k":[1,null,false]}}"#);
}

/// Checks the results a handler recorded against the fixtures.
fn assert_results(results: &[ToolResultEvent]) {
    assert_eq!(results, typed::tool_results());
    let summaries: Vec<String> = results.iter().map(tool_result_summary).collect();
    assert_eq!(
        summaries[0],
        "truncated=Some(\"Bytes\") full=Some(\"/tmp/full\")"
    );
    assert_eq!(summaries[2], "diff=-a\n+b line=Some(7)");
    assert_eq!(summaries[3], "details=none", "write has no details");
    assert_eq!(
        summaries[4],
        "match_limit=Some(100.0) lines_truncated=Some(false)"
    );
    assert_eq!(
        summaries[7],
        "custom details=Some(\"{\\\"deep\\\":{\\\"x\\\":null}}\")"
    );
}

/// The replacement of a user bash command: a result and the operations to run it with.
fn replace_bash(bash: &events::UserBashEvent) -> ExtensionEventResult {
    assert_eq!(
        (
            bash.command.as_str(),
            bash.exclude_from_context,
            bash.cwd.as_str()
        ),
        ("echo \u{e9}", true, "/w")
    );
    ExtensionEventResult::UserBash(UserBashEventResult {
        data: events::UserBashEventResultData {
            result: Some(session::BashResult {
                output: "out".into(),
                exit_code: None,
                cancelled: false,
                truncated: true,
                full_output_path: None,
            }),
        },
        operations: Some(BashOperations::new(Rc::new(Operations))),
    })
}

#[test]
fn maestro_tool_events_keep_builtin_inputs_and_custom_extras() {
    let calls: Rc<RefCell<Vec<ToolCallEvent>>> = Rc::default();
    let results: Rc<RefCell<Vec<ToolResultEvent>>> = Rc::default();
    let (record_calls, record_results) = (Rc::clone(&calls), Rc::clone(&results));
    let host = host_with("tools", move |event| {
        match event {
            ExtensionEvent::ToolCall(call) => record_calls.borrow_mut().push(call.clone()),
            ExtensionEvent::ToolResult(result) => record_results.borrow_mut().push(result.clone()),
            ExtensionEvent::UserBash(bash) => return Ok(Some(replace_bash(bash))),
            _ => {}
        }
        Ok(None)
    });
    for call in typed::tool_calls() {
        assert_eq!(
            deliver(&host, "tools", ExtensionEvent::ToolCall(call)),
            Ok(None)
        );
    }
    for result in typed::tool_results() {
        assert_eq!(
            deliver(&host, "tools", ExtensionEvent::ToolResult(result)),
            Ok(None)
        );
    }
    assert_calls(&calls.borrow());
    assert_results(&results.borrow());
    let bash = deliver(&host, "tools", ExtensionEvent::UserBash(typed::user_bash()));
    assert!(matches!(
        bash,
        Ok(Some(events::ExtensionEventResult::UserBash(
            events::UserBashEventResultData { result: Some(_) }
        )))
    ));
    let outcome = UserBashEventResult {
        data: events::UserBashEventResultData { result: None },
        operations: Some(BashOperations::new(Rc::new(Operations))),
    };
    let (_, operations) = ExtensionEventResult::UserBash(outcome).into_wire();
    assert!(
        operations.is_some(),
        "the operations stay with the extension beside the wire result"
    );
}

/// Pairs of an author result and the wire result it must become.
fn result_pairs() -> Vec<(ExtensionEventResult, events::ExtensionEventResult)> {
    let tool_call = || events::ToolCallEventResult {
        block: Some(false),
        reason: Some(String::new()),
    };
    let replacement = || events::ToolResultEventResult {
        content: Some(Vec::new()),
        details: Some("0".into()),
        is_error: Some(false),
    };
    let nothing = || events::ToolResultEventResult {
        content: None,
        details: None,
        is_error: None,
    };
    let cancel = || events::SessionBeforeCompactResult {
        cancel: Some(true),
        compaction: None,
    };
    vec![
        (
            ExtensionEventResult::BeforeProviderRequest("null".into()),
            events::ExtensionEventResult::BeforeProviderRequest("null".into()),
        ),
        (
            ExtensionEventResult::ToolCall(tool_call()),
            events::ExtensionEventResult::ToolCall(tool_call()),
        ),
        (
            ExtensionEventResult::ToolResult(replacement()),
            events::ExtensionEventResult::ToolResult(replacement()),
        ),
        (
            ExtensionEventResult::ToolResult(nothing()),
            events::ExtensionEventResult::ToolResult(nothing()),
        ),
        (
            ExtensionEventResult::SessionBeforeCompact(cancel()),
            events::ExtensionEventResult::SessionBeforeCompact(cancel()),
        ),
        (
            ExtensionEventResult::Context(events::ContextEventResult { messages: None }),
            events::ExtensionEventResult::Context(events::ContextEventResult { messages: None }),
        ),
        (
            ExtensionEventResult::MessageEnd(events::MessageEndEventResult { message: None }),
            events::ExtensionEventResult::MessageEnd(events::MessageEndEventResult {
                message: None,
            }),
        ),
    ]
}

#[test]
fn maestro_event_results_keep_absent_and_falsy_replacements() {
    for (author_result, wire) in result_pairs() {
        let (converted, operations) = author_result.into_wire();
        assert_eq!(converted, wire);
        assert!(operations.is_none());
    }
    let response = typed::provider_response();
    assert_eq!(
        (response.status, response.headers[1].1.as_str()),
        (429, ""),
        "status and ordered headers survive"
    );
    assert_eq!(
        typed::provider_request().payload,
        r#"{"stream":false,"n":0,"x":null}"#
    );
    assert_eq!(
        typed::custom_event().payload.as_deref(),
        Some("null"),
        "a present null payload is not absence"
    );
}

#[test]
fn maestro_custom_json_keeps_order_and_valid_unicode() {
    let host = ControlledHost::new(Log::default());
    let api = host.api();
    let payload =
        json!({ "z": 1, "a": { "y": null, "b": [false, 0, ""] }, "m": "\u{feff}\u{85}\u{1f680}" });
    api.append_entry("order", Some(payload)).unwrap();
    api.append_entry("null", Some(Value::Null)).unwrap();
    api.append_entry("absent", None).unwrap();
    let lines = host.log_lines();
    assert_eq!(
        lines[0],
        "entry order {\"z\":1,\"a\":{\"y\":null,\"b\":[false,0,\"\"]},\"m\":\"\u{feff}\u{85}\u{1f680}\"}",
        "keys keep their insertion order and the characters stay as written"
    );
    assert_eq!(lines[1], "entry null null");
    assert_eq!(lines[2], "entry absent none");
    let ToolCallEvent::Custom(custom) = typed::tool_calls().pop().unwrap() else {
        panic!("not a custom call")
    };
    let parsed: Value = serde_json::from_str(&custom.input).unwrap();
    assert_eq!(parsed["nested"]["k"], json!([1, null, false]));
    assert_eq!(
        custom.input, r#"{"nested":{"k":[1,null,false]}}"#,
        "nested JSON text is kept byte for byte"
    );
}

/// The nine inputs of the reference run and the summaries it produced; the boundary case is the
/// corrected result, since the reference cut a supplementary character in half.
fn summary_cases() -> Vec<(Vec<AgentMessage>, String)> {
    let user_blocks = |text: &str| {
        AgentMessage::User(models::UserMessage {
            content: UserContent::Blocks(vec![typed::text_block(text)]),
            timestamp: 1.0,
        })
    };
    let assistant = || AgentMessage::Assistant(typed::assistant());
    let x = |n: usize| "x".repeat(n);
    vec![
        (vec![], "User requests:\n".into()),
        (vec![assistant()], "User requests:\n".into()),
        (vec![user_text("short")], "User requests:\n- short".into()),
        (vec![user_text("")], "User requests:\n- ".into()),
        (
            vec![user_text(&x(100))],
            format!("User requests:\n- {}", x(100)),
        ),
        (
            vec![user_text(&x(101))],
            format!("User requests:\n- {}", x(100)),
        ),
        (
            vec![user_text(&format!("{}\u{1f680}z", x(99)))],
            format!("User requests:\n- {}\u{1f680}", x(99)),
        ),
        (
            vec![user_blocks("complex")],
            "User requests:\n- [complex]".into(),
        ),
        (
            vec![
                user_text("\u{e9}\r\n\u{feff}\u{85}"),
                assistant(),
                user_text("last"),
            ],
            "User requests:\n- \u{e9}\r\n\u{feff}\u{85}\n- last".into(),
        ),
    ]
}

#[test]
fn maestro_compaction_summary_keeps_unicode_boundaries() {
    let cases = summary_cases();
    assert_eq!(cases.len(), 9);
    for (messages, expected) in cases {
        assert_eq!(compaction_example::user_requests(&messages), expected);
    }
    let boundary =
        compaction_example::user_requests(&[user_text(&format!("{}\u{1f680}z", "x".repeat(99)))]);
    assert!(
        boundary.ends_with('\u{1f680}'),
        "character one hundred is the whole supplementary character"
    );
    assert_eq!(
        boundary.chars().count(),
        "User requests:\n- ".chars().count() + 100
    );
}

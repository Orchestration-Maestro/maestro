//! Shared event contracts through native records and stored typed handlers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/model.rs"]
mod model_support;
#[allow(dead_code, reason = "Shared local fixture.")]
#[path = "support/local.rs"]
mod support;
use maestro_extensions::{ExtensionHandler, types::*};
use maestro_models::{Local, extension_events as shared};
use maestro_session::Presence;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{rc::Rc, sync::Arc};

fn check<T: Serialize + DeserializeOwned>(input: Value) -> T {
    let expected = input.to_string();
    let event: T = serde_json::from_value(input).unwrap();
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        serde_json::from_str::<Value>(&expected).unwrap()
    );
    event
}
fn optional<T: Serialize + DeserializeOwned>(base: Value, fields: &[(&str, Value)]) {
    check::<T>(base.clone());
    for (name, value) in fields {
        for value in [Value::Null, value.clone()] {
            let mut input = base.clone();
            input[*name] = value;
            check::<T>(input);
        }
    }
    let mut all = base;
    for (name, value) in fields {
        all[*name] = value.clone();
    }
    check::<T>(all);
}

#[test]
fn tool_name_predicates_compare_exact_untrimmed_names() {
    let names = ["bash", "read", "edit", "write", "grep", "find", "ls"];
    let predicates: [ResultPredicate; 7] = [
        shared::is_bash_tool_result,
        shared::is_read_tool_result,
        shared::is_edit_tool_result,
        shared::is_write_tool_result,
        shared::is_grep_tool_result,
        shared::is_find_tool_result,
        shared::is_ls_tool_result,
    ];
    for name in
        names
            .into_iter()
            .chain(["custom", "BASH", "bash ", "\u{feff}bash", "bash\u{85}", ""])
    {
        let result = shared::ToolResultEvent {
            tool_call_id: "id".into(),
            tool_name: name.into(),
            input: json!([false]),
            content: vec![],
            is_error: false,
            details: Presence::Present(json!({"opaque":2})),
        };
        for (expected, predicate) in names.iter().zip(predicates) {
            assert_eq!(predicate(&result), *expected == name, "{expected} / {name}");
        }
        let call = shared::ToolCallEvent {
            tool_call_id: "id".into(),
            tool_name: name.into(),
            input: json!(false),
        };
        assert!(shared::is_tool_call_event_type(name, &call));
        assert!(!shared::is_tool_call_event_type(&format!("{name}x"), &call));
    }
}

#[test]
fn resource_and_session_records_keep_every_field() {
    for reason in ["startup", "reload"] {
        check::<ResourcesDiscoverEvent>(json!({"cwd":"/resources","reason":reason}));
    }
    optional::<ResourcesDiscoverResult>(
        json!({}),
        &[
            ("skillPaths", json!(["z", "a", "z"])),
            ("promptPaths", json!([])),
            ("themePaths", json!(["theme"])),
        ],
    );
    for reason in ["startup", "reload", "new", "resume", "fork"] {
        optional::<SessionStartEvent>(
            json!({"reason":reason}),
            &[("previousSessionFile", json!(""))],
        );
    }
    for reason in ["new", "resume"] {
        optional::<SessionBeforeSwitchEvent>(
            json!({"reason":reason}),
            &[("targetSessionFile", json!("target"))],
        );
    }
    for reason in ["quit", "reload", "new", "resume", "fork"] {
        optional::<SessionShutdownEvent>(
            json!({"reason":reason}),
            &[("targetSessionFile", json!(""))],
        );
    }
    for position in ["before", "at"] {
        check::<SessionBeforeForkEvent>(json!({"entryId":"branch","position":position}));
    }
    check_session_navigation();
    check_native_session_operands();
}

#[test]
fn agent_event_records_keep_message_and_prompt_operands() {
    check_native_message_owners();
    let user: NativeAgentMessage = maestro_agent::AgentMessage::User(Arc::new(
        std::sync::RwLock::new(maestro_models::UserMessage {
            content: maestro_models::UserContent::Text("question".into()),
            timestamp: 3.0,
        }),
    ));
    let user_alias = user.clone();
    let context = ContextEvent {
        messages: vec![user],
    };
    let maestro_agent::AgentMessage::User(original) = &user_alias else {
        panic!("user")
    };
    let maestro_agent::AgentMessage::User(retained) = &context.messages[0] else {
        panic!("user")
    };
    assert!(Arc::ptr_eq(original, retained));
    original.write().unwrap().timestamp = 7.0;
    assert_eq!(
        retained.read().unwrap().timestamp.to_bits(),
        7.0_f64.to_bits()
    );
    let message = json!({"role":"user","content":"question","timestamp":3.0});
    check::<AgentEndEvent<maestro_agent::AgentMessage>>(json!({"messages":[message]}));
    check::<MessageStartEvent<maestro_agent::AgentMessage>>(json!({"message":message}));
    check::<MessageEndEvent<maestro_agent::AgentMessage>>(json!({"message":message}));
    check::<TurnEndEvent<maestro_agent::AgentMessage>>(
        json!({"turnIndex":-0.5,"message":message,"toolResults":[{"role":"toolResult","toolCallId":"result-id","toolName":"custom","content":[{"type":"text","text":"output"}],"details":false,"isError":false,"timestamp":5.0}]}),
    );
    check::<AgentStartEvent>(json!({}));
    let turn = check::<TurnStartEvent>(json!({"turnIndex":-0.0,"timestamp":-2.5}));
    assert!(turn.turn_index.is_sign_negative());
    optional::<maestro_models::system_prompt::BuildSystemPromptOptions>(
        json!({"cwd":"/prompt"}),
        &[
            ("customPrompt", json!("")),
            ("selectedTools", json!(["read", "read"])),
            ("toolSnippets", json!({"z":"last","a":"first"})),
            ("promptGuidelines", json!([])),
            ("appendSystemPrompt", json!("append")),
            (
                "contextFiles",
                json!([{"path":"relative","content":"instructions"}]),
            ),
            (
                "skills",
                json!([{ "name":"skill","description":"instruction","filePath":"skill.md","baseDir":".","sourceInfo":{"path":"skill.md","source":"local","scope":"temporary","origin":"top-level"},"disableModelInvocation":false}]),
            ),
        ],
    );
    optional::<BeforeAgentStartEvent>(
        json!({"prompt":"prompt","systemPrompt":"system","systemPromptOptions":{"cwd":"/prompt"}}),
        &[(
            "images",
            json!([{ "type":"image","data":"aGVsbG8=","mimeType":"image/png"}]),
        )],
    );
}

#[test]
fn execution_selection_and_input_records_keep_domains() {
    for payload in [
        Value::Null,
        json!(false),
        json!(""),
        json!([]),
        json!({}),
        json!(-0.0),
        json!({"z":1,"a":2}),
    ] {
        check::<BeforeProviderRequestEvent>(json!({"payload":payload.clone()}));
        check::<ToolExecutionStartEvent>(
            json!({"toolCallId":"id","toolName":"name","args":payload.clone()}),
        );
        check::<ToolExecutionUpdateEvent>(
            json!({"toolCallId":"id","toolName":"name","args":payload.clone(),"partialResult":{"different":true}}),
        );
        for error in [false, true] {
            check::<ToolExecutionEndEvent>(
                json!({"toolCallId":"id","toolName":"name","result":payload,"isError":error}),
            );
        }
    }
    for status in [-2.5_f64, -0.0, 200.25] {
        let event = check::<AfterProviderResponseEvent>(
            json!({"status":status,"headers":{"A":"one","z":"two"}}),
        );
        assert_eq!(event.status.to_bits(), status.to_bits());
    }
    let current = model_support::model("current");
    let previous = model_support::model("previous");
    for source in [
        ModelSelectSource::Set,
        ModelSelectSource::Cycle,
        ModelSelectSource::Restore,
    ] {
        let event = ModelSelectEvent {
            model: current.clone(),
            previous_model: Presence::Present(previous.clone()),
            source,
        };
        assert!(Arc::ptr_eq(&event.model, &current));
        let Presence::Present(ref model) = event.previous_model else {
            panic!("previous")
        };
        assert!(Arc::ptr_eq(model, &previous));
    }
    for level in ["off", "minimal", "low", "medium", "high", "xhigh"] {
        check::<ThinkingLevelSelectEvent>(json!({"level":level,"previousLevel":"off"}));
        check::<ThinkingLevelSelectEvent>(json!({"level":"off","previousLevel":level}));
    }
    check_input_and_shell_domains();
}

#[test]
fn event_results_keep_each_optional_and_opaque_branch() {
    optional::<ToolCallEventResult>(json!({}), &[("block", json!(false)), ("reason", json!(""))]);
    optional::<ToolResultEventResult>(
        json!({}),
        &[
            ("content", json!([])),
            ("details", json!({"z":1,"a":2})),
            ("isError", json!(false)),
        ],
    );
    optional::<shared::ContextEventResult<Value>>(
        json!({}),
        &[("messages", json!([{"role":"custom","text":"replacement"}]))],
    );
    optional::<shared::MessageEndEventResult<Value>>(
        json!({}),
        &[("message", json!({"role":"custom","text":"replacement"}))],
    );
    optional::<BeforeAgentStartEventResult>(
        json!({}),
        &[
            ("systemPrompt", json!("")),
            (
                "message",
                json!({"customType":"custom","content":"content","display":false}),
            ),
        ],
    );
    optional::<SessionBeforeSwitchResult>(json!({}), &[("cancel", json!(false))]);
    optional::<SessionBeforeForkResult>(
        json!({}),
        &[
            ("cancel", json!(false)),
            ("skipConversationRestore", json!(false)),
        ],
    );
    optional::<shared::SessionBeforeCompactResult<Value>>(
        json!({}),
        &[
            ("cancel", json!(false)),
            ("compaction", json!({"summary":"result","tokensBefore":3.5})),
        ],
    );
    optional::<SessionBeforeTreeResult>(
        json!({}),
        &[
            ("cancel", json!(false)),
            ("summary", json!({"summary":"nonempty","details":false})),
            ("customInstructions", json!("")),
            ("replaceInstructions", json!(false)),
            ("label", json!("")),
        ],
    );
    optional::<shared::SessionBeforeTreeSummary<Value>>(
        json!({"summary":"summary"}),
        &[("details", json!(false))],
    );
    check_input_results();
    check_supplied_shell_handles();
}

async fn install<V: 'static, R: Serialize + DeserializeOwned + 'static>(
    loaded: &support::Loaded,
    tag: &str,
    mut event: V,
    result: Value,
) {
    let output = result.clone();
    let handler: ExtensionHandler<V, R, Local> = Rc::new(move |_, ctx| {
        assert_eq!(ctx.cwd().unwrap(), "/work");
        let output = output.clone();
        Box::pin(async move {
            tokio::task::yield_now().await;
            Ok(Some(serde_json::from_value(output).unwrap()))
        })
    });
    loaded.api.on(tag.into(), handler.clone()).unwrap();
    let stored = loaded.owner.read().unwrap().handlers[tag][0]
        .downcast_ref::<ExtensionHandler<V, R, Local>>()
        .unwrap()
        .clone();
    assert!(Rc::ptr_eq(&handler, &stored));
    let runner = support::runner(&loaded.owners);
    assert_eq!(
        serde_json::to_value(
            stored(&mut event, runner.create_context())
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        result
    );
}

#[test]
fn all_event_records_preserve_domain_fields_and_open_event_registration() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("events", bus.controller.for_extension());
        install_group_0(&loaded).await;
        install_group_1(&loaded).await;
        install_group_2(&loaded).await;
        install_group_3(&loaded).await;
        install_group_4(&loaded).await;
        assert_eq!(loaded.owner.read().unwrap().handlers.len(), 29);
        let duplicate: ExtensionHandler<(), (), Local> =
            Rc::new(|(), _| Box::pin(async { Ok(None) }));
        loaded
            .api
            .on("tool_call".into(), duplicate.clone())
            .unwrap();
        loaded
            .api
            .on("tool_call".into(), duplicate.clone())
            .unwrap();
        loaded
            .api
            .on("custom event".into(), duplicate.clone())
            .unwrap();
        loaded.api.on(String::new(), duplicate).unwrap();
        let owner = loaded.owner.read().unwrap();
        let handlers = &owner.handlers["tool_call"];
        assert_eq!(handlers.len(), 3);
        let a = handlers[1]
            .downcast_ref::<ExtensionHandler<(), (), Local>>()
            .unwrap();
        let b = handlers[2]
            .downcast_ref::<ExtensionHandler<(), (), Local>>()
            .unwrap();
        assert!(Rc::ptr_eq(a, b));
        assert_eq!(owner.handlers.keys().last().unwrap(), "");
        drop(owner);
        loaded.runtime.invalidate(Some("inactive".into()));
        let blocked: ExtensionHandler<(), (), Local> =
            Rc::new(|(), _| Box::pin(async { Ok(None) }));
        assert!(loaded.api.on("blocked".into(), blocked).is_err());
        assert!(
            !loaded
                .owner
                .read()
                .unwrap()
                .handlers
                .contains_key("blocked")
        );
    });
}

#[test]
fn handler_families_keep_context_and_capture_affinity() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("local", bus.controller.for_extension());
        let count = Rc::new(std::cell::Cell::new(0));
        let captured = count.clone();
        let handler: ExtensionHandler<String, String, Local> = Rc::new(move |value, context| {
            captured.set(captured.get() + 1);
            Box::pin(async move {
                tokio::task::yield_now().await;
                assert_eq!(context.cwd().unwrap(), "/work");
                value.push_str(" changed");
                Ok(Some(value.clone()))
            })
        });
        loaded.api.on("typed".into(), handler).unwrap();
        let handler = loaded.owner.read().unwrap().handlers["typed"][0]
            .downcast_ref::<ExtensionHandler<String, String, Local>>()
            .unwrap()
            .clone();
        let runner = support::runner(&loaded.owners);
        let mut value = "input".to_owned();
        assert_eq!(
            handler(&mut value, runner.create_context()).await.unwrap(),
            Some("input changed".into())
        );
        assert_eq!(value, "input changed");
        assert_eq!(count.get(), 1);
        let threaded: ExtensionHandler<String, String> = Arc::new(|value, context| {
            Box::pin(async move {
                tokio::task::yield_now().await;
                assert_eq!(context.cwd().unwrap(), "/work");
                Ok(Some(value.clone()))
            })
        });
        send_sync(&threaded);
        assert_eq!(
            threaded(&mut value, runner.create_context()).await.unwrap(),
            Some("input changed".into())
        );
    });
}

/// Built-in name check over opaque native data.
type ResultPredicate = fn(&shared::ToolResultEvent<Value, Value>) -> bool;

/// Navigation preparations and retained cancellation.
fn check_session_navigation() {
    let tree = shared::TreePreparation::<Value> {
        target_id: "target".into(),
        old_leaf_id: Some("old".into()),
        common_ancestor_id: Some("common".into()),
        entries_to_summarize: vec![json!({"id":"entry"})],
        user_wants_summary: false,
        custom_instructions: Presence::Present("instructions".into()),
        replace_instructions: Presence::Present(false),
        label: Presence::Null,
    };
    assert_eq!(tree.entries_to_summarize[0]["id"], "entry");
    assert!(!tree.user_wants_summary);
    assert_eq!(tree.old_leaf_id.as_deref(), Some("old"));
    assert_eq!(tree.common_ancestor_id.as_deref(), Some("common"));
    let signal = maestro_models::Cancellation::new();
    let event = shared::SessionBeforeTreeEvent {
        preparation: tree,
        signal: signal.clone(),
    };
    let retained = event.signal.clone();
    drop(event);
    signal.abort();
    assert!(retained.is_aborted());
    optional::<shared::SessionTreeEvent<Value>>(
        json!({"newLeafId":null,"oldLeafId":"old"}),
        &[
            ("summaryEntry", json!({"id":"summary","summary":"nonempty"})),
            ("fromExtension", json!(false)),
        ],
    );
    check::<shared::SessionCompactEvent<Value>>(
        json!({"compactionEntry":{"summary":"kept","tokensBefore":2.5},"fromExtension":false}),
    );
    let compact = shared::SessionBeforeCompactEvent {
        data: shared::SessionBeforeCompactEventData {
            preparation: json!({"messagesToSummarize":["one"],"settings":{"enabled":false},"fileOps":{"read":["a"]}}),
            branch_entries: vec![json!({"id":"branch"})],
            custom_instructions: Presence::Present("keep".into()),
        },
        signal,
    };
    assert_eq!(compact.branch_entries[0]["id"], "branch");
    assert_eq!(compact.preparation["messagesToSummarize"], json!(["one"]));
    assert!(compact.signal.is_aborted());
}

/// Supplied shell operations and results coexist without new execution policy.
fn check_supplied_shell_handles() {
    let operations = Rc::new("operations".to_owned());
    let result = Rc::new("result".to_owned());
    let supplied = shared::UserBashEventResult {
        operations: Some(operations.clone()),
        result: Presence::Present(result.clone()),
    };
    assert!(Rc::ptr_eq(
        supplied.operations.as_ref().unwrap(),
        &operations
    ));
    let Presence::Present(ref supplied_result) = supplied.result else {
        panic!("result")
    };
    assert!(Rc::ptr_eq(supplied_result, &result));
}

/// Input sources and shell inclusion are independent fields.
fn check_input_and_shell_domains() {
    for source in ["interactive", "rpc", "extension"] {
        optional::<InputEvent>(
            json!({"text":"  text  ","source":source}),
            &[("images", json!([]))],
        );
    }
    for excluded in [false, true] {
        check::<UserBashEvent>(
            json!({"command":"command","cwd":"/bash","excludeFromContext":excluded}),
        );
    }
}

/// Install each typed contract through the real registration owner.
macro_rules! event {
    ($loaded:expr,$tag:literal,$v:ty,$r:ty,$input:expr,$result:expr) => {
        install::<$v, $r>(
            $loaded,
            $tag,
            serde_json::from_value($input).unwrap(),
            $result,
        )
        .await;
    };
}

/// Typed registration operands in source order.
async fn install_group_0(loaded: &support::Loaded) {
    event!(
        loaded,
        "tool_call",
        ToolCallEvent,
        ToolCallEventResult,
        json!({"toolCallId":"call","toolName":"custom","input":false}),
        json!({"block":false})
    );
    event!(
        loaded,
        "tool_result",
        ToolResultEvent,
        ToolResultEventResult,
        json!({"toolCallId":"result","toolName":"custom","input":[],"content":[],"isError":false}),
        json!({"isError":true})
    );
    event!(loaded,"user_bash",UserBashEvent,shared::UserBashEventResult<(),Value>,json!({"command":"echo","cwd":"/bash","excludeFromContext":false}),json!({"result":false}));
    let selected = ModelSelectEvent {
        model: model_support::model("model"),
        previous_model: Presence::Missing,
        source: ModelSelectSource::Set,
    };
    install::<_, ()>(loaded, "model_select", selected, Value::Null).await;
    event!(
        loaded,
        "thinking_level_select",
        ThinkingLevelSelectEvent,
        (),
        json!({"level":"off","previousLevel":"high"}),
        Value::Null
    );
    event!(
        loaded,
        "tool_execution_start",
        ToolExecutionStartEvent,
        (),
        json!({"toolCallId":"one","toolName":"custom","args":null}),
        Value::Null
    );
    event!(
        loaded,
        "tool_execution_update",
        ToolExecutionUpdateEvent,
        (),
        json!({"toolCallId":"two","toolName":"custom","args":false,"partialResult":[]}),
        Value::Null
    );
    event!(
        loaded,
        "tool_execution_end",
        ToolExecutionEndEvent,
        (),
        json!({"toolCallId":"three","toolName":"custom","result":{},"isError":false}),
        Value::Null
    );
}

/// Typed registration operands in source order.
async fn install_group_1(loaded: &support::Loaded) {
    event!(
        loaded,
        "context",
        shared::ContextEvent<String>,
        shared::ContextEventResult<String>,
        json!({"messages":["input"]}),
        json!({"messages":["output"]})
    );
    event!(
        loaded,
        "before_agent_start",
        BeforeAgentStartEvent,
        BeforeAgentStartEventResult,
        json!({"prompt":"prompt","systemPrompt":"system","systemPromptOptions":{"cwd":"/work"}}),
        json!({"systemPrompt":"changed"})
    );
    event!(
        loaded,
        "agent_start",
        AgentStartEvent,
        (),
        json!({}),
        Value::Null
    );
    event!(
        loaded,
        "agent_end",
        shared::AgentEndEvent<String>,
        (),
        json!({"messages":["final"]}),
        Value::Null
    );
    event!(
        loaded,
        "turn_start",
        TurnStartEvent,
        (),
        json!({"turnIndex":1.5,"timestamp":-0.0}),
        Value::Null
    );
    event!(
        loaded,
        "turn_end",
        shared::TurnEndEvent<String>,
        (),
        json!({"turnIndex":2.5,"message":"message","toolResults":[]}),
        Value::Null
    );
    install_message_group(loaded).await;
}

/// Typed registration operands in source order.
async fn install_group_2(loaded: &support::Loaded) {
    event!(
        loaded,
        "session_start",
        SessionStartEvent,
        (),
        json!({"reason":"new"}),
        Value::Null
    );
    event!(
        loaded,
        "session_before_switch",
        SessionBeforeSwitchEvent,
        SessionBeforeSwitchResult,
        json!({"reason":"resume"}),
        json!({"cancel":false})
    );
    event!(
        loaded,
        "session_before_fork",
        SessionBeforeForkEvent,
        SessionBeforeForkResult,
        json!({"entryId":"fork","position":"at"}),
        json!({"skipConversationRestore":false})
    );
}

/// Typed registration operands in source order.
async fn install_group_3(loaded: &support::Loaded) {
    let compact = shared::SessionBeforeCompactEvent {
        data: shared::SessionBeforeCompactEventData {
            preparation: json!({"firstKeptEntryId":"one"}),
            branch_entries: vec![json!({"id":"one"})],
            custom_instructions: Presence::Null,
        },
        signal: maestro_models::Cancellation::new(),
    };
    install::<_, shared::SessionBeforeCompactResult<Value>>(
        loaded,
        "session_before_compact",
        compact,
        json!({"cancel":false}),
    )
    .await;
    event!(
        loaded,
        "session_compact",
        shared::SessionCompactEvent<Value>,
        (),
        json!({"compactionEntry":{"summary":"summary"},"fromExtension":false}),
        Value::Null
    );
    let tree = shared::SessionBeforeTreeEvent {
        preparation: json!({"targetId":"target"}),
        signal: maestro_models::Cancellation::new(),
    };
    install::<_, SessionBeforeTreeResult>(
        loaded,
        "session_before_tree",
        tree,
        json!({"label":"label"}),
    )
    .await;
    event!(
        loaded,
        "session_tree",
        shared::SessionTreeEvent<Value>,
        (),
        json!({"newLeafId":"new","oldLeafId":null}),
        Value::Null
    );
}

/// Typed registration operands in source order.
async fn install_group_4(loaded: &support::Loaded) {
    event!(
        loaded,
        "session_shutdown",
        SessionShutdownEvent,
        (),
        json!({"reason":"quit"}),
        Value::Null
    );
    event!(
        loaded,
        "before_provider_request",
        BeforeProviderRequestEvent,
        BeforeProviderRequestEventResult,
        json!({"payload":{"opaque":true}}),
        json!({"replacement":true})
    );
    event!(
        loaded,
        "after_provider_response",
        AfterProviderResponseEvent,
        (),
        json!({"status":200.5,"headers":{"z":"value"}}),
        Value::Null
    );
    event!(
        loaded,
        "input",
        InputEvent,
        InputEventResult,
        json!({"text":"input","source":"extension"}),
        json!({"action":"handled"})
    );
}

/// Each input verdict keeps its own transformed payload.
fn check_input_results() {
    for action in [
        json!({"action":"continue"}),
        json!({"action":"handled"}),
        json!({"action":"transform","text":""}),
        json!({"action":"transform","text":"text","images":null}),
        json!({"action":"transform","text":"text","images":[]}),
    ] {
        check::<InputEventResult>(action);
    }
}

/// Compile-time storage affinity control.
fn send_sync<T: Send + Sync>(_: &T) {}

/// Finalized and streaming message handler contracts.
async fn install_message_group(loaded: &support::Loaded) {
    event!(
        loaded,
        "message_start",
        shared::MessageStartEvent<String>,
        (),
        json!({"message":"start"}),
        Value::Null
    );
    event!(loaded,"message_update",shared::MessageUpdateEvent<String,Value>,(),json!({"message":"update","assistantMessageEvent":{"type":"text_delta"}}),Value::Null);
    event!(
        loaded,
        "message_end",
        shared::MessageEndEvent<String>,
        shared::MessageEndEventResult<String>,
        json!({"message":"end"}),
        json!({"message":"replacement"})
    );
    event!(
        loaded,
        "resources_discover",
        ResourcesDiscoverEvent,
        ResourcesDiscoverResult,
        json!({"cwd":"/resources","reason":"reload"}),
        json!({"skillPaths":["skill"]})
    );
}

/// Native aliases retain complete session-owner preparations and persisted entries.
fn check_native_session_operands() {
    let preparation = check::<maestro_session::compaction::CompactionPreparation>(
        json!({"firstKeptEntryId":"kept","messagesToSummarize":[{"role":"custom","content":"message"}],"turnPrefixMessages":[{"role":"user","content":"prefix"}],"isSplitTurn":false,"tokensBefore":-1.5,"previousSummary":"previous","fileOps":{"read":["r"],"written":["w"],"edited":["e"]},"settings":{"enabled":false,"reserveTokens":2.5,"keepRecentTokens":3.5}}),
    );
    let event = SessionBeforeCompactEvent {
        data: SessionBeforeCompactEventData {
            preparation,
            branch_entries: vec![json!({"id":"branch","parentId":"parent"})],
            custom_instructions: Presence::Present("instructions".into()),
        },
        signal: maestro_models::Cancellation::new(),
    };
    assert_eq!(event.preparation.first_kept_entry_id, "kept");
    assert_eq!(event.branch_entries[0]["parentId"], "parent");
    let stored = event.signal.clone();
    event.signal.abort();
    drop(event);
    assert!(stored.is_aborted());
    for from_extension in [false, true] {
        check::<SessionCompactEvent>(
            json!({"compactionEntry":{"type":"compaction","id":"compact","parentId":"parent","timestamp":"2026-10-17T00:00:00Z","summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5,"details":{"z":1,"a":2},"fromHook":false},"fromExtension":from_extension}),
        );
    }
    optional::<SessionTreeEvent>(
        json!({"newLeafId":"new","oldLeafId":null}),
        &[
            (
                "summaryEntry",
                json!({"type":"branch_summary","id":"branch","parentId":null,"timestamp":"2026-10-17T00:00:00Z","fromId":"root","summary":"summary","details":false,"fromHook":false}),
            ),
            ("fromExtension", json!(true)),
        ],
    );
    optional::<SessionBeforeCompactResult>(
        json!({}),
        &[
            ("cancel", json!(false)),
            (
                "compaction",
                json!({"summary":"summary","firstKeptEntryId":"kept","tokensBefore":2.5,"details":false}),
            ),
        ],
    );
    let base = json!({"targetId":"target","oldLeafId":null,"commonAncestorId":"common","entriesToSummarize":[{"id":"entry"}],"userWantsSummary":true});
    optional::<TreePreparation>(
        base,
        &[
            ("customInstructions", json!("")),
            ("replaceInstructions", json!(false)),
            ("label", json!("label")),
        ],
    );
}

/// Native messages and streaming observations keep distinct shared owners.
fn check_native_message_owners() {
    let user: NativeAgentMessage = maestro_agent::AgentMessage::User(Arc::new(
        std::sync::RwLock::new(maestro_models::UserMessage {
            content: maestro_models::UserContent::Text("independent".into()),
            timestamp: 1.0,
        }),
    ));
    let custom = Arc::new(std::sync::RwLock::new(
        maestro_session::messages::CustomAgentMessages::Custom(
            maestro_session::messages::CustomMessage {
                custom_type: "notice".into(),
                content: maestro_models::UserContent::Text("custom".into()),
                display: false,
                details: Presence::Present(json!({"value":3})),
                timestamp: 2.0,
            },
        ),
    ));
    let context = ContextEvent {
        messages: vec![
            user.clone(),
            maestro_agent::AgentMessage::Custom(custom.clone()),
        ],
    };
    assert!(matches!(
        &context.messages[..],
        [
            maestro_agent::AgentMessage::User(_),
            maestro_agent::AgentMessage::Custom(_)
        ]
    ));
    let maestro_agent::AgentMessage::Custom(retained) = &context.messages[1] else {
        panic!("custom")
    };
    assert!(Arc::ptr_eq(retained, &custom));
    check_native_stream_owners(&user);
}

/// Every existing stream branch supplies the same retained native observation.
fn stream_variants(
    shared: &maestro_models::SharedAssistantMessage,
) -> Vec<maestro_models::AssistantMessageEvent> {
    use maestro_models::AssistantMessageEvent as E;
    vec![
        E::Start {
            partial: shared.clone(),
        },
        E::TextStart {
            content_index: 0,
            partial: shared.clone(),
        },
        E::TextDelta {
            content_index: 0,
            delta: "delta".into(),
            partial: shared.clone(),
        },
        E::TextEnd {
            content_index: 0,
            content: "text".into(),
            partial: shared.clone(),
        },
        E::ThinkingStart {
            content_index: 1,
            partial: shared.clone(),
        },
        E::ThinkingDelta {
            content_index: 1,
            delta: "thinking".into(),
            partial: shared.clone(),
        },
        E::ThinkingEnd {
            content_index: 1,
            content: "thinking".into(),
            partial: shared.clone(),
        },
        E::ToolcallStart {
            content_index: 2,
            partial: shared.clone(),
        },
        E::ToolcallDelta {
            content_index: 2,
            delta: "arguments".into(),
            partial: shared.clone(),
        },
        E::ToolcallEnd {
            content_index: 2,
            tool_call: stream_tool_call(),
            partial: shared.clone(),
        },
        E::Done {
            reason: maestro_models::DoneReason::Stop,
            message: shared.clone(),
        },
        E::Error {
            reason: maestro_models::ErrorReason::Error,
            error: shared.clone(),
        },
    ]
}

/// Streaming payload owners remain independent from the event message.
fn check_native_stream_owners(user: &NativeAgentMessage) {
    use maestro_models::AssistantMessageEvent as Stream;
    let message=serde_json::from_value(json!({"role":"assistant","content":[{"type":"text","text":"stream"}],"api":"fixture","provider":"fixture","model":"model","usage":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"totalTokens":10.0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"stop","timestamp":3.0})).unwrap();
    let shared = Arc::new(std::sync::RwLock::new(message));
    for stream in stream_variants(&shared) {
        let event = MessageUpdateEvent {
            message: user.clone(),
            assistant_message_event: stream,
        };
        let observed = match &event.assistant_message_event {
            Stream::Start { partial }
            | Stream::TextStart { partial, .. }
            | Stream::TextDelta { partial, .. }
            | Stream::TextEnd { partial, .. }
            | Stream::ThinkingStart { partial, .. }
            | Stream::ThinkingDelta { partial, .. }
            | Stream::ThinkingEnd { partial, .. }
            | Stream::ToolcallStart { partial, .. }
            | Stream::ToolcallDelta { partial, .. }
            | Stream::ToolcallEnd { partial, .. } => partial,
            Stream::Done { message, .. } => message,
            Stream::Error { error, .. } => error,
        };
        assert!(Arc::ptr_eq(observed, &shared));
        assert!(matches!(
            event.message,
            maestro_agent::AgentMessage::User(_)
        ));
    }
}

/// Complete tool-call operands carried by the terminal stream observation.
fn stream_tool_call() -> maestro_models::ToolCall {
    maestro_models::ToolCall {
        id: "call".into(),
        name: "custom".into(),
        arguments: serde_json::Map::new(),
        thought_signature: None,
    }
}

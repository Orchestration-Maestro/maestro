//! Typed replacement hooks through registered native callbacks.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::local_support as support;
use maestro_extensions::*;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

#[test]
fn provider_payload_chains_every_present_json_kind() {
    support::local(provider_payload_chains_every_present_json_kind_case());
}

use maestro_agent::AgentMessage;
use maestro_session::Presence;
use std::sync::{Arc, RwLock};

fn native_messages() -> Vec<NativeAgentMessage> {
    let standard: Vec<AgentMessage> = serde_json::from_value(json!([
        {"role":"user","content":[{"type":"text","text":"user"}],"timestamp":1},
        {"role":"assistant","content":[{"type":"text","text":"assistant","textSignature":"signature"}],"api":"fixture","provider":"fixture","model":"fixture","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":2},
        {"role":"toolResult","toolCallId":"call","toolName":"tool","content":[{"type":"text","text":"tool"}],"details":{"nested":{"value":1}},"isError":false,"timestamp":3}
    ])).unwrap();
    let mut messages: Vec<_> = standard
        .into_iter()
        .map(|message| match message {
            AgentMessage::User(value) => AgentMessage::User(value),
            AgentMessage::Assistant(value) => AgentMessage::Assistant(value),
            AgentMessage::ToolResult(value) => AgentMessage::ToolResult(value),
            AgentMessage::Custom(_) => unreachable!(),
        })
        .collect();
    messages.push(AgentMessage::Custom(Arc::new(RwLock::new(
        maestro_session::messages::CustomAgentMessages::Custom(serde_json::from_value(json!({"role":"custom","customType":"custom","content":"custom","details":{"nested":{"value":1}},"display":true,"timestamp":4})).unwrap())
    ))));
    messages
}

fn message_values(messages: &[NativeAgentMessage]) -> Value {
    Value::Array(
        messages
            .iter()
            .map(|message| match message {
                AgentMessage::Custom(value) => {
                    let value = value.read().unwrap();
                    let maestro_session::messages::CustomAgentMessages::Custom(value) = &*value
                    else {
                        panic!("custom")
                    };
                    serde_json::to_value(value).unwrap()
                }
                _ => serde_json::to_value(message.to_message().unwrap()).unwrap(),
            })
            .collect(),
    )
}

#[test]
fn context_messages_are_deep_copied_once_and_chain_owned_edits() {
    support::local(context_messages_are_deep_copied_once_and_chain_owned_edits_case());
}

#[test]
fn payload_edits_survive_failure_and_use_rust_assignment() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("payload", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        loaded
            .api
            .on::<BeforeProviderRequestEvent, Value>(
                "before_provider_request".into(),
                Rc::new(|event, _| {
                    event.payload["nested"]["value"] = json!(2);
                    Box::pin(std::future::ready(Err("nested failure".into())))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<BeforeProviderRequestEvent, Value>(
                "before_provider_request".into(),
                Rc::new(|event, _| {
                    assert_eq!(event.payload["nested"]["value"], 2);
                    event.payload = json!(["assigned"]);
                    Box::pin(std::future::ready(Err("assignment failure".into())))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<BeforeProviderRequestEvent, Value>(
                "before_provider_request".into(),
                Rc::new(|event, _| {
                    assert_eq!(event.payload, json!(["assigned"]));
                    Box::pin(std::future::ready(Ok(None)))
                }),
            )
            .unwrap();
        assert_eq!(
            runner
                .emit_before_provider_request(json!({"nested":{"value":1}}))
                .await
                .unwrap(),
            json!(["assigned"])
        );
        bus.settle().await;
    });
}

#[test]
fn context_edits_survive_failure_and_empty_replacement() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("context", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        loaded
            .api
            .on::<ContextEvent, ContextEventResult>(
                "context".into(),
                Rc::new(|event, _| {
                    let AgentMessage::Custom(value) = &event.messages[3] else {
                        panic!("custom")
                    };
                    let mut value = value.write().unwrap();
                    let maestro_session::messages::CustomAgentMessages::Custom(value) = &mut *value
                    else {
                        panic!("custom")
                    };
                    value.details = Presence::Present(json!({"nested":{"value":7}}));
                    Box::pin(std::future::ready(Err("nested failure".into())))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<ContextEvent, ContextEventResult>(
                "context".into(),
                Rc::new(|event, _| {
                    assert_eq!(
                        message_values(&event.messages)[3]["details"]["nested"]["value"],
                        7
                    );
                    event.messages = vec![native_messages()[0].clone()];
                    Box::pin(std::future::ready(Err("assignment failure".into())))
                }),
            )
            .unwrap();
        loaded
            .api
            .on::<ContextEvent, ContextEventResult>(
                "context".into(),
                Rc::new(|event, _| {
                    assert_eq!(event.messages.len(), 1);
                    Box::pin(std::future::ready(Ok(Some(ContextEventResult {
                        messages: Presence::Present(vec![]),
                    }))))
                }),
            )
            .unwrap();
        assert!(
            runner
                .emit_context(&native_messages())
                .await
                .unwrap()
                .is_empty()
        );
        bus.settle().await;
    });
}

struct SnapshotPayload {
    values: Vec<u32>,
    reads: Arc<std::sync::atomic::AtomicU32>,
}
impl Clone for SnapshotPayload {
    fn clone(&self) -> Self {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            values: self.values.clone(),
            reads: self.reads.clone(),
        }
    }
}
impl maestro_agent::CustomAgentMessages for SnapshotPayload {
    fn role(&self) -> &'static str {
        "snapshot"
    }
}
#[test]
fn custom_payload_clone_reads_shared_state_and_keeps_independent_snapshot() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("snapshot", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let reads = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let payload = Arc::new(RwLock::new(SnapshotPayload {
            values: vec![1],
            reads: reads.clone(),
        }));
        let original = AgentMessage::Custom(payload.clone());
        let result = runner.emit_context(&[original]).await.unwrap();
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        payload.write().unwrap().values.push(2);
        let AgentMessage::Custom(snapshot) = &result[0] else {
            panic!("custom")
        };
        assert_eq!(snapshot.read().unwrap().values, [1]);
        assert_eq!(payload.read().unwrap().values, [1, 2]);
        bus.settle().await;
    });
}

#[test]
fn message_end_keeps_role_and_marks_identical_replacements() {
    support::local(message_end_keeps_role_and_marks_identical_replacements_case());
}

#[test]
fn message_end_reports_changed_role_without_losing_prior_message() {
    support::local(message_end_reports_changed_role_without_losing_prior_message_case());
}

struct TypedRole {
    role: String,
    data: Vec<u32>,
}
impl maestro_agent::CustomAgentMessages for TypedRole {
    fn role(&self) -> &str {
        &self.role
    }
}
#[test]
fn message_replacements_keep_typed_custom_data_and_signatures() {
    support::local(async {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("typed", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        loaded
            .api
            .on::<MessageEndEvent, MessageEndEventResult>(
                "message_end".into(),
                Rc::new(retain_message),
            )
            .unwrap();
        let mut event = MessageEndEvent {
            message: native_messages()[1].clone(),
        };
        let result = runner.emit_message_end(&mut event).await.unwrap().unwrap();
        assert_eq!(
            message_values(&[result])[0]["content"][0]["textSignature"],
            "signature"
        );
        loaded.api.on::<MessageEndEvent<AgentMessage<TypedRole>>, MessageEndEventResult<AgentMessage<TypedRole>>>("message_end".into(), Rc::new(|_, _| {
            Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {message:Presence::Present(AgentMessage::Custom(Arc::new(RwLock::new(TypedRole {role:"special".into(), data:vec![7,9]}))))}))))
        })).unwrap();
        let mut typed = MessageEndEvent {
            message: AgentMessage::Custom(Arc::new(RwLock::new(TypedRole {
                role: "special".into(),
                data: vec![1],
            }))),
        };
        let result = runner.emit_message_end(&mut typed).await.unwrap().unwrap();
        let AgentMessage::Custom(value) = result else {
            panic!("custom")
        };
        assert_eq!(value.read().unwrap().data, [7, 9]);
        bus.settle().await;
    });
}

#[test]
fn input_hooks_chain_text_and_keep_missing_or_null_images() {
    support::local(input_hooks_chain_text_and_keep_missing_or_null_images_case());
}
fn image(data: &str) -> maestro_models::ImageContent {
    serde_json::from_value(json!({"type":"image","data":data,"mimeType":"image/png"})).unwrap()
}

#[test]
fn input_continue_keeps_edits_and_explicit_image_transform_counts_as_change() {
    support::local(input_continue_keeps_edits_and_explicit_image_transform_counts_as_change_case());
}
#[test]
fn input_source_and_handled_verdict_control_later_handlers() {
    support::local(input_source_and_handled_verdict_control_later_handlers_case());
}

#[test]
fn resource_paths_keep_kind_order_duplicates_and_source() {
    support::local(resource_paths_keep_kind_order_duplicates_and_source_case());
}

fn prompt_options() -> BuildSystemPromptOptions {
    serde_json::from_value(json!({"cwd":"relative","customPrompt":"custom","selectedTools":["read","read"],"toolSnippets":{"z":"last","a":"first"},"promptGuidelines":["rule"],"appendSystemPrompt":"append","contextFiles":[{"path":"instructions","content":"context"}],"skills":[{"name":"skill","description":"instruction","filePath":"skill.md","baseDir":".","sourceInfo":{"path":"skill.md","source":"local","scope":"temporary","origin":"top-level"},"disableModelInvocation":false}]})).unwrap()
}
#[test]
fn agent_start_hooks_accumulate_messages_and_empty_prompts() {
    support::local(agent_start_hooks_accumulate_messages_and_empty_prompts_case());
}

#[test]
fn concurrent_agent_start_hooks_keep_independent_prompt_readers() {
    support::local(concurrent_agent_start_hooks_keep_independent_prompt_readers_case());
}

#[test]
fn retained_prompt_context_keeps_final_value_and_stale_guard() {
    support::local(retained_prompt_context_keeps_final_value_and_stale_guard_case());
}

async fn provider_payload_chains_every_present_json_kind_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("payload", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let values = [
        Value::Null,
        json!(false),
        json!(0),
        json!(-0.0),
        json!(""),
        json!("text"),
        json!([]),
        json!([1]),
        json!({}),
        json!({"z":1,"a":2}),
    ];
    let seen = Rc::new(RefCell::new(Vec::new()));
    install_payload_values(&loaded, &seen, values);
    let result = runner
        .emit_before_provider_request(json!({"initial":true}))
        .await
        .unwrap();
    assert_eq!(
        *seen.borrow(),
        vec![
            json!({"initial":true}),
            Value::Null,
            json!(false),
            json!(0),
            json!(-0.0),
            json!(""),
            json!("text"),
            json!([]),
            json!([1]),
            json!({}),
            json!({"z":1,"a":2})
        ]
    );
    assert!(seen.borrow()[4].as_f64().unwrap().is_sign_negative());
    assert_eq!(
        result
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(result, json!({"z":1,"a":2}));
    bus.settle().await;
}

async fn context_messages_are_deep_copied_once_and_chain_owned_edits_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("context", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let original = native_messages();
    let before = message_values(&original);
    let replacement = native_messages()[0].clone();
    let retained = replacement.clone();
    let expected_replacement = replacement.clone();
    loaded
        .api
        .on::<ContextEvent, ContextEventResult>(
            "context".into(),
            Rc::new(move |event, _| {
                edit_context_payloads(&event.messages);
                event.messages.push(replacement.clone());
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<ContextEvent, ContextEventResult>(
            "context".into(),
            Rc::new(move |event, _| {
                assert_eq!(event.messages.len(), 5);
                let values = message_values(&event.messages);
                assert_eq!(values[0]["content"], "changed user");
                assert_eq!(values[1]["content"], json!([]));
                assert_eq!(values[2]["details"]["nested"]["value"], 2);
                assert_eq!(values[3]["details"]["nested"]["value"], 2);
                let retained = retained.clone();
                Box::pin(std::future::ready(Ok(Some(ContextEventResult {
                    messages: Presence::Present(vec![retained]),
                }))))
            }),
        )
        .unwrap();
    let result = runner.emit_context(&original).await.unwrap();
    assert_eq!(message_values(&original), before);
    let (AgentMessage::User(result), AgentMessage::User(original_replacement)) =
        (&result[0], &expected_replacement)
    else {
        panic!("user")
    };
    assert!(Arc::ptr_eq(result, original_replacement));
    bus.settle().await;
}

async fn message_end_keeps_role_and_marks_identical_replacements_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("message", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let mut event = MessageEndEvent {
        message: native_messages()[0].clone(),
    };
    assert!(runner.emit_message_end(&mut event).await.unwrap().is_none());
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(|_, _| Box::pin(std::future::ready(Ok(None)))),
        )
        .unwrap();
    assert!(runner.emit_message_end(&mut event).await.unwrap().is_none());
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>("message_end".into(), Rc::new(retain_message))
        .unwrap();
    let result = runner.emit_message_end(&mut event).await.unwrap().unwrap();
    let (AgentMessage::User(actual), AgentMessage::User(original)) = (&result, &event.message)
    else {
        panic!("user")
    };
    assert!(Arc::ptr_eq(actual, original));
    replace_user_message(&loaded, &runner, &mut event).await;
    bus.settle().await;
}

async fn message_end_reports_changed_role_without_losing_prior_message_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("message", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let diagnostics = Rc::new(RefCell::new(Vec::new()));
    let observed = diagnostics.clone();
    let owner = Rc::downgrade(&loaded.owner);
    let _remove = runner.on_error(Rc::new(move |error| {
        owner.upgrade().unwrap().write().unwrap().path = "listener-edited".into();
        observed.borrow_mut().push((
            error.extension_path.clone(),
            error.event.clone(),
            error.error.clone(),
        ));
        Ok(())
    }));
    install_role_replacements(&loaded);
    let mut event = MessageEndEvent {
        message: native_messages()[0].clone(),
    };
    assert!(runner.emit_message_end(&mut event).await.unwrap().is_some());
    assert_eq!(
        *diagnostics.borrow(),
        [(
            "message".into(),
            "message_end".into(),
            "message_end handlers must return a message with the same role".into()
        )]
    );
    bus.settle().await;
}

async fn input_hooks_chain_text_and_keep_missing_or_null_images_case() {
    for (answers, expected) in [
        (vec![], json!({"action":"continue"})),
        (
            vec![None, Some(json!({"action":"continue"}))],
            json!({"action":"continue"}),
        ),
        (
            vec![
                Some(json!({"action":"transform","text":"edited"})),
                Some(json!({"action":"transform","text":"original","images":null})),
            ],
            json!({"action":"continue"}),
        ),
        (
            vec![
                Some(json!({"action":"transform","text":"edited"})),
                Some(json!({"action":"transform","text":"","images":null})),
            ],
            json!({"action":"transform","text":"","images":[{"type":"image","data":"original","mimeType":"image/png"}]}),
        ),
        (
            vec![Some(
                json!({"action":"transform","text":"original","images":[]}),
            )],
            json!({"action":"transform","text":"original","images":[]}),
        ),
        (
            vec![Some(
                json!({"action":"transform","text":"original","images":[{"type":"image","data":"different","mimeType":"image/png"}]}),
            )],
            json!({"action":"transform","text":"original","images":[{"type":"image","data":"different","mimeType":"image/png"}]}),
        ),
    ] {
        input_chain_case(answers, expected).await;
    }
    for initial in [Presence::Missing, Presence::Null] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("input", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let mut images = initial;
        assert!(matches!(
            runner
                .emit_input("original", &mut images, InputSource::Rpc, None)
                .await
                .unwrap(),
            InputEventResult::Continue
        ));
        bus.settle().await;
    }
}

async fn input_continue_keeps_edits_and_explicit_image_transform_counts_as_change_case() {
    for explicit in [false, true] {
        for initial in [vec![], vec![image("original")]] {
            input_image_intent_case(explicit, initial).await;
        }
    }
}

async fn input_source_and_handled_verdict_control_later_handlers_case() {
    for source in [
        InputSource::Interactive,
        InputSource::Rpc,
        InputSource::Extension,
    ] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("input", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let trace = support::Trace::default();
        install_handled_input(&loaded, source, &trace);
        assert!(matches!(
            runner
                .emit_input("original", &mut Presence::Missing, source, None)
                .await
                .unwrap(),
            InputEventResult::Handled
        ));
        assert_eq!(*trace.borrow(), ["handled"]);
        bus.settle().await;
    }
}

async fn resource_paths_keep_kind_order_duplicates_and_source_case() {
    for reason in [
        ResourcesDiscoverReason::Startup,
        ResourcesDiscoverReason::Reload,
    ] {
        let bus = support::LocalBus::new();
        let loaded = support::loaded("resources", bus.controller.for_extension());
        let runner = support::runner(&loaded.owners);
        let errors = Rc::new(RefCell::new(Vec::new()));
        let errors_out = errors.clone();
        let _remove = runner.on_error(Rc::new(move |error| {
            errors_out.borrow_mut().push(error.extension_path.clone());
            Ok(())
        }));
        install_resource_paths(&loaded, reason);
        install_resource_tail(&loaded);
        let result = runner
            .emit_resources_discover("relative/../root".into(), reason)
            .await
            .unwrap();
        let pairs = |paths: Vec<maestro_extensions::runner::ResourcePath>| {
            paths
                .into_iter()
                .map(|path| (path.path, path.extension_path))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            pairs(result.skill_paths),
            [
                ("z".into(), "current".into()),
                ("a".into(), "current".into()),
                ("z".into(), "current".into()),
                ("last".into(), "failure-path".into())
            ]
        );
        assert_eq!(
            pairs(result.prompt_paths),
            [
                ("p".into(), "current".into()),
                ("p".into(), "current".into()),
                ("last".into(), "failure-path".into())
            ]
        );
        assert_eq!(
            pairs(result.theme_paths),
            [
                ("t".into(), "current".into()),
                ("last".into(), "failure-path".into())
            ]
        );
        assert_eq!(*errors.borrow(), ["failure-path"]);
        bus.settle().await;
    }
}

async fn agent_start_hooks_accumulate_messages_and_empty_prompts_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("start", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    assert!(
        runner
            .emit_before_agent_start(
                "user".into(),
                &mut Presence::Missing,
                "base".into(),
                prompt_options()
            )
            .await
            .unwrap()
            .is_none()
    );
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let message = |text| json!({"customType":"notice","content":text,"display":false});
    install_agent_start_chain(&loaded, &prompts);
    let result = runner
        .emit_before_agent_start(
            "user".into(),
            &mut Presence::Present(vec![image("image")]),
            "base".into(),
            prompt_options(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        *prompts.borrow(),
        ["base", "base", "base", "", "final", "final"]
    );
    assert_eq!(result.system_prompt.as_deref(), Some("final"));
    assert_eq!(
        serde_json::to_value(result.messages.unwrap()).unwrap(),
        json!([message("one"), message("two"), message("three")])
    );
    bus.settle().await;
}

async fn concurrent_agent_start_hooks_keep_independent_prompt_readers_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("concurrent", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    let (release, waiting) = tokio::sync::oneshot::channel();
    let retained = Rc::new(RefCell::new(Vec::new()));
    install_concurrent_prompt_chain(&loaded, waiting, &retained);
    let mut images_a = Presence::Missing;
    let mut first = std::pin::pin!(runner.emit_before_agent_start(
        "A".into(),
        &mut images_a,
        "A-base".into(),
        prompt_options()
    ));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(std::future::Future::poll(first.as_mut(), &mut context).is_pending());
    let second = runner
        .emit_before_agent_start(
            "B".into(),
            &mut Presence::Missing,
            "B-base".into(),
            prompt_options(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.system_prompt.as_deref(), Some("B-final"));
    assert_eq!(
        retained.borrow()[0].1.get_system_prompt().unwrap(),
        "A-base"
    );
    release.send(()).unwrap();
    assert_eq!(
        first.await.unwrap().unwrap().system_prompt.as_deref(),
        Some("")
    );
    assert_eq!(retained.borrow()[0].1.get_system_prompt().unwrap(), "");
    assert_eq!(
        retained.borrow()[1].1.get_system_prompt().unwrap(),
        "B-final"
    );
    bus.settle().await;
}

async fn retained_prompt_context_keeps_final_value_and_stale_guard_case() {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("retained", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    bind_prompt(&runner, "ordinary");
    let ordinary = runner.create_context();
    let retained = Rc::new(RefCell::new(None));
    let captured = retained.clone();
    loaded
        .api
        .on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |_, context| {
                *captured.borrow_mut() = Some(context);
                Box::pin(std::future::ready(Ok(Some(BeforeAgentStartEventResult {
                    message: Presence::Missing,
                    system_prompt: Presence::Present("final".into()),
                }))))
            }),
        )
        .unwrap();
    runner
        .emit_before_agent_start(
            "retained".into(),
            &mut Presence::Missing,
            "initial".into(),
            prompt_options(),
        )
        .await
        .unwrap();
    let retained = retained.borrow().as_ref().unwrap().clone();
    assert_eq!(retained.get_system_prompt().unwrap(), "final");
    assert_eq!(ordinary.get_system_prompt().unwrap(), "ordinary");
    bind_prompt(&runner, "rebound");
    assert_eq!(retained.get_system_prompt().unwrap(), "final");
    assert_eq!(ordinary.get_system_prompt().unwrap(), "rebound");
    runner.invalidate(Some("stale".into()));
    assert_eq!(
        retained.get_system_prompt().unwrap_err().to_string(),
        "stale"
    );
    assert_eq!(
        ordinary.get_system_prompt().unwrap_err().to_string(),
        "stale"
    );
    bus.settle().await;
}

fn install_payload_values(
    loaded: &support::Loaded,
    seen: &Rc<RefCell<Vec<Value>>>,
    values: [Value; 10],
) {
    for value in values {
        let seen = seen.clone();
        loaded
            .api
            .on::<BeforeProviderRequestEvent, BeforeProviderRequestEventResult>(
                "before_provider_request".into(),
                Rc::new(move |event, _| {
                    seen.borrow_mut().push(event.payload.clone());
                    let value = value.clone();
                    Box::pin(std::future::ready(Ok(Some(value))))
                }),
            )
            .unwrap();
    }
    let seen_last = seen.clone();
    loaded
        .api
        .on::<BeforeProviderRequestEvent, BeforeProviderRequestEventResult>(
            "before_provider_request".into(),
            Rc::new(move |event, _| {
                seen_last.borrow_mut().push(event.payload.clone());
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
}

fn edit_context_payloads(messages: &[NativeAgentMessage]) {
    for message in messages {
        match message {
            AgentMessage::User(value) => {
                value.write().unwrap().content =
                    maestro_models::UserContent::Text("changed user".into());
            }
            AgentMessage::Assistant(value) => {
                value.write().unwrap().content.clear();
            }
            AgentMessage::ToolResult(value) => {
                let mut value = value.write().unwrap();
                value.content.clear();
                value.details = Some(json!({"nested":{"value":2}}));
            }
            AgentMessage::Custom(value) => {
                let mut value = value.write().unwrap();
                let maestro_session::messages::CustomAgentMessages::Custom(value) = &mut *value
                else {
                    panic!("custom")
                };
                let Presence::Present(details) = &mut value.details else {
                    panic!("details")
                };
                details["nested"]["value"] = json!(2);
            }
        }
    }
}

fn retain_message(
    event: &mut MessageEndEvent,
    _context: ExtensionContext,
) -> <maestro_models::Local as maestro_models::Execution>::Future<
    '_,
    Result<Option<MessageEndEventResult>, EventError>,
> {
    let message = event.message.clone();
    Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {
        message: Presence::Present(message),
    }))))
}

async fn replace_user_message(
    loaded: &support::Loaded,
    runner: &ExtensionRunner<maestro_models::Local>,
    event: &mut MessageEndEvent,
) {
    let next = native_messages()[0].clone();
    let AgentMessage::User(value) = &next else {
        panic!("user")
    };
    value.write().unwrap().content = maestro_models::UserContent::Text("replacement".into());
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(move |_, _| {
                let next = next.clone();
                Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {
                    message: Presence::Present(next),
                }))))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(|event, _| {
                assert_eq!(
                    message_values(std::slice::from_ref(&event.message))[0]["content"],
                    "replacement"
                );
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
    let result = runner.emit_message_end(event).await.unwrap().unwrap();
    assert_eq!(message_values(&[result])[0]["content"], "replacement");
}

fn install_role_replacements(loaded: &support::Loaded) {
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(|_, _| {
                let message = native_messages()[0].clone();
                let AgentMessage::User(value) = &message else {
                    panic!("user")
                };
                value.write().unwrap().content =
                    maestro_models::UserContent::Text("accepted".into());
                Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {
                    message: Presence::Present(message),
                }))))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {
                    message: Presence::Present(native_messages()[1].clone()),
                }))))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<MessageEndEvent, MessageEndEventResult>(
            "message_end".into(),
            Rc::new(|event, _| {
                assert_eq!(
                    message_values(std::slice::from_ref(&event.message))[0]["content"],
                    json!("accepted")
                );
                let message = native_messages()[0].clone();
                Box::pin(std::future::ready(Ok(Some(MessageEndEventResult {
                    message: Presence::Present(message),
                }))))
            }),
        )
        .unwrap();
}

async fn input_chain_case(answers: Vec<Option<Value>>, expected: Value) {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("input", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    for answer in answers {
        loaded
            .api
            .on::<InputEvent, InputEventResult>(
                "input".into(),
                Rc::new(move |_, _| {
                    let answer = answer.clone();
                    Box::pin(std::future::ready(Ok(
                        answer.map(|value| serde_json::from_value(value).unwrap())
                    )))
                }),
            )
            .unwrap();
    }
    let mut images = Presence::Present(vec![image("original")]);
    let result = runner
        .emit_input("original", &mut images, InputSource::Interactive, None)
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(result).unwrap(), expected);
    bus.settle().await;
}

async fn input_image_intent_case(explicit: bool, initial: Vec<maestro_models::ImageContent>) {
    let bus = support::LocalBus::new();
    let loaded = support::loaded("input", bus.controller.for_extension());
    let runner = support::runner(&loaded.owners);
    loaded
        .api
        .on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                if explicit {
                    let images = event.images.clone();
                    let text = event.text.clone();
                    Box::pin(std::future::ready(Ok(Some(InputEventResult::Transform(
                        InputTransform { text, images },
                    )))))
                } else {
                    let Presence::Present(images) = &mut event.images else {
                        panic!("images")
                    };
                    images.push(image("edited"));
                    Box::pin(std::future::ready(Ok(Some(InputEventResult::Continue))))
                }
            }),
        )
        .unwrap();
    let mut images = Presence::Present(initial.clone());
    let result = runner
        .emit_input("same", &mut images, InputSource::Interactive, None)
        .await
        .unwrap();
    if explicit {
        assert!(matches!(result, InputEventResult::Transform(_)));
        assert_eq!(
            serde_json::to_value(&images).unwrap(),
            serde_json::to_value(Presence::Present(initial)).unwrap()
        );
    } else {
        assert!(matches!(result, InputEventResult::Continue));
        let Presence::Present(images) = images else {
            panic!("images")
        };
        assert_eq!(images.len(), initial.len() + 1);
        assert_eq!(images.last().unwrap().data, "edited");
    }
    bus.settle().await;
}

fn install_handled_input(loaded: &support::Loaded, source: InputSource, trace: &support::Trace) {
    loaded
        .api
        .on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                assert_eq!(event.source, source);
                Box::pin(std::future::ready(Err("input failure".into())))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(InputEventResult::Transform(
                    InputTransform {
                        text: "transformed".into(),
                        images: Presence::Missing,
                    },
                )))))
            }),
        )
        .unwrap();
    let reached = trace.clone();
    loaded
        .api
        .on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |event, _| {
                assert_eq!(event.text, "transformed");
                support::mark(&reached, "handled");
                Box::pin(std::future::ready(Ok(Some(InputEventResult::Handled))))
            }),
        )
        .unwrap();
    let later = trace.clone();
    loaded
        .api
        .on::<InputEvent, InputEventResult>(
            "input".into(),
            Rc::new(move |_, _| {
                support::mark(&later, "later");
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
}

fn install_resource_paths(loaded: &support::Loaded, reason: ResourcesDiscoverReason) {
    let owner = Rc::downgrade(&loaded.owner);
    loaded
        .api
        .on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
            "resources_discover".into(),
            Rc::new(move |event, _| {
                assert_eq!(event.cwd, "relative/../root");
                assert_eq!(event.reason, reason);
                owner.upgrade().unwrap().write().unwrap().path = "current".into();
                Box::pin(std::future::ready(Ok(Some(ResourcesDiscoverResult {
                    skill_paths: Presence::Present(vec!["z".into(), "a".into(), "z".into()]),
                    prompt_paths: Presence::Present(vec!["p".into(), "p".into()]),
                    theme_paths: Presence::Present(vec!["t".into()]),
                }))))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
            "resources_discover".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(ResourcesDiscoverResult {
                    skill_paths: Presence::Null,
                    prompt_paths: Presence::Missing,
                    theme_paths: Presence::Present(vec![]),
                }))))
            }),
        )
        .unwrap();
}
fn install_resource_tail(loaded: &support::Loaded) {
    let owner = Rc::downgrade(&loaded.owner);
    loaded
        .api
        .on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
            "resources_discover".into(),
            Rc::new(move |_, _| {
                owner.upgrade().unwrap().write().unwrap().path = "failure-path".into();
                Box::pin(std::future::ready(Err("resource failure".into())))
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
            "resources_discover".into(),
            Rc::new(|_, _| {
                Box::pin(std::future::ready(Ok(Some(ResourcesDiscoverResult {
                    skill_paths: Presence::Present(vec!["last".into()]),
                    prompt_paths: Presence::Present(vec!["last".into()]),
                    theme_paths: Presence::Present(vec!["last".into()]),
                }))))
            }),
        )
        .unwrap();
}

fn install_agent_start_chain(loaded: &support::Loaded, prompts: &support::Trace) {
    let message = |text| json!({"customType":"notice","content":text,"display":false});
    for answer in [
        None,
        Some(json!({"message":message("one")})),
        Some(json!({"systemPrompt":""})),
        Some(json!({"message":message("two"),"systemPrompt":"final"})),
        Some(json!({"message":message("three")})),
        Some(json!("failure")),
    ] {
        let prompts = prompts.clone();
        loaded
            .api
            .on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
                "before_agent_start".into(),
                Rc::new(move |event, ctx| {
                    assert_eq!(event.prompt, "user");
                    assert_eq!(
                        serde_json::to_value(&event.system_prompt_options).unwrap(),
                        serde_json::to_value(prompt_options()).unwrap()
                    );
                    let Presence::Present(images) = &event.images else {
                        panic!("images")
                    };
                    assert_eq!(images[0].data, "image");
                    assert_eq!(ctx.get_system_prompt().unwrap(), event.system_prompt);
                    prompts.borrow_mut().push(event.system_prompt.clone());
                    let answer = answer.clone();
                    Box::pin(std::future::ready(match answer {
                        Some(value) if value == "failure" => Err("start failure".into()),
                        Some(value) => Ok(Some(serde_json::from_value(value).unwrap())),
                        None => Ok(None),
                    }))
                }),
            )
            .unwrap();
    }
}

fn install_concurrent_prompt_chain(
    loaded: &support::Loaded,
    waiting: tokio::sync::oneshot::Receiver<()>,
    retained: &Rc<RefCell<Vec<(String, ExtensionContext)>>>,
) {
    let waiting = RefCell::new(Some(waiting));
    let contexts = retained.clone();
    loaded
        .api
        .on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |event, context| {
                let label = event.prompt.clone();
                let initial = event.system_prompt.clone();
                contexts.borrow_mut().push((label.clone(), context.clone()));
                let waiting = if label == "A" {
                    waiting.borrow_mut().take()
                } else {
                    None
                };
                Box::pin(async move {
                    if let Some(waiting) = waiting {
                        waiting.await.unwrap();
                    }
                    assert_eq!(context.get_system_prompt().unwrap(), initial);
                    Ok(Some(BeforeAgentStartEventResult {
                        message: Presence::Missing,
                        system_prompt: Presence::Present(format!("{label}-next")),
                    }))
                })
            }),
        )
        .unwrap();
    loaded
        .api
        .on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(|event, context| {
                assert_eq!(
                    context.get_system_prompt().unwrap(),
                    format!("{}-next", event.prompt)
                );
                let prompt = if event.prompt == "A" { "" } else { "B-final" };
                Box::pin(std::future::ready(Ok(Some(BeforeAgentStartEventResult {
                    message: Presence::Missing,
                    system_prompt: Presence::Present(prompt.into()),
                }))))
            }),
        )
        .unwrap();
}

fn bind_prompt(runner: &ExtensionRunner<maestro_models::Local>, prompt: &str) {
    let prompt = prompt.to_owned();
    runner
        .bind_core(
            maestro_models::Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || Ok(prompt.clone())),
                ..Default::default()
            },
            types::ProviderActions::default(),
        )
        .unwrap();
}

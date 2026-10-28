//! Prompts through a real application session: idle and tool turns, images, skill and
//! template expansion, extension commands and messages, and the streaming and credential
//! refusals, compared with the recorded operation cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, HarnessOptions, Scripted, answer, as_recorded, command_extension, delivery_requests,
    delivery_view, expected, harness, observed_preflight, on_task_set, start_prompt, template,
    waiting_tool,
};
use indexmap::IndexMap;
use maestro_agent::{AgentToolRecord, AgentToolResult, SharedAgentTool};
use maestro_app::PromptOptions;
use maestro_models::{
    FauxResponseStep, ImageContent, Local, TextContent, Tool, UserBlock, UserContent,
};
use maestro_resources::{SyntheticSourceOptions, create_synthetic_source_info};
use maestro_session::messages::SendUserMessageOptions;
use serde_json::json;
use std::{cell::RefCell, rc::Rc, sync::RwLock};
use tokio::sync::Notify;

/// Executable tools that record `name:value` and answer with it.
fn echoing_tools(
    runs: &Rc<RefCell<Vec<String>>>,
    names: &[&str],
) -> IndexMap<String, SharedAgentTool<Local>> {
    names
        .iter()
        .map(|name| {
            let name = (*name).to_owned();
            let runs = Rc::clone(runs);
            let recorded = name.clone();
            let tool = AgentToolRecord::<Local> {
                definition: Tool {
                    name: name.clone(),
                    description: format!("{name} tool"),
                    parameters: json!({
                        "type": "object",
                        "properties": {"text": {"type": "string"}, "value": {"type": "string"}},
                    })
                    .into(),
                },
                description: None,
                label: name.clone(),
                prepare_arguments: None,
                execute: Rc::new(move |_, arguments, _, _| {
                    let value = arguments["text"]
                        .as_str()
                        .or(arguments["value"].as_str())
                        .unwrap_or_default();
                    let text = format!("{recorded}:{value}");
                    runs.borrow_mut().push(text.clone());
                    Box::pin(std::future::ready(Ok(AgentToolResult {
                        content: vec![UserBlock::Text(TextContent::new(text))],
                        details: Some(json!({})),
                        terminate: None,
                    })))
                }),
                execution_mode: None,
            };
            (name, Rc::new(RwLock::new(tool)) as SharedAgentTool<Local>)
        })
        .collect()
}

#[test]
fn idle_prompt_records_one_answer() {
    on_task_set(|_| async {
        let expected = expected("idle_prompt_records_one_answer");
        let h = harness(HarnessOptions::default()).await;
        h.faux
            .set_responses(vec![FauxResponseStep::Message(Box::new(answer(
                "hello", None, None,
            )))]);
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("hi", options).await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["response_pending"] = json!(h.faux.get_pending_response_count());
        observed["preflight"] = json!(*accepted.borrow());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn tool_turn_waits_for_the_final_answer() {
    on_task_set(|_| async {
        let expected = expected("tool_turn_waits_for_the_final_answer");
        let runs = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            base_tools_override: Some(echoing_tools(&runs, &["echo"])),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![("echo".into(), json!({"text": "hello"}))],
                    ..Scripted::default()
                },
                Scripted::text("done"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let result = h.session.prompt("start", PromptOptions::default()).await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        observed["runs"] = json!(*runs.borrow());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn tool_batch_reaches_one_continuation() {
    on_task_set(|_| async {
        let expected = expected("tool_batch_reaches_one_continuation");
        let runs = Rc::new(RefCell::new(Vec::new()));
        let h = harness(HarnessOptions {
            base_tools_override: Some(echoing_tools(&runs, &["slow", "fast"])),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![
                        ("slow".into(), json!({"value": "a"})),
                        ("fast".into(), json!({"value": "b"})),
                    ],
                    ..Scripted::default()
                },
                Scripted::text("tool results: 2"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let result = h
            .session
            .prompt("run tools", PromptOptions::default())
            .await;
        assert!(result.is_ok());
        let mut completed = runs.borrow().clone();
        completed.sort();
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        observed["runs"] = json!(completed);
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn prompt_images_reach_the_provider() {
    on_task_set(|_| async {
        let expected = expected("prompt_images_reach_the_provider");
        let h = harness(HarnessOptions {
            responses: Some(vec![Scripted::text("ok")]),
            ..HarnessOptions::default()
        })
        .await;
        let options = PromptOptions {
            images: Some(vec![ImageContent {
                data: "ZmFrZQ==".into(),
                mime_type: "image/png".into(),
            }]),
            ..PromptOptions::default()
        };
        let result = h.session.prompt("describe", options).await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn skill_input_reaches_the_provider_expanded() {
    on_task_set(|_| async {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let directory = scratch.path().join("skills");
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("test-skill.md");
        std::fs::write(&file, "# Test Skill\n\nUse the skill body.").unwrap();
        let root = directory.display().to_string();
        let skill = maestro_resources::Skill {
            name: "test".into(),
            description: "Test skill".into(),
            source_info: create_synthetic_source_info(
                file.display().to_string(),
                SyntheticSourceOptions {
                    source: "local".into(),
                    scope: None,
                    origin: None,
                    base_dir: None,
                },
            ),
            file_path: file.display().to_string(),
            base_dir: root.clone(),
            disable_model_invocation: false,
        };
        let h = harness(HarnessOptions {
            skills: vec![skill],
            scratch: Some(scratch),
            responses: Some(vec![Scripted::text("ok")]),
            ..HarnessOptions::default()
        })
        .await;
        let result = h
            .session
            .prompt("/skill:test explain this", PromptOptions::default())
            .await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        assert_eq!(
            as_recorded(observed, &[(&root, "$SKILL_ROOT")]),
            expected("skill_input_reaches_the_provider_expanded")
        );
        h.session.dispose();
    });
}

#[test]
fn template_input_reaches_the_provider_expanded() {
    on_task_set(|_| async {
        let expected = expected("template_input_reaches_the_provider_expanded");
        let h = harness(HarnessOptions {
            prompts: vec![template("review", "Review this code: $1")],
            responses: Some(vec![Scripted::text("ok")]),
            ..HarnessOptions::default()
        })
        .await;
        let result = h
            .session
            .prompt("/review src/index.ts", PromptOptions::default())
            .await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn idle_command_keeps_a_scripted_answer() {
    on_task_set(|_| async {
        let expected = expected("idle_command_keeps_a_scripted_answer");
        let heard = Rc::new(RefCell::new(None));
        let h = harness(HarnessOptions {
            factories: vec![command_extension("testcmd", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        h.faux
            .set_responses(vec![FauxResponseStep::Message(Box::new(answer(
                "should stay queued",
                None,
                None,
            )))]);
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("/testcmd hello world", options).await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["runs"] = json!([*heard.borrow()]);
        observed["response_pending"] = json!(h.faux.get_pending_response_count());
        observed["preflight"] = json!(*accepted.borrow());
        observed["calls"] = json!(h.faux.state.call_count());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn idle_extension_message_starts_a_turn() {
    on_task_set(|_| async {
        let expected = expected("idle_extension_message_starts_a_turn");
        let h = harness(HarnessOptions {
            responses: Some(vec![Scripted::text("response")]),
            ..HarnessOptions::default()
        })
        .await;
        let result = h
            .session
            .send_user_message(
                UserContent::Text("from extension".into()),
                SendUserMessageOptions::default(),
            )
            .await;
        assert!(result.is_ok());
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn busy_prompt_reports_the_queue_choice() {
    on_task_set(|_| async {
        let expected = expected("busy_prompt_reports_the_queue_choice");
        let started = Rc::new(Notify::new());
        let release = Rc::new(Notify::new());
        let h = harness(HarnessOptions {
            base_tools_override: Some(IndexMap::from([(
                "wait".to_owned(),
                waiting_tool(Rc::clone(&started), Rc::clone(&release)),
            )])),
            responses: Some(vec![
                Scripted {
                    tool_calls: vec![("wait".into(), json!({}))],
                    ..Scripted::default()
                },
                Scripted::text("done"),
            ]),
            ..HarnessOptions::default()
        })
        .await;
        let running = start_prompt(&h, "start");
        started.notified().await;
        let during = json!({
            "pending": h.session.pending_message_count(),
            "steering": h.session.get_steering_messages(),
            "followUp": h.session.get_follow_up_messages(),
        });
        let (accepted, options) = observed_preflight();
        let refused = h.session.prompt("second", options).await;
        release.notify_one();
        running.await.unwrap().unwrap();
        h.session.agent().wait_for_idle().await;
        let mut observed = delivery_view(&h);
        observed["requests"] = delivery_requests(&h);
        observed["during"] = during;
        observed["error"] = json!(refused.unwrap_err().message);
        observed["preflight"] = json!(*accepted.borrow());
        assert_eq!(observed, expected);
        h.session.dispose();
    });
}

#[test]
fn unauthenticated_prompt_fails_before_dispatch() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            auth: Auth::None,
            ..HarnessOptions::default()
        })
        .await;
        let (accepted, options) = observed_preflight();
        let refused = h.session.prompt("hi", options).await;
        let mut observed = delivery_view(&h);
        observed["error"] = json!(refused.unwrap_err().message);
        observed["preflight"] = json!(*accepted.borrow());
        observed["calls"] = json!(h.faux.state.call_count());
        let docs = maestro_app::configuration::config::get_docs_path().unwrap();
        assert_eq!(
            as_recorded(observed, &[(&docs, "$DOCS")]),
            expected("unauthenticated_prompt_fails_before_dispatch")
        );
        h.session.dispose();
    });
}

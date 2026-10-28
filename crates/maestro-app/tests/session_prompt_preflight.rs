//! Sending a prompt: registered commands, input handlers, skill and template expansion,
//! streaming admission, credentials, message order and the deferred shell flush, compared
//! with the recorded prompt cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, Transcript, agent_ended, answer, as_recorded, assert_transcript,
    assistant, case, command_extension, entries_view, harness, history_view, hold_first_answer,
    input_extension, labelled, load_history, messages_view, observed_preflight, of_kinds,
    on_task_set, outcome, queue_view, recorded_skills, script_responses, shell_entries,
    start_and_abandon, start_prompt, supplied_failure, template, transcribed, usage, user,
    with_placeholders,
};
use maestro_app::agent_session::PromptOptions;
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ExtensionFactory, InputEventResult,
    InputTransform,
};
use maestro_models::{ImageContent, Local};
use maestro_session::Presence;
use maestro_session::messages::{CustomMessageInput, SendMessageOptions, UserMessageDelivery};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The recorded prompt case `name`.
fn expected(name: &str) -> Value {
    case("prompt_preflight", name)
}

/// Asserts the observations every prompt case shares, the stored turn entries included.
fn assert_prompt_case(h: &Harness, t: &Transcript, case: &Value) {
    assert_transcript(h, t, case);
    assert_eq!(queue_view(h, t), case["queue"]);
    assert_eq!(
        with_placeholders(json!(entries_view(h)), h),
        case["entries"]
    );
}

/// The image every recorded case attaches.
fn image() -> ImageContent {
    ImageContent {
        data: "AAAA".into(),
        mime_type: "image/png".into(),
    }
}

/// The extension whose before-agent-start handler answers each prompt with `answer`.
fn start_extension(
    answer: impl Fn(&BeforeAgentStartEvent) -> Option<BeforeAgentStartEventResult> + 'static,
) -> ExtensionFactory<Local> {
    let answer = Rc::new(answer);
    Rc::new(move |api| {
        let answer = Rc::clone(&answer);
        let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |event, _| Box::pin(std::future::ready(Ok(answer(event))))),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// Sends one prompt to a session over `options` and asserts it against `name`.
async fn single_prompt_case(name: &str, options: HarnessOptions, text: &str) {
    let expected = expected(name);
    let (h, t) = transcribed(options).await;
    script_responses(
        &h,
        vec![crate::support::session_harness::answer("reply", None, None)],
    );
    let result = h.session.prompt(text, PromptOptions::default()).await;
    assert_eq!(json!([outcome(result)]), expected["results"]);
    assert_prompt_case(&h, &t, &expected);
}

/// Options over the recorded skills and templates in a fresh project.
fn with_resources() -> HarnessOptions {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    HarnessOptions {
        skills: recorded_skills(&scratch),
        prompts: vec![template("plan", "Plan for $1 with $2")],
        scratch: Some(scratch),
        ..HarnessOptions::default()
    }
}

#[test]
fn registered_command_runs_while_a_response_streams() {
    on_task_set(|_| async {
        let expected = expected("a registered command runs even while a turn is streaming");
        let heard = Rc::new(RefCell::new(None));
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![command_extension("hello", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let (requested, release) = hold_first_answer(&h, "first", &[]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let (accepted, options) = observed_preflight();
        let ran = h.session.prompt("/hello now", options).await;
        release.notify_one();
        running.await.unwrap().unwrap();
        let results = json!([
            outcome(ran),
            labelled("preflight", &*accepted.borrow()),
            labelled("commandArgs", &*heard.borrow()),
        ]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn handled_input_sends_nothing_and_accepts() {
    on_task_set(|_| async {
        let expected = expected("an input handler that handles the text stops the prompt");
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![input_extension(|_| Ok(Some(InputEventResult::Handled)))],
            ..HarnessOptions::default()
        })
        .await;
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("skipped", options).await;
        let results = json!([outcome(result), labelled("preflight", &*accepted.borrow())]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn input_transform_replaces_text_and_images() {
    on_task_set(|_| async {
        let transform = input_extension(|event| {
            Ok(Some(InputEventResult::Transform(InputTransform {
                text: format!("transformed {}", event.text),
                images: Presence::Present(vec![image()]),
            })))
        });
        let options = HarnessOptions {
            factories: vec![transform],
            ..HarnessOptions::default()
        };
        single_prompt_case(
            "an input transform replaces text and images",
            options,
            "original",
        )
        .await;
    });
}

#[test]
fn skill_command_expands_with_arguments() {
    on_task_set(|_| async {
        let name = "a skill command expands to its block with arguments";
        single_prompt_case(
            name,
            with_resources(),
            "/skill:review please look at main.rs",
        )
        .await;
    });
}

#[test]
fn skill_command_without_arguments_expands_alone() {
    on_task_set(|_| async {
        let name = "a skill command without arguments expands to the block alone";
        single_prompt_case(name, with_resources(), "/skill:review").await;
    });
}

#[test]
fn unknown_skill_command_keeps_the_text() {
    on_task_set(|_| async {
        let name = "an unknown skill command is left alone";
        single_prompt_case(name, with_resources(), "/skill:unknown keep me").await;
    });
}

#[test]
fn unreadable_skill_file_reports_and_keeps_the_text() {
    on_task_set(|_| async {
        let mut expected = expected("an unreadable skill file reports and keeps the text");
        let (h, t) = transcribed(with_resources()).await;
        script_responses(
            &h,
            vec![crate::support::session_harness::answer("reply", None, None)],
        );
        let result = h
            .session
            .prompt("/skill:gone keep me", PromptOptions::default())
            .await;
        assert_eq!(json!([outcome(result)]), expected["results"]);
        // The failure text is the platform's own read error for the missing file.
        let missing = format!(
            "{}/project/skills/review/missing.md",
            h.scratch.path().display()
        );
        let error = std::fs::read(&missing).unwrap_err().to_string();
        expected["errorEvents"][0]["error"] = with_placeholders(json!(error), &h);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn prompt_template_expands_before_sending() {
    on_task_set(|_| async {
        let name = "a prompt template expands before sending";
        single_prompt_case(name, with_resources(), "/plan alpha beta").await;
    });
}

#[test]
fn prompt_during_a_response_needs_a_queue_choice() {
    on_task_set(|_| async {
        let expected = expected("a streaming prompt without a behaviour is refused");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &[]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let (accepted, options) = observed_preflight();
        let refused = h.session.prompt("second", options).await;
        release.notify_one();
        running.await.unwrap().unwrap();
        let results = json!([outcome(refused), labelled("preflight", &*accepted.borrow())]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

/// Queues `text` with `behavior` while the first answer streams and asserts `name`; the
/// queued text is delivered by the agent after the first answer.
async fn queued_prompt_case(name: &str, behavior: UserMessageDelivery, images: Vec<ImageContent>) {
    let expected = expected(name);
    let (h, t) = transcribed(HarnessOptions::default()).await;
    let (requested, release) = hold_first_answer(&h, "first", &["second"]);
    let running = start_prompt(&h, "first");
    requested.notified().await;
    let text = if images.is_empty() { "later" } else { "look" };
    let options = PromptOptions {
        streaming_behavior: Some(behavior),
        images: (!images.is_empty()).then_some(images),
        ..PromptOptions::default()
    };
    let queued = h.session.prompt(text, options).await;
    let streaming = queue_view(&h, &t);
    release.notify_one();
    running.await.unwrap().unwrap();
    let results = json!([outcome(queued), labelled("queueWhileStreaming", streaming)]);
    assert_eq!(results, expected["results"]);
    assert_prompt_case(&h, &t, &expected);
    let updates = t.events_of(&["queue_update"]);
    let recorded = crate::support::session_harness::of_kinds(
        expected["sessionEvents"].as_array().unwrap(),
        &["queue_update"],
    );
    assert_eq!(updates, recorded);
}

#[test]
fn follow_up_queue_holds_the_message_until_the_turn_ends() {
    on_task_set(|_| async {
        let name = "a streaming prompt queues a followed-up message when selected";
        queued_prompt_case(name, UserMessageDelivery::FollowUp, Vec::new()).await;
    });
}

#[test]
fn steer_queue_holds_text_and_images_until_the_turn_ends() {
    on_task_set(|_| async {
        let name = "a streaming prompt queues steering and its images after the text";
        queued_prompt_case(name, UserMessageDelivery::Steer, vec![image()]).await;
    });
}

#[test]
fn accepted_prompt_sends_user_then_next_turn_then_hook_messages() {
    on_task_set(|_| async {
        let expected =
            expected("an accepted prompt sends the user message then next-turn then hook messages");
        let hook = start_extension(|_| {
            Some(BeforeAgentStartEventResult {
                message: Presence::Present(CustomMessageInput {
                    custom_type: "aside".into(),
                    content: maestro_models::UserContent::Text("hook text".into()),
                    display: true,
                    details: Presence::Present(json!({"from": "hook"})),
                }),
                system_prompt: Presence::Missing,
            })
        });
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![hook],
            ..HarnessOptions::default()
        })
        .await;
        script_responses(
            &h,
            vec![crate::support::session_harness::answer("reply", None, None)],
        );
        let note = CustomMessageInput {
            custom_type: "note".into(),
            content: maestro_models::UserContent::Text("next turn".into()),
            display: true,
            details: Presence::Present(json!({"a": 1})),
        };
        let next_turn = SendMessageOptions {
            trigger_turn: None,
            deliver_as: Some(maestro_session::messages::MessageDelivery::NextTurn),
        };
        let queued = h.session.send_custom_message(note, next_turn).await;
        let result = h.session.prompt("question", PromptOptions::default()).await;
        assert_eq!(
            json!([outcome(queued), outcome(result)]),
            expected["results"]
        );
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn hook_prompt_override_resets_on_the_next_prompt() {
    on_task_set(|_| async {
        let expected =
            expected("a hook prompt replaces the system prompt and the next prompt resets it");
        let hook = start_extension(|event| {
            (event.prompt == "replace").then(|| BeforeAgentStartEventResult {
                message: Presence::Missing,
                system_prompt: Presence::Present("replaced".into()),
            })
        });
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![hook],
            ..HarnessOptions::default()
        })
        .await;
        let answer = crate::support::session_harness::answer;
        script_responses(
            &h,
            vec![answer("first", None, None), answer("second", None, None)],
        );
        let first = h.session.prompt("replace", PromptOptions::default()).await;
        let after_replace = h.session.system_prompt();
        let second = h.session.prompt("plain", PromptOptions::default()).await;
        let after_plain = queue_view(&h, &t)["systemPrompt"].clone();
        let results = json!([
            outcome(first),
            labelled("afterReplace", after_replace),
            outcome(second),
            labelled("afterPlain", after_plain),
        ]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn prompt_without_credentials_is_refused() {
    on_task_set(|_| async {
        let expected = expected("a prompt without credentials is refused");
        let (h, t) = transcribed(HarnessOptions {
            auth: Auth::None,
            ..HarnessOptions::default()
        })
        .await;
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("hi", options).await;
        let docs = maestro_app::configuration::config::get_docs_path().unwrap();
        let results = json!([outcome(result), labelled("preflight", &*accepted.borrow())]);
        assert_eq!(
            as_recorded(results, &[(&docs, "$DOCS")]),
            expected["results"]
        );
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn throwing_input_handler_is_reported_and_the_prompt_continues() {
    on_task_set(|_| async {
        let expected = expected("a throwing input handler is reported and the prompt continues");
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![input_extension(|_| Err("input exploded"))],
            ..HarnessOptions::default()
        })
        .await;
        script_responses(
            &h,
            vec![crate::support::session_harness::answer("reply", None, None)],
        );
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("hi", options).await;
        let results = json!([outcome(result), labelled("preflight", &*accepted.borrow())]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn throwing_command_handler_is_reported_and_counts_as_handled() {
    on_task_set(|_| async {
        let expected = expected("a throwing command handler is reported and counts as handled");
        let heard = Rc::new(RefCell::new(None));
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![command_extension("boom", heard, Some("command exploded"))],
            ..HarnessOptions::default()
        })
        .await;
        let (accepted, options) = observed_preflight();
        let result = h.session.prompt("/boom", options).await;
        let results = json!([outcome(result), labelled("preflight", &*accepted.borrow())]);
        assert_eq!(results, expected["results"]);
        assert_prompt_case(&h, &t, &expected);
    });
}

#[test]
fn deferred_shell_result_flushes_before_the_next_prompt() {
    on_task_set(|_| async {
        let expected = expected("a deferred shell result flushes before the next prompt");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["second"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let operations =
            crate::support::session_harness::ScriptedShell::output("waiting", Some(0.0));
        let options = maestro_app::agent_session::BashRunOptions {
            exclude_from_context: None,
            operations: Some(Rc::new(operations)),
        };
        let shell = h.session.execute_bash("echo waiting", None, options).await;
        let pending = h.session.has_pending_bash_messages();
        let during = messages_view(&h);
        release.notify_one();
        running.await.unwrap().unwrap();
        let after = h.session.has_pending_bash_messages();
        let result = h.session.prompt("second", PromptOptions::default()).await;
        let results = json!([
            {"ok": crate::support::session_harness::bash_result_view(&shell.unwrap())},
            labelled("pendingDuringTurn", pending),
            labelled("messagesDuringTurn", during),
            labelled("pendingAfterTurn", after),
            outcome(result),
        ]);
        assert_eq!(results, expected["results"]);
        assert_transcript(&h, &t, &expected);
        assert_eq!(queue_view(&h, &t), expected["queue"]);
        assert_eq!(json!(shell_entries(&entries_view(&h))), expected["entries"]);
    });
}

#[test]
fn prompt_compacts_after_an_aborted_full_response_before_sending() {
    on_task_set(|_| async {
        let (h, _) = transcribed(HarnessOptions {
            settings: json!({"compaction": {"keepRecentTokens": 1}}),
            context_window: Some(200_000.0),
            ..HarnessOptions::default()
        })
        .await;
        let aborted = json!({"stopReason": "aborted", "usage": usage([199_999, 0, 0, 0], 0.0, Some(199_999))});
        for record in [
            user(&json!("one"), 1),
            assistant("first", 2, &json!({})),
            user(&json!("two"), 3),
            assistant("", 4, &aborted),
        ] {
            drop(
                h.session
                    .session_manager()
                    .borrow_mut()
                    .append_message(record)
                    .unwrap(),
            );
        }
        load_history(&h);
        let requests = script_responses(
            &h,
            vec![
                answer("auto compacted", None, None),
                answer("reply", None, None),
            ],
        );
        h.session
            .prompt("three", PromptOptions::default())
            .await
            .unwrap();
        let kinds: Vec<Value> = h
            .events
            .borrow()
            .iter()
            .map(|event| event["type"].clone())
            .collect();
        assert_eq!(kinds, ["compaction_start", "compaction_end"]);
        assert_eq!(h.events.borrow()[0]["reason"], "threshold");
        assert_eq!(requests.lock().unwrap().len(), 2);
        let history = messages_view(&h);
        let roles: Vec<&Value> = history.iter().map(|message| &message["role"]).collect();
        assert_eq!(
            roles,
            [
                "compactionSummary",
                "user",
                "assistant",
                "user",
                "assistant"
            ]
        );
        assert_eq!(history[0]["summary"], "auto compacted");
        assert_eq!(history[3]["content"]["blocks"][0]["text"], "three");
    });
}

#[test]
fn supplied_system_prompt_reaches_the_first_prompt() {
    on_task_set(|_| async {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let record = Rc::clone(&seen);
        let observer = start_extension(move |event| {
            record.borrow_mut().push(event.system_prompt.clone());
            None
        });
        let h = harness(HarnessOptions {
            system_prompt: Some("Supplied instructions."),
            factories: vec![observer],
            ..HarnessOptions::default()
        })
        .await;
        script_responses(&h, vec![answer("reply", None, None)]);
        h.session
            .prompt("hi", PromptOptions::default())
            .await
            .unwrap();
        let installed = h.session.system_prompt();
        assert!(
            installed.starts_with("Supplied instructions.\n"),
            "{installed}"
        );
        assert_eq!(*seen.borrow(), [installed]);
    });
}

#[test]
fn input_transform_clearing_images_keeps_an_empty_list() {
    on_task_set(|_| async {
        let clear = input_extension(|event| {
            Ok(Some(InputEventResult::Transform(InputTransform {
                text: event.text.clone(),
                images: Presence::Present(Vec::new()),
            })))
        });
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![clear],
            ..HarnessOptions::default()
        })
        .await;
        script_responses(&h, vec![answer("reply", None, None)]);
        let options = PromptOptions {
            images: Some(vec![image()]),
            ..PromptOptions::default()
        };
        h.session.prompt("look", options).await.unwrap();
        let starts = of_kinds(&t.extension.borrow(), &["before_agent_start"]);
        assert_eq!(starts[0]["images"], json!(0));
        let history = messages_view(&h);
        assert_eq!(history[0]["content"]["blocks"].as_array().unwrap().len(), 1);
    });
}

#[test]
fn failing_error_listener_keeps_its_supplied_fields() {
    on_task_set(|_| async {
        let heard = Rc::new(RefCell::new(None));
        let h = harness(HarnessOptions {
            factories: vec![command_extension("boom", heard, Some("command exploded"))],
            ..HarnessOptions::default()
        })
        .await;
        let _unsubscribe = h
            .runner()
            .on_error(Rc::new(|_| Err(Box::new(supplied_failure()))));
        let failure = h.session.prompt("/boom", PromptOptions::default()).await;
        assert_eq!(failure.unwrap_err(), supplied_failure());
    });
}

#[test]
fn failing_credential_lookup_keeps_its_supplied_fields() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            auth: Auth::None,
            ..HarnessOptions::default()
        })
        .await;
        h.registry
            .auth_storage()
            .set_fallback_resolver(|_| Err(supplied_failure().into()));
        let failure = h.session.prompt("hi", PromptOptions::default()).await;
        assert_eq!(failure.unwrap_err(), supplied_failure());
    });
}

/// The extension whose before-agent-start handler waits for `gate` once.
fn gated_start(gate: Rc<tokio::sync::Notify>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let gate = Rc::clone(&gate);
        let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |_, _| {
                let gate = Rc::clone(&gate);
                Box::pin(async move {
                    gate.notified().await;
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

#[test]
fn abandoned_prompt_still_delivers_its_next_turn_messages() {
    on_task_set(|_| async {
        let gate = Rc::new(tokio::sync::Notify::new());
        let spawned = Rc::new(std::cell::Cell::new(0));
        let count = Rc::clone(&spawned);
        let h = harness(HarnessOptions {
            factories: vec![gated_start(Rc::clone(&gate))],
            spawn: Some(Rc::new(move |task| {
                count.set(count.get() + 1);
                drop(tokio::task::spawn_local(task));
            })),
            ..HarnessOptions::default()
        })
        .await;
        script_responses(&h, vec![answer("reply", None, None)]);
        let note = CustomMessageInput {
            custom_type: "note".into(),
            content: maestro_models::UserContent::Text("next turn".into()),
            display: true,
            details: Presence::Missing,
        };
        let next_turn = SendMessageOptions {
            trigger_turn: None,
            deliver_as: Some(maestro_session::messages::MessageDelivery::NextTurn),
        };
        h.session
            .send_custom_message(note, next_turn)
            .await
            .unwrap();
        let ended = agent_ended(&h);
        start_and_abandon(h.session.prompt("question", PromptOptions::default()));
        assert_eq!(spawned.get(), 1, "the session's facility holds the prompt");
        gate.notify_one();
        ended.notified().await;
        let kinds: Vec<Value> = messages_view(&h)
            .iter()
            .map(|message| message["customType"].clone())
            .collect();
        assert_eq!(kinds, [Value::Null, json!("note"), Value::Null]);
    });
}

#[test]
fn prompt_starts_before_observer_is_polled() {
    on_task_set(|_| async {
        let refused = harness(HarnessOptions {
            auth: Auth::None,
            ..HarnessOptions::default()
        })
        .await;
        let order = Rc::new(RefCell::new(Vec::new()));
        let queued = Rc::clone(&order);
        drop(refused.session.subscribe(Rc::new(move |event| {
            if let maestro_app::agent_session::AgentSessionEvent::QueueUpdate { .. } = event {
                queued.borrow_mut().push("queue".to_owned());
            }
        })));
        let checked = Rc::clone(&order);
        let options = PromptOptions {
            preflight_result: Some(Rc::new(move |accepted| {
                checked.borrow_mut().push(format!("preflight:{accepted}"));
            })),
            ..PromptOptions::default()
        };
        let observer = refused.session.prompt("refused", options);
        refused.session.steer("tail", &[]).unwrap();
        assert_eq!(*order.borrow(), ["preflight:false", "queue"]);
        assert!(observer.await.is_err());

        let gate = Rc::new(tokio::sync::Notify::new());
        let inputs = Rc::new(RefCell::new(Vec::new()));
        let heard = Rc::clone(&inputs);
        let accepted = harness(HarnessOptions {
            factories: vec![
                input_extension(move |event| {
                    heard.borrow_mut().push(event.text.clone());
                    Ok(None)
                }),
                gated_start(Rc::clone(&gate)),
            ],
            ..HarnessOptions::default()
        })
        .await;
        script_responses(&accepted, vec![answer("reply", None, None)]);
        let ended = agent_ended(&accepted);
        drop(
            accepted
                .session
                .prompt("accepted", PromptOptions::default()),
        );
        assert_eq!(
            *inputs.borrow(),
            ["accepted"],
            "the prompt began at invocation"
        );
        gate.notify_one();
        ended.notified().await;
        assert_eq!(
            history_view(&accepted),
            [
                json!({"role": "user", "text": "accepted"}),
                json!({"role": "assistant", "text": "reply"})
            ]
        );
    });
}

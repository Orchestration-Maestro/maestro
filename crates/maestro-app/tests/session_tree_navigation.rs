//! Moving around the session tree, compared with the recorded navigation cases: result,
//! leaf, appended entries, rebuilt history, summary prompts and extension events.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, HarnessOptions, Names, answer, as_recorded, build_tree, case, harness, history_view,
    load_history, on_task_set, script_responses,
};
use maestro_app::agent_session::NavigateTreeResult;
use maestro_extensions::{
    ExtensionFactory, SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionTreeEvent,
};
use maestro_models::{AssistantMessage, DiagnosticErrorInfo, Local, StopReason};
use maestro_session::{
    Presence, SessionManager,
    session_manager::{NavigateTreeOptions, ReadonlySessionManager},
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// What a before-tree handler does with the event, and what it returns.
type Handler = Rc<dyn Fn(&mut SessionBeforeTreeEvent) -> Option<Value>>;

/// Renders a tree event of a kind as recorded.
type Views = Rc<dyn Fn(&str, &Value) -> Value>;

/// The history and tree names event views read once the tree is built.
type Built = Rc<RefCell<Option<(Rc<RefCell<SessionManager>>, Names)>>>;

/// One recorded navigation scenario.
#[derive(Default)]
struct Scenario {
    /// Credentials; a runtime key when absent.
    auth: Option<Auth>,
    /// The before-tree handler.
    handler: Option<Handler>,
    /// Scripted summary answers.
    responses: Vec<AssistantMessage>,
    /// Whether the leaf is reset before navigating.
    reset: bool,
    /// Changes to the tree before navigating.
    setup: Option<fn(&mut SessionManager, &Names)>,
    /// A file-backed history replacing the in-memory one.
    file: bool,
}

/// A handler returning the result written as JSON.
fn returns(result: Value) -> Handler {
    Rc::new(move |_| Some(result.clone()))
}

/// The recorded name of an entry identity: a tree name, else `e<position>`.
fn symbol(manager: &SessionManager, names: &Names, id: Option<&str>) -> Value {
    let Some(id) = id else { return Value::Null };
    if let Some((name, _)) = names.iter().find(|(_, value)| *value == id) {
        return json!(name);
    }
    let entries = manager.get_entries();
    let position = entries.iter().position(|entry| entry["id"] == id);
    json!(format!("e{}", position.unwrap()))
}

/// The recorded form of a stored summary entry.
fn summary_view(manager: &SessionManager, names: &Names, entry: &Value) -> Value {
    let sym = |key: &str| symbol(manager, names, entry[key].as_str());
    let from_id = if entry["fromId"] == "root" {
        json!("root")
    } else {
        sym("fromId")
    };
    json!({
        "type": entry["type"], "parentId": sym("parentId"), "fromId": from_id,
        "summary": entry["summary"], "details": entry.get("details").unwrap_or(&Value::Null),
        "fromHook": entry.get("fromHook").unwrap_or(&Value::Null),
    })
}

/// The recorded form of an appended entry.
fn appended_view(manager: &SessionManager, names: &Names, entry: &Value) -> Value {
    if entry["type"] != "label" {
        return summary_view(manager, names, entry);
    }
    json!({
        "type": "label",
        "targetId": symbol(manager, names, entry["targetId"].as_str()),
        "label": entry.get("label").unwrap_or(&Value::Null),
        "parentId": symbol(manager, names, entry["parentId"].as_str()),
    })
}

/// Navigation options as the recorded case spelled them.
fn navigate_options(options: &Value) -> NavigateTreeOptions {
    let text = |key: &str| options[key].as_str().map(str::to_owned);
    NavigateTreeOptions {
        summarize: options["summarize"].as_bool(),
        custom_instructions: text("customInstructions"),
        replace_instructions: options["replaceInstructions"].as_bool(),
        label: text("label"),
    }
}

/// An extension recording both tree events and applying `handler` before the move.
fn tree_recorder(
    log: &Rc<RefCell<Vec<Value>>>,
    handler: Option<Handler>,
    views: Views,
) -> ExtensionFactory<Local> {
    let log = Rc::clone(log);
    Rc::new(move |api| {
        let (before, handler, before_views) = (Rc::clone(&log), handler.clone(), Rc::clone(&views));
        api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |event, _| {
                let preparation = serde_json::to_value(&event.preparation).unwrap();
                before
                    .borrow_mut()
                    .push(before_views("session_before_tree", &preparation));
                let result = handler.as_ref().and_then(|handler| handler(event));
                let result = result.map(|result| serde_json::from_value(result).unwrap());
                Box::pin(std::future::ready(Ok(result)))
            }),
        )
        .unwrap();
        let (after, views) = (Rc::clone(&log), Rc::clone(&views));
        api.on::<SessionTreeEvent, ()>(
            "session_tree".into(),
            Rc::new(move |event, _| {
                let event = serde_json::to_value(&*event).unwrap();
                after.borrow_mut().push(views("session_tree", &event));
                Box::pin(std::future::ready(Ok(None)))
            }),
        )
        .unwrap();
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The recorded form of a tree event.
fn event_view(manager: &SessionManager, names: &Names, kind: &str, event: &Value) -> Value {
    let sym = |key: &str| symbol(manager, names, event[key].as_str());
    let or_null = |key: &str| event.get(key).cloned().unwrap_or(Value::Null);
    if kind == "session_tree" {
        let summary = event
            .get("summaryEntry")
            .map_or(Value::Null, |entry| summary_view(manager, names, entry));
        let from_extension = event
            .get("fromExtension")
            .cloned()
            .unwrap_or(json!("missing"));
        return json!({"type": kind, "newLeafId": sym("newLeafId"), "oldLeafId": sym("oldLeafId"),
            "summaryEntry": summary, "fromExtension": from_extension});
    }
    let entries: Vec<Value> = event["entriesToSummarize"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| symbol(manager, names, entry["id"].as_str()))
        .collect();
    json!({"type": kind, "targetId": sym("targetId"), "oldLeafId": sym("oldLeafId"),
        "commonAncestorId": sym("commonAncestorId"), "entriesToSummarize": entries,
        "userWantsSummary": event["userWantsSummary"], "customInstructions": or_null("customInstructions"),
        "replaceInstructions": or_null("replaceInstructions"), "label": or_null("label"), "signal": true})
}

/// The recorded form of a navigation result.
fn result_view(
    manager: &SessionManager,
    names: &Names,
    result: Result<NavigateTreeResult, DiagnosticErrorInfo>,
) -> Value {
    match result {
        Err(error) => json!({"error": error.message}),
        Ok(result) => {
            let summary = result.summary_entry.map_or(Value::Null, |entry| {
                summary_view(manager, names, &serde_json::to_value(entry).unwrap())
            });
            json!({"ok": {"editorText": result.editor_text, "cancelled": result.cancelled,
                "aborted": result.aborted.then_some(true), "summaryEntry": summary}})
        }
    }
}

/// Runs a scenario against its recorded case and returns the observations.
fn navigate(name: &str, scenario: Scenario) -> Value {
    let expected = case("navigation", name);
    let docs = maestro_app::configuration::config::get_docs_path().unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let state: Built = Rc::default();
    let shared = Rc::clone(&state);
    let views: Views = Rc::new(move |kind, event| {
        let shared = shared.borrow();
        let (manager, names) = shared.as_ref().unwrap();
        event_view(&manager.borrow(), names, kind, event)
    });
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    on_task_set(|_| async {
        let session_manager = scenario.file.then(|| {
            let sessions = format!("{root}/sessions");
            SessionManager::create(&format!("{root}/project"), Some(&sessions), &root).unwrap()
        });
        let h = harness(HarnessOptions {
            factories: vec![tree_recorder(&log, scenario.handler, views)],
            auth: scenario.auth.unwrap_or(Auth::Key),
            session_manager,
            ..HarnessOptions::default()
        })
        .await;
        let manager = Rc::clone(h.session.session_manager());
        let names = build_tree(&mut manager.borrow_mut());
        if scenario.reset {
            manager.borrow_mut().reset_leaf();
        }
        if let Some(setup) = scenario.setup {
            setup(&mut manager.borrow_mut(), &names);
        }
        *state.borrow_mut() = Some((Rc::clone(&manager), names.clone()));
        load_history(&h);
        let prompts = script_responses(&h, scenario.responses);
        let before = manager.borrow().get_entries().len();
        let target = expected["target"].as_str().unwrap();
        let target = names.get(target).map_or(target, String::as_str).to_owned();
        let result = h
            .session
            .navigate_tree(&target, navigate_options(&expected["navigateOptions"]))
            .await;
        let manager = manager.borrow();
        let messages = history_view(&h);
        let observed = json!({
            "name": name, "target": expected["target"], "navigateOptions": expected["navigateOptions"],
            "result": result_view(&manager, &names, result),
            "leaf": symbol(&manager, &names, manager.get_leaf_id()),
            "appended": manager.get_entries()[before..].iter().map(|entry| appended_view(&manager, &names, entry)).collect::<Vec<_>>(),
            "messages": messages,
            "prompts": *prompts.lock().unwrap(),
            "extensionEvents": *log.borrow(),
        });
        as_recorded(observed, &[(&docs, "$DOCS")])
    })
}

/// Runs a scenario and compares every observation with its recorded case.
fn check(name: &str, scenario: Scenario) {
    assert_eq!(navigate(name, scenario), case("navigation", name));
}

/// The default summarizer's answer.
fn generated() -> Vec<AssistantMessage> {
    vec![answer("Generated", None, None)]
}

#[test]
fn current_leaf_navigation_changes_nothing() {
    check("the current leaf is a no-op", Scenario::default());
}

#[test]
fn unknown_entry_navigation_is_refused() {
    check("an unknown entry is refused", Scenario::default());
}

#[test]
fn user_target_moves_to_parent_with_editor_text() {
    check(
        "a user message moves to its parent with editor text",
        Scenario::default(),
    );
}

#[test]
fn root_user_target_resets_leaf() {
    check("a root user message resets the leaf", Scenario::default());
}

#[test]
fn assistant_target_becomes_leaf() {
    check("an assistant message becomes the leaf", Scenario::default());
}

#[test]
fn custom_message_target_moves_to_parent_with_editor_text() {
    check(
        "a custom message moves to its parent with editor text",
        Scenario::default(),
    );
}

#[test]
fn label_without_summary_marks_target() {
    check(
        "a label without a summary marks the target",
        Scenario::default(),
    );
}

#[test]
fn extension_veto_leaves_history_unchanged() {
    let handler = Some(returns(json!({"cancel": true})));
    check(
        "an extension veto leaves history unchanged",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn false_veto_continues() {
    let handler = Some(returns(json!({"cancel": false})));
    check(
        "a false veto continues",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn extension_summary_is_stored_when_summarizing() {
    let handler = Some(returns(
        json!({"summary": {"summary": "From extension", "details": {"k": 1}}}),
    ));
    check(
        "an extension summary is used when summarizing",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn extension_summary_is_ignored_without_summarizing() {
    let handler = Some(returns(json!({"summary": {"summary": "From extension"}})));
    check(
        "an extension summary is ignored without summarizing",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn extension_overrides_reach_summarizer() {
    let handler = Some(returns(
        json!({"customInstructions": "ext focus", "replaceInstructions": true, "label": "ext label"}),
    ));
    let scenario = Scenario {
        handler,
        responses: generated(),
        ..Scenario::default()
    };
    check("extension overrides reach the summarizer", scenario);
}

#[test]
fn in_place_entry_edit_reaches_summarizer() {
    let handler: Handler = Rc::new(|event| {
        event.preparation.entries_to_summarize.remove(0);
        None
    });
    let scenario = Scenario {
        handler: Some(handler),
        responses: generated(),
        ..Scenario::default()
    };
    check(
        "an in-place edit of the prepared entries reaches the summarizer",
        scenario,
    );
}

/// Replacing the prepared list is indistinguishable from editing it in place, so the
/// summarizer sees the empty list and the move happens without a summary, exactly like
/// a plain move to the same entry.
#[test]
fn replaced_entry_list_reaches_summarizer() {
    let handler: Handler = Rc::new(|event| {
        event.preparation.entries_to_summarize = Vec::new();
        None
    });
    let scenario = Scenario {
        handler: Some(handler),
        responses: generated(),
        ..Scenario::default()
    };
    let observed = navigate(
        "a replaced prepared entry list does not reach the summarizer",
        scenario,
    );
    let recorded = case(
        "navigation",
        "a replaced prepared entry list does not reach the summarizer",
    );
    let plain = case("navigation", "an assistant message becomes the leaf");
    for key in ["result", "leaf", "appended", "messages", "prompts"] {
        assert_eq!(observed[key], plain[key], "{key}");
    }
    assert_eq!(
        observed["extensionEvents"][0],
        recorded["extensionEvents"][0]
    );
    assert_eq!(observed["extensionEvents"][1], plain["extensionEvents"][1]);
}

#[test]
fn edited_preparation_instructions_are_ignored() {
    let handler: Handler = Rc::new(|event| {
        event.preparation.custom_instructions = Presence::Present("edited".into());
        event.preparation.label = Presence::Present("edited".into());
        None
    });
    let scenario = Scenario {
        handler: Some(handler),
        responses: generated(),
        ..Scenario::default()
    };
    check(
        "edited preparation instructions do not reach the summarizer",
        scenario,
    );
}

#[test]
fn null_label_override_removes_label() {
    let handler = Some(returns(json!({"label": null})));
    check(
        "a null label override removes the label",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn default_summary_records_file_details() {
    let scenario = Scenario {
        responses: generated(),
        ..Scenario::default()
    };
    check("the default summarizer records file details", scenario);
}

#[test]
fn custom_instructions_extend_summary_prompt() {
    let scenario = Scenario {
        responses: generated(),
        ..Scenario::default()
    };
    check(
        "custom instructions are appended to the summary prompt",
        scenario,
    );
}

#[test]
fn nothing_to_summarize_navigates_plainly() {
    let scenario = Scenario {
        reset: true,
        responses: vec![answer("unused", None, None)],
        ..Scenario::default()
    };
    check("nothing to summarize navigates plainly", scenario);
}

#[test]
fn aborted_summary_cancels_navigation() {
    let responses = vec![answer("", Some(StopReason::Aborted), None)];
    check(
        "an aborted summary cancels navigation",
        Scenario {
            responses,
            ..Scenario::default()
        },
    );
}

#[test]
fn failed_summary_is_reported() {
    let responses = vec![answer("", Some(StopReason::Error), Some("boom"))];
    check(
        "a failed summary is reported",
        Scenario {
            responses,
            ..Scenario::default()
        },
    );
}

#[test]
fn empty_extension_summary_navigates_plainly() {
    let handler = Some(returns(json!({"summary": {"summary": ""}})));
    check(
        "an empty extension summary navigates plainly",
        Scenario {
            handler,
            ..Scenario::default()
        },
    );
}

#[test]
fn branch_without_messages_stores_no_content_summary() {
    let scenario = Scenario {
        setup: Some(|manager, names| {
            manager.branch(&names["a1"]).unwrap();
            manager.append_model_change("faux", "think").unwrap();
        }),
        responses: vec![answer("unused", None, None)],
        ..Scenario::default()
    };
    check(
        "a branch without messages stores a no-content summary",
        scenario,
    );
}

#[test]
fn missing_key_header_reports_login_help() {
    let scenario = Scenario {
        auth: Some(Auth::Header),
        ..Scenario::default()
    };
    check("a missing key header is reported with login help", scenario);
}

#[test]
fn missing_key_reports_login_help() {
    let scenario = Scenario {
        auth: Some(Auth::None),
        ..Scenario::default()
    };
    check("a missing key is reported with login help", scenario);
}

#[test]
fn expired_subscription_asks_to_log_in() {
    let scenario = Scenario {
        auth: Some(Auth::Subscription),
        ..Scenario::default()
    };
    check("an expired subscription asks to log in again", scenario);
}

#[test]
fn failing_key_command_is_reported_verbatim() {
    let scenario = Scenario {
        auth: Some(Auth::Command),
        ..Scenario::default()
    };
    check("a failing key command is reported verbatim", scenario);
}

#[test]
fn file_sessions_navigate_like_memory_sessions() {
    let scenario = Scenario {
        file: true,
        responses: generated(),
        ..Scenario::default()
    };
    check("the default summarizer records file details", scenario);
}

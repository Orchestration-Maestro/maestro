//! The session's extension runtime: tool hooks installed at construction, the definition
//! registry and its allowlist, partial bindings, the start event, discovered resources and
//! their sources, every bound capability, owned and borrowed UI hosts, provider refreshes
//! and the reload, compared with the recorded binding and registry cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Scripted, answer, assistant, echo_tools, harness, load_history,
    on_task_set, script_responses, sdk_definition, tool_definition, user,
};
use maestro_app::agent_session::{ExtensionBindings, PromptOptions};
use maestro_extensions::types::{ExtensionFlagOptions, FlagValue, RegisteredCommandOptions};
use maestro_extensions::{
    ExtensionAPI, ExtensionFactory, ResourcesDiscoverEvent, ResourcesDiscoverResult,
    SessionShutdownEvent, SessionStartEvent, ToolCallEvent, ToolCallEventResult,
};
use maestro_models::Local;
use maestro_session::Presence;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};

/// The recorded SDK case `name`.
fn expected(name: &str) -> Value {
    crate::support::session_harness::corpus("sdk")[name]["ok"].clone()
}

/// What the alpha extension heard, in order.
type Seen = Rc<RefCell<Vec<Value>>>;

/// The recorded alpha extension: the `alpha-cmd` command, the `alpha-tool` tool, and
/// handlers recording the start, shutdown and discovery events, discovery contributing
/// `skill`.
fn alpha(seen: &Seen, skill: String) -> ExtensionFactory<Local> {
    let seen = Rc::clone(seen);
    Rc::new(move |api| {
        let registered = register_alpha(&api, &seen, skill.clone());
        Box::pin(std::future::ready(registered))
    })
}

/// Registers the alpha extension's command, tool and handlers on `api`.
fn register_alpha(
    api: &ExtensionAPI<Local>,
    seen: &Seen,
    skill: String,
) -> Result<(), maestro_extensions::EventError> {
    api.register_command(
        "alpha-cmd".into(),
        RegisteredCommandOptions {
            description: Some("Alpha".into()),
            get_argument_completions: None,
            handler: Rc::new(|_, _| Box::pin(std::future::ready(Ok(())))),
        },
    )?;
    let mut tool = tool_definition(
        "alpha-tool",
        Some("  Alpha   snippet  "),
        Some(&["  Line one  ", "Line one", ""]),
    );
    tool.label = "Alpha".into();
    tool.definition.description = "Alpha tool".into();
    api.register_tool(tool)?;
    let starts = Rc::clone(seen);
    api.on::<SessionStartEvent, ()>(
        "session_start".into(),
        Rc::new(move |event, _| {
            starts
                .borrow_mut()
                .push(json!({"event": "session_start", "reason": event.reason}));
            Box::pin(std::future::ready(Ok(None)))
        }),
    )?;
    let shutdowns = Rc::clone(seen);
    api.on::<SessionShutdownEvent, ()>(
        "session_shutdown".into(),
        Rc::new(move |event, _| {
            shutdowns
                .borrow_mut()
                .push(json!({"event": "session_shutdown", "reason": event.reason}));
            Box::pin(std::future::ready(Ok(None)))
        }),
    )?;
    let discoveries = Rc::clone(seen);
    api.on::<ResourcesDiscoverEvent, ResourcesDiscoverResult>(
        "resources_discover".into(),
        Rc::new(move |event, _| {
            discoveries
                .borrow_mut()
                .push(json!({"event": "resources_discover", "reason": event.reason}));
            let result = ResourcesDiscoverResult {
                skill_paths: Presence::Present(vec![skill.clone()]),
                prompt_paths: Presence::Missing,
                theme_paths: Presence::Missing,
            };
            Box::pin(std::future::ready(Ok(Some(result))))
        }),
    )
}

/// A session with the alpha extension at `<alpha>` and the allowlist `allowed`.
async fn alpha_session(seen: &Seen, allowed: Option<Vec<String>>) -> Harness {
    let skill = String::from("alpha-skill.md");
    harness(HarnessOptions {
        labelled_factories: vec![("<alpha>", alpha(seen, skill))],
        allowed_tool_names: allowed,
        ..HarnessOptions::default()
    })
    .await
}

#[test]
fn subscribes_before_installing_tool_hooks() {
    on_task_set(|_| async {
        let executed = Rc::new(RefCell::new(false));
        let blocking: ExtensionFactory<Local> = Rc::new(|api| {
            let registered = api.on::<ToolCallEvent, ToolCallEventResult>(
                "tool_call".into(),
                Rc::new(|_, _| {
                    Box::pin(std::future::ready(Ok(Some(ToolCallEventResult {
                        block: Presence::Present(true),
                        reason: Presence::Present("blocked by policy".into()),
                    }))))
                }),
            );
            Box::pin(std::future::ready(registered))
        });
        let call = Scripted {
            tool_calls: vec![("echo".into(), json!({}))],
            ..Scripted::default()
        };
        let h = harness(HarnessOptions {
            factories: vec![blocking],
            responses: Some(vec![call, Scripted::text("done")]),
            base_tools_override: Some(echo_tools(Rc::clone(&executed))),
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .prompt("use it", PromptOptions::default())
            .await
            .unwrap();
        assert!(!*executed.borrow(), "the hook blocked the call");
        let history = h.session.messages();
        let result = history
            .read()
            .unwrap()
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .find(|record| record["role"] == "toolResult")
            .unwrap();
        assert_eq!(result["isError"], json!(true));
        assert_eq!(result["content"][0]["text"], json!("blocked by policy"));
    });
}

/// Each registered tool's name, source and path.
fn registry_rows(h: &Harness) -> Vec<Value> {
    h.session
        .get_all_tools()
        .into_iter()
        .map(|tool| json!({"name": tool.name, "source": tool.source_info.source, "path": tool.source_info.path}))
        .collect()
}

#[test]
fn builds_tool_definitions_with_extension_and_sdk_overrides() {
    let expected = expected("tool_registry_without_allowlist");
    on_task_set(|_| async {
        let seen = Seen::default();
        let h = harness(HarnessOptions {
            labelled_factories: vec![("<alpha>", alpha(&seen, "alpha-skill.md".into()))],
            custom_tools: vec![sdk_definition(tool_definition("sdk-tool", None, None))],
            ..HarnessOptions::default()
        })
        .await;
        let mut rows = expected["tools"].as_array().unwrap().clone();
        rows.push(json!({"name": "sdk-tool", "source": "sdk", "path": "<sdk:sdk-tool>"}));
        assert_eq!(json!(registry_rows(&h)), json!(rows));
        let mut active = expected["active"].as_array().unwrap().clone();
        active.push(json!("sdk-tool"));
        assert_eq!(json!(h.session.get_active_tool_names()), json!(active));
        let commands: Vec<Value> = h
            .runner()
            .get_registered_commands()
            .into_iter()
            .map(|command| {
                let source = command.command.source_info.read().unwrap().source.clone();
                json!({"invocationName": command.invocation_name, "source": source})
            })
            .collect();
        assert_eq!(json!(commands), expected["commands"]);
        assert_eq!(
            json!(h.session.has_extension_handlers("resources_discover")),
            expected["has_resources_discover"]
        );
    });
}

#[test]
fn keeps_declared_tool_allowlist_in_active_names() {
    let expected = expected("bindings_and_reload");
    on_task_set(|_| async {
        let seen = Seen::default();
        let allowed = vec!["read".to_owned(), "alpha-tool".to_owned()];
        let h = alpha_session(&seen, Some(allowed)).await;
        assert_eq!(
            json!(h.session.get_active_tool_names()),
            expected["before"]["active"]
        );
        let names: Vec<String> = h
            .session
            .get_all_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert_eq!(names, ["read", "alpha-tool"]);
    });
}

#[test]
fn emits_session_start_only_after_bindings() {
    let expected = expected("bindings_and_reload");
    on_task_set(|_| async {
        let seen = Seen::default();
        let allowed = vec!["read".to_owned(), "alpha-tool".to_owned()];
        let h = alpha_session(&seen, Some(allowed)).await;
        assert_eq!(json!(*seen.borrow()), expected["before"]["seen"]);
        h.session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
        assert_eq!(json!(*seen.borrow()), expected["after_bind"]["seen"]);
        assert_eq!(
            json!(h.session.get_active_tool_names()),
            expected["after_bind"]["active"]
        );
    });
}

#[test]
fn extends_resources_from_extension_discovery() {
    let expected = expected("bindings_and_reload");
    on_task_set(|_| async {
        let seen = Seen::default();
        let allowed = vec!["read".to_owned(), "alpha-tool".to_owned()];
        let h = alpha_session(&seen, Some(allowed)).await;
        h.session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
        assert_eq!(
            json!(h.extended.borrow().len()),
            expected["after_bind"]["extended"]
        );
        assert_eq!(
            json!(h.session.system_prompt().contains("Alpha snippet")),
            expected["after_bind"]["system_prompt_has_alpha"]
        );
        let paths = &h.extended.borrow()[0];
        assert_eq!(paths.skill_paths[0].path, "alpha-skill.md");
    });
}

#[test]
fn labels_extension_resources_by_source() {
    on_task_set(|_| async {
        let seen = Seen::default();
        let h = harness(HarnessOptions {
            labelled_factories: vec![
                ("<alpha>", alpha(&seen, "inline.md".into())),
                ("/opt/extensions/beta.ts", alpha(&seen, "file.md".into())),
            ],
            ..HarnessOptions::default()
        })
        .await;
        h.session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
        let extended = h.extended.borrow();
        let labels: Vec<Value> = extended[0]
            .skill_paths
            .iter()
            .map(|path| json!({"path": path.path, "source": path.metadata.source, "baseDir": path.metadata.base_dir}))
            .collect();
        assert_eq!(
            json!(labels),
            json!([
                {"path": "inline.md", "source": "extension:alpha", "baseDir": null},
                {"path": "file.md", "source": "extension:beta", "baseDir": "/opt/extensions"},
            ])
        );
        let metadata = &extended[0].skill_paths[0].metadata;
        assert_eq!(
            serde_json::to_value(metadata.scope).unwrap(),
            json!("temporary")
        );
        assert_eq!(
            serde_json::to_value(metadata.origin).unwrap(),
            json!("top-level")
        );
    });
}

#[test]
fn reload_emits_shutdown_then_start_only_with_bindings() {
    let bound = expected("bindings_and_reload");
    let unbound = expected("reload_without_bindings");
    on_task_set(|_| async {
        let seen = Seen::default();
        let h = alpha_session(&seen, Some(vec!["read".into()])).await;
        assert_eq!(json!(*seen.borrow()), unbound["before"]);
        h.session.reload().await.unwrap();
        assert_eq!(json!(*seen.borrow()), unbound["after"]);

        let seen = Seen::default();
        let allowed = vec!["read".to_owned(), "alpha-tool".to_owned()];
        let h = alpha_session(&seen, Some(allowed)).await;
        h.session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
        h.session.reload().await.unwrap();
        assert_eq!(json!(*seen.borrow()), bound["after_reload"]["seen"]);
        assert_eq!(
            json!(h.session.get_active_tool_names()),
            bound["after_reload"]["active"]
        );
        assert_eq!(
            json!(h.extended.borrow().len()),
            bound["after_reload"]["extended"]
        );
    });
}

/// An extension that keeps its registration facade in `slot` for later calls and has a
/// `fail` command that always fails.
fn keeping(slot: Rc<RefCell<Option<ExtensionAPI<Local>>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let registered = api.register_command(
            "fail".into(),
            RegisteredCommandOptions {
                description: Some("Fails".into()),
                get_argument_completions: None,
                handler: Rc::new(|_, _| Box::pin(std::future::ready(Err("command failed".into())))),
            },
        );
        *slot.borrow_mut() = Some(api);
        Box::pin(std::future::ready(registered))
    })
}

/// A session with the keeping extension, and its facade.
async fn kept_session() -> (Harness, ExtensionAPI<Local>) {
    let slot = Rc::new(RefCell::new(None));
    let h = harness(HarnessOptions {
        factories: vec![keeping(Rc::clone(&slot))],
        ..HarnessOptions::default()
    })
    .await;
    let api = slot.borrow_mut().take().unwrap();
    (h, api)
}

#[test]
fn binds_partial_ui_bindings_without_clearing_others() {
    on_task_set(|_| async {
        let (h, _api) = kept_session().await;
        let heard = Rc::new(RefCell::new(Vec::new()));
        let listener = Rc::clone(&heard);
        let errors = ExtensionBindings {
            on_error: Some(Rc::new(move |error| {
                listener.borrow_mut().push(error.error.clone());
                Ok(())
            })),
            ..ExtensionBindings::default()
        };
        h.session.bind_extensions(errors).await.unwrap();
        let commands = ExtensionBindings {
            command_context_actions: Some(maestro_extensions::ExtensionCommandContextActions::new(
                crate::command_operations::silent(),
            )),
            ..ExtensionBindings::default()
        };
        h.session.bind_extensions(commands).await.unwrap();
        h.session
            .prompt("/fail", PromptOptions::default())
            .await
            .unwrap();
        assert_eq!(*heard.borrow(), ["command failed"]);
        h.session.reload().await.unwrap();
        h.session
            .prompt("/fail", PromptOptions::default())
            .await
            .unwrap();
        assert_eq!(
            *heard.borrow(),
            ["command failed", "command failed"],
            "the reload kept the listener"
        );
    });
}

#[test]
fn binds_every_extension_capability() {
    on_task_set(|_| async {
        let (h, api) = kept_session().await;
        let session = &h.session;
        assert_eq!(
            api.get_active_tools().unwrap(),
            session.get_active_tool_names()
        );
        api.set_active_tools(vec!["read".into(), "missing".into()])
            .unwrap();
        assert_eq!(session.get_active_tool_names(), ["read"]);
        assert_eq!(
            api.get_all_tools().unwrap().len(),
            session.get_all_tools().len()
        );
        let commands: Vec<String> = api
            .get_commands()
            .unwrap()
            .into_iter()
            .map(|command| command.name)
            .collect();
        assert_eq!(commands, ["fail"]);
        api.set_session_name("Named".into()).unwrap();
        assert_eq!(session.session_name().unwrap().as_deref(), Some("Named"));
        assert_eq!(api.get_session_name().unwrap().as_deref(), Some("Named"));
        api.append_entry("note".into(), Some(json!({"kept": true})))
            .unwrap();
        let entry_id = session
            .session_manager()
            .borrow()
            .get_leaf_id()
            .unwrap()
            .to_owned();
        api.set_label(entry_id.clone(), Some("tag".into())).unwrap();
        assert_eq!(
            session.session_manager().borrow().get_label(&entry_id),
            Some(&json!("tag"))
        );
        api.set_thinking_level(maestro_models::ModelThinkingLevel::High)
            .unwrap();
        assert_eq!(api.get_thinking_level().unwrap(), session.thinking_level());
        assert!(api.set_model(h.model("think")).unwrap().await.unwrap());
        assert!(
            !api.set_model(h.elsewhere("think")).unwrap().await.unwrap(),
            "no credentials"
        );
        let context = h.runner().create_context();
        assert!(context.is_idle().unwrap());
        assert!(!context.has_pending_messages().unwrap());
        assert_eq!(
            context.get_system_prompt().unwrap(),
            session.system_prompt()
        );
        assert_eq!(
            context.get_context_usage().unwrap(),
            session.get_context_usage().unwrap()
        );
        let model = context.model().unwrap().unwrap();
        assert!(std::sync::Arc::ptr_eq(&model, &session.model()));
    });
}

#[test]
fn keeps_captured_contexts_usable_across_a_direct_reload() {
    on_task_set(|_| async {
        let (h, api) = kept_session().await;
        let context = h.runner().create_context();
        h.session.reload().await.unwrap();
        assert_eq!(context.cwd().unwrap(), h.cwd);
        assert_eq!(
            api.get_active_tools().unwrap(),
            h.session.get_active_tool_names()
        );
    });
}

/// A UI host whose editor text tells it apart from the headless fallback.
#[derive(Default)]
struct Editor(RefCell<String>);
impl maestro_extensions::types::ExtensionUIContextPort for Editor {
    fn set_editor_text(&self, text: String) -> Result<(), maestro_extensions::EventError> {
        *self.0.borrow_mut() = text;
        Ok(())
    }
    fn get_editor_text(&self) -> Result<String, maestro_extensions::EventError> {
        Ok(self.0.borrow().clone())
    }
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::default()
    }
}

#[test]
fn session_ui_bindings_keep_owned_hosts_and_reapply_borrowed_hosts() {
    on_task_set(|_| async {
        let (h, _api) = kept_session().await;
        let owned = Rc::new(Editor::default());
        let released = Rc::downgrade(&owned);
        let ui_context = Some(maestro_app::ExtensionUIContext::new(owned).into());
        let sdk = ExtensionBindings {
            ui_context,
            ..ExtensionBindings::default()
        };
        h.session.bind_extensions(sdk).await.unwrap();
        let context = h.runner().create_context();
        context
            .ui()
            .unwrap()
            .set_editor_text("owned".into())
            .unwrap();
        let host = released.upgrade().expect("the session owns an SDK host");
        assert_eq!(*host.0.borrow(), "owned");
        drop(host);
        let borrowed = Rc::new(Editor::default());
        let installed = maestro_app::ExtensionUIContext::new(borrowed.clone());
        let native = ExtensionBindings {
            ui_context: Some(maestro_app::ExtensionUIContextBinding::borrowed(&installed)),
            ..ExtensionBindings::default()
        };
        drop(installed);
        h.session.bind_extensions(native).await.unwrap();
        assert!(
            released.upgrade().is_none(),
            "rebinding released the owned host"
        );
        h.session
            .bind_extensions(ExtensionBindings::default())
            .await
            .unwrap();
        h.session.reload().await.unwrap();
        let reloaded = h.runner().create_context();
        assert!(reloaded.has_ui().unwrap());
        reloaded
            .ui()
            .unwrap()
            .set_editor_text("borrowed".into())
            .unwrap();
        assert_eq!(*borrowed.0.borrow(), "borrowed");
        assert_eq!(context.ui().unwrap().get_editor_text().unwrap(), "borrowed");
        drop(borrowed);
        assert!(!reloaded.has_ui().unwrap());
        assert!(!h.runner().has_ui());
        assert_eq!(reloaded.ui().unwrap().get_editor_text().unwrap(), "");
    });
}

/// Stores two turns and queues both possible summary requests.
fn prepare_compaction_history(h: &Harness) {
    for record in [
        user(&json!("first"), 1),
        assistant("answer", 2, &json!({})),
        user(&json!("second"), 3),
        assistant("last", 4, &json!({})),
    ] {
        h.session
            .session_manager()
            .borrow_mut()
            .append_message(record)
            .unwrap();
    }
    load_history(h);
    script_responses(
        h,
        vec![answer("summary", None, None), answer("summary", None, None)],
    );
}

#[test]
fn compaction_completion_failure_reaches_the_error_callback_unchanged() {
    on_task_set(|_| async {
        let tasks = Rc::new(RefCell::new(Vec::new()));
        let spawned = Rc::clone(&tasks);
        let h = harness(HarnessOptions {
            settings: json!({"compaction": {"keepRecentTokens": 1}}),
            spawn: Some(Rc::new(move |task| {
                spawned.borrow_mut().push(tokio::task::spawn_local(task));
            })),
            ..HarnessOptions::default()
        })
        .await;
        prepare_compaction_history(&h);
        let failure = RefCell::new(Some(
            Box::new(std::io::Error::from_raw_os_error(13)) as maestro_extensions::EventError
        ));
        let identity = std::ptr::from_ref(failure.borrow().as_ref().unwrap().as_ref()).cast::<()>();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let completed = Rc::clone(&trace);
        let failed = Rc::clone(&trace);
        h.runner()
            .create_context()
            .compact(Some(maestro_app::CompactOptions {
                on_complete: Some(Rc::new(move |result| {
                    assert!(result.summary.contains("summary"));
                    completed.borrow_mut().push("complete");
                    Err(failure.borrow_mut().take().unwrap())
                })),
                on_error: Some(Rc::new(move |error| {
                    assert_eq!(
                        *failed.borrow(),
                        ["complete"],
                        "unexpected compaction failure: {error}"
                    );
                    assert_eq!(std::ptr::from_ref(error.as_ref()).cast::<()>(), identity);
                    assert_eq!(
                        error
                            .downcast_ref::<std::io::Error>()
                            .unwrap()
                            .raw_os_error(),
                        Some(13)
                    );
                    failed.borrow_mut().push("error");
                    Ok(())
                })),
                ..maestro_app::CompactOptions::default()
            }))
            .unwrap();
        loop {
            let pending = std::mem::take(&mut *tasks.borrow_mut());
            if pending.is_empty() {
                break;
            }
            for task in pending {
                task.await.unwrap();
            }
        }
        assert_eq!(*trace.borrow(), ["complete", "error"]);
    });
}

#[test]
fn routes_shutdown_requests_to_the_bound_handler() {
    on_task_set(|_| async {
        let (h, _api) = kept_session().await;
        let requested = Rc::new(RefCell::new(0));
        let count = Rc::clone(&requested);
        let bindings = ExtensionBindings {
            shutdown_handler: Some(Rc::new(move || {
                *count.borrow_mut() += 1;
                Ok(())
            })),
            ..ExtensionBindings::default()
        };
        h.session.bind_extensions(bindings).await.unwrap();
        h.runner().create_context().shutdown().unwrap();
        assert_eq!(*requested.borrow(), 1);
    });
}

#[test]
fn refreshes_the_current_model_after_provider_registration() {
    on_task_set(|_| async {
        let (h, api) = kept_session().await;
        let before = h.session.model();
        let (provider, id) = {
            let model = before.read().unwrap();
            (model.provider.clone(), model.id.clone())
        };
        let model = h.faux.models[0].clone();
        let config = maestro_catalog::ProviderConfigInput {
            base_url: Some(model.base_url.clone()),
            api_key: Some("faux-key".into()),
            api: Some(h.faux.api.clone()),
            models: Some(vec![maestro_catalog::ProviderModelInput {
                id: model.id.clone(),
                name: "Renamed".into(),
                api: Some(model.api.clone()),
                base_url: Some(model.base_url.clone()),
                reasoning: model.reasoning,
                thinking_level_map: None,
                input: model.input.clone(),
                cost: model.cost.clone(),
                context_window: model.context_window,
                max_tokens: model.max_tokens,
                headers: None,
                compat: None,
            }]),
            ..maestro_catalog::ProviderConfigInput::default()
        };
        api.register_provider(provider.clone(), std::sync::Arc::new(RwLock::new(config)))
            .unwrap();
        let after = h.session.model();
        assert!(
            !std::sync::Arc::ptr_eq(&before, &after),
            "the selected model was refreshed"
        );
        assert!(std::sync::Arc::ptr_eq(
            &after,
            &h.registry.find(&provider, &id).unwrap()
        ));
        assert_eq!(after.read().unwrap().name, "Renamed");
        api.unregister_provider(&provider).unwrap();
        assert!(h.registry.find(&provider, &id).is_none());
        assert!(
            std::sync::Arc::ptr_eq(&h.session.model(), &after),
            "nothing replaces a gone model"
        );
    });
}

#[test]
fn reload_keeps_extension_flag_values() {
    on_task_set(|local| async move {
        let h = harness(HarnessOptions {
            models: vec![("faux-1", true)],
            ..HarnessOptions::default()
        })
        .await;
        let flagged: ExtensionFactory<Local> = Rc::new(|api| {
            let options = ExtensionFlagOptions {
                description: None,
                r#type: maestro_extensions::types::FlagType::Boolean,
                default: Some(FlagValue::Boolean(false)),
            };
            Box::pin(std::future::ready(
                api.register_flag("verbose".into(), options),
            ))
        });
        let module_loader: maestro_extensions::ModuleLoader<Local> =
            Rc::new(|_| Box::pin(std::future::ready(Ok(None))));
        let mut resources = maestro_app::resources::resource_loader::ResourceLoaderOptions::new(
            local,
            module_loader,
        );
        resources.extension_factories = vec![flagged];
        let settings =
            maestro_settings::SettingsManager::in_memory(maestro_settings::Settings::default());
        let created =
            maestro_app::sdk::create_agent_session(maestro_app::sdk::CreateAgentSessionOptions {
                cwd: Some(h.cwd.clone()),
                agent_dir: Some(h.scratch.path().display().to_string()),
                auth_storage: None,
                model_registry: Some(std::sync::Arc::clone(&h.registry)),
                model: Some(h.model("faux-1")),
                thinking_level: None,
                scoped_models: Vec::new(),
                no_tools: None,
                tools: None,
                custom_tools: Vec::new(),
                resources: maestro_app::sdk::SessionResources::Default(Box::new(resources)),
                session_manager: None,
                settings_manager: Some(Rc::new(RefCell::new(settings))),
                session_start_event: None,
                theme: maestro_theme::LiveTheme::default(),
                offline: None,
            })
            .await
            .unwrap();
        let session = created.session;
        session
            .extension_runner()
            .set_flag_value("verbose".into(), FlagValue::Boolean(true));
        session.reload().await.unwrap();
        let values = session.extension_runner().get_flag_values();
        assert_eq!(values.get("verbose"), Some(&FlagValue::Boolean(true)));
    });
}

/// What a live tool description could change while it resolved: its own tool, and the
/// session settings once a session is supplied.
type Changes = Rc<RefCell<Vec<(bool, Option<bool>)>>>;

/// Where a test puts the session a callback reaches once it exists.
type SessionSlot = Rc<RefCell<std::rc::Weak<maestro_app::AgentSession<Local>>>>;

/// The echo tool, its description recording what it could change while resolving, and the
/// slot the session goes into.
fn self_updating_tools(
    changes: &Changes,
) -> (
    indexmap::IndexMap<String, maestro_agent::SharedAgentTool<Local>>,
    SessionSlot,
) {
    let tools = echo_tools(Rc::default());
    let tool = Rc::downgrade(&tools["echo"]);
    let session = SessionSlot::default();
    let (changes, live) = (Rc::clone(changes), Rc::clone(&session));
    tools["echo"].write().unwrap().description = Some(Rc::new(move || {
        let tool_writable = tool.upgrade().unwrap().try_write().is_ok();
        let settings_writable = live
            .borrow()
            .upgrade()
            .map(|session| session.settings_manager().try_borrow_mut().is_ok());
        changes
            .borrow_mut()
            .push((tool_writable, settings_writable));
        Ok("Echo back".into())
    }));
    (tools, session)
}

#[test]
fn resolves_tool_descriptions_with_no_guard_held() {
    on_task_set(|_| async {
        let changes = Changes::default();
        let (tools, slot) = self_updating_tools(&changes);
        let h = harness(HarnessOptions {
            base_tools_override: Some(tools),
            ..HarnessOptions::default()
        })
        .await;
        assert_eq!(*changes.borrow(), [(true, None)]);
        *slot.borrow_mut() = Rc::downgrade(&h.session);
        h.session.reload().await.unwrap();
        assert_eq!(*changes.borrow(), [(true, None), (true, Some(true))]);
    });
}

#[test]
fn extension_session_name_reaches_the_retained_session_and_listener() {
    on_task_set(|_| async {
        let (h, api) = kept_session().await;
        api.set_session_name("from extension".into()).unwrap();
        assert_eq!(
            h.session.session_name().unwrap().as_deref(),
            Some("from extension")
        );
        let names: Vec<Value> = h
            .events
            .borrow()
            .iter()
            .filter(|event| event["type"] == "session_info_changed")
            .map(|event| event["name"].clone())
            .collect();
        assert_eq!(names, [json!("from extension")]);
    });
}

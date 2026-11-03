//! SDK registration phases, tool metadata and caller-selected tool policies.
use super::super::*;
use maestro_app::{
    CreateAgentSessionOptions, ExtensionBindings, NoTools, PromptOptions, SessionResources,
    create_agent_session,
};
use maestro_extensions::{ExtensionFactory, SessionStartEvent};
use std::sync::{Arc, Mutex, RwLock};

/// Registration point relative to session construction.
#[derive(Clone, Copy)]
enum Phase {
    /// The factory's initial registration.
    Load,
    /// The bound session's start event.
    Start,
    /// A slash command handled without reloading.
    Command,
}

/// Provider descriptors the scripted protocol actually receives.
struct Capture(Arc<Mutex<Option<String>>>);

/// Install both callback families required by the SDK's scripted provider.
macro_rules! capture_family {
    ($family:ty) => {
        impl maestro_models::Invoke<$family, maestro_models::FauxResponseInput<$family>>
            for Capture
        {
            type Return = <$family as maestro_models::Execution>::Future<
                'static,
                Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
            >;
            /// Observe the real request before returning its controlled answer.
            fn invoke(&self, input: maestro_models::FauxResponseInput<$family>) -> Self::Return {
                *self.0.lock().unwrap() = Some(input.model.base_url);
                Box::pin(std::future::ready(Ok(
                    maestro_models::faux_assistant_message(
                        "ok",
                        maestro_models::FauxAssistantMessageOptions::default(),
                    ),
                )))
            }
        }
    };
}
capture_family!(Local);
capture_family!(maestro_models::Threaded);

/// Apply a descriptor override through the real retained registration API.
fn override_provider(api: &ExtensionAPI<Local>, url: &str) -> Result<(), EventError> {
    api.register_provider(
        "faux".into(),
        Arc::new(RwLock::new(maestro_app::ProviderConfig {
            base_url: Some(url.into()),
            ..maestro_app::ProviderConfig::default()
        })),
    )
}

/// Install one registration at its caller-selected lifecycle boundary.
fn provider_factory(phase: Phase, url: &'static str) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let retained = api.clone();
        let registered = match phase {
            Phase::Load => override_provider(&api, url),
            Phase::Start => api.on::<SessionStartEvent, ()>(
                "session_start".into(),
                Rc::new(move |_, _| {
                    Box::pin(std::future::ready(
                        override_provider(&retained, url).map(|()| None),
                    ))
                }),
            ),
            Phase::Command => api.register_command(
                "use-proxy".into(),
                maestro_extensions::types::RegisteredCommandOptions {
                    description: None,
                    get_argument_completions: None,
                    handler: Rc::new(move |_, _| {
                        Box::pin(std::future::ready(override_provider(&retained, url)))
                    }),
                },
            ),
        };
        Box::pin(std::future::ready(registered))
    })
}

/// Keep the real default loader while adding a caller's native factory.
fn options(
    sample: &Sample,
    factory: ExtensionFactory<Local>,
) -> Result<CreateAgentSessionOptions, Error> {
    let mut options = sample.options()?;
    if let SessionResources::Default(resources) = &mut options.resources {
        resources.extension_factories.push(factory);
    }
    Ok(options)
}

/// Observe the active descriptor and the subsequent protocol invocation independently.
fn provider_case(phase: Phase, url: &'static str) -> Result<Value, Error> {
    support::execute(|sample| async move {
        let created = create_agent_session(options(&sample, provider_factory(phase, url))?).await?;
        created
            .session
            .bind_extensions(ExtensionBindings::default())
            .await?;
        if matches!(phase, Phase::Command) {
            created
                .session
                .prompt("/use-proxy", PromptOptions::default())
                .await?;
        }
        let selected = created.session.model().read().unwrap().base_url.clone();
        let observed = Arc::new(Mutex::new(None));
        sample
            .provider
            .set_responses(vec![maestro_models::FauxResponseStep::Factory(Arc::new(
                Capture(Arc::clone(&observed)),
            ))]);
        created
            .session
            .prompt("hello", PromptOptions::default())
            .await?;
        let result = json!({"selected": selected, "requested": *observed.lock().unwrap()});
        created.session.dispose();
        Ok(result)
    })
}

/// Initial registration replaces the active model before its first request.
#[test]
fn provider_overrides_apply_during_factory_loading() -> Result<(), Error> {
    let value = provider_case(Phase::Load, "https://example.invalid/load")?;
    assert_eq!(value["selected"], "https://example.invalid/load");
    assert_eq!(value["requested"], "https://example.invalid/load");
    Ok(())
}

/// Start handlers replace the selected descriptor before prompting.
#[test]
fn provider_overrides_apply_at_session_start() -> Result<(), Error> {
    let value = provider_case(Phase::Start, "https://example.invalid/start")?;
    assert_eq!(value["selected"], "https://example.invalid/start");
    assert_eq!(value["requested"], "https://example.invalid/start");
    Ok(())
}

/// Command-time registrations take effect without a resource reload.
#[test]
fn provider_overrides_apply_after_a_command_without_reload() -> Result<(), Error> {
    let value = provider_case(Phase::Command, "https://example.invalid/command")?;
    assert_eq!(value["selected"], "https://example.invalid/command");
    assert_eq!(value["requested"], "https://example.invalid/command");
    Ok(())
}

/// Register a real tool only once the session emits its start event.
fn tool_factory(snippet: bool) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let retained = api.clone();
        let registered = api.on::<SessionStartEvent, ()>(
            "session_start".into(),
            Rc::new(move |_, _| {
                let mut definition = tool();
                if snippet {
                    definition.prompt_guidelines =
                        Some(vec!["Prefer my_tool for text processing.".into()]);
                } else {
                    definition.prompt_snippet = None;
                }
                Box::pin(std::future::ready(
                    retained.register_tool(definition).map(|()| None),
                ))
            }),
        );
        Box::pin(std::future::ready(registered))
    })
}

/// Late registration refreshes executable tools, source metadata, snippets and guidelines.
#[test]
fn session_start_tools_refresh_the_live_registry_and_prompt() -> Result<(), Error> {
    support::execute(|sample| async move {
        let created = create_agent_session(options(&sample, tool_factory(true))?).await?;
        assert!(created.session.get_tool_definition("my_tool").is_none());
        created
            .session
            .bind_extensions(ExtensionBindings::default())
            .await?;
        assert!(
            created
                .session
                .get_active_tool_names()
                .contains(&"my_tool".into())
        );
        let tools = created.session.get_all_tools();
        let custom = tools.iter().find(|tool| tool.name == "my_tool").unwrap();
        assert_eq!(custom.source_info.path, "<inline:1>");
        assert_eq!(custom.source_info.source, "inline");
        assert_eq!(
            custom.source_info.scope,
            maestro_resources::SourceScope::Temporary
        );
        assert_eq!(
            custom.source_info.origin,
            maestro_resources::SourceOrigin::TopLevel
        );
        let read = tools.iter().find(|tool| tool.name == "read").unwrap();
        assert_eq!(read.source_info.path, "<builtin:read>");
        assert_eq!(read.source_info.source, "builtin");
        let prompt = created.session.system_prompt();
        assert!(prompt.contains("- my_tool: Process text with my_tool"));
        assert!(prompt.contains("- Prefer my_tool for text processing."));
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// SDK-owned definitions retain their SDK provenance rather than an extension's.
#[test]
fn sdk_custom_tools_keep_their_source_metadata() -> Result<(), Error> {
    use maestro_extensions::ExtensionExecution;
    support::execute(|sample| async move {
        let mut options = sample.options()?;
        options.custom_tools.push(Local::retain_definition(
            maestro_extensions::RegisteredToolInput::from(tool()),
        ));
        let created = create_agent_session(options).await?;
        let tools = created.session.get_all_tools();
        let custom = tools.iter().find(|tool| tool.name == "my_tool").unwrap();
        assert_eq!(custom.source_info.path, "<sdk:my_tool>");
        assert_eq!(custom.source_info.source, "sdk");
        assert_eq!(
            custom.source_info.scope,
            maestro_resources::SourceScope::Temporary
        );
        assert_eq!(
            custom.source_info.origin,
            maestro_resources::SourceOrigin::TopLevel
        );
        assert!(
            created
                .session
                .get_active_tool_names()
                .contains(&"my_tool".into())
        );
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// A missing prompt snippet hides metadata from the prompt, not the executable registry.
#[test]
fn tools_without_snippets_stay_active_but_out_of_the_prompt() -> Result<(), Error> {
    support::execute(|sample| async move {
        let created = create_agent_session(options(&sample, tool_factory(false))?).await?;
        created
            .session
            .bind_extensions(ExtensionBindings::default())
            .await?;
        assert!(created.session.get_tool_definition("my_tool").is_some());
        assert!(
            created
                .session
                .get_active_tool_names()
                .contains(&"my_tool".into())
        );
        let prompt = created.session.system_prompt();
        assert!(!prompt.contains("my_tool"));
        assert!(!prompt.contains("Process supplied text"));
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Create a session whose real late-registered tool is filtered by caller policy.
async fn selected_tools(
    sample: &Sample,
    no_tools: Option<NoTools>,
    allowed: Option<Vec<String>>,
) -> Result<Rc<maestro_app::AgentSession>, Error> {
    let mut options = options(sample, tool_factory(true))?;
    options.no_tools = no_tools;
    options.tools = allowed;
    let created = create_agent_session(options).await?;
    created
        .session
        .bind_extensions(ExtensionBindings::default())
        .await?;
    Ok(created.session)
}

/// An allowlist filters built-ins and extensions through the same registry.
#[test]
fn tool_allowlists_filter_builtins_and_extensions() -> Result<(), Error> {
    support::execute(|sample| async move {
        let session =
            selected_tools(&sample, None, Some(vec!["read".into(), "my_tool".into()])).await?;
        assert_eq!(session.get_active_tool_names(), ["read", "my_tool"]);
        let mut names: Vec<_> = session
            .get_all_tools()
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        names.sort();
        assert_eq!(names, ["my_tool", "read"]);
        let prompt = session.system_prompt();
        assert!(prompt.contains("- read:"));
        assert!(prompt.contains("- my_tool:"));
        assert!(!prompt.contains("- bash:"));
        assert!(!prompt.contains("- edit:"));
        session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// An explicit empty allowlist also excludes tools registered after initialization.
#[test]
fn empty_tool_allowlists_exclude_late_registrations() -> Result<(), Error> {
    support::execute(|sample| async move {
        let session = selected_tools(&sample, None, Some(Vec::new())).await?;
        assert!(session.get_active_tool_names().is_empty());
        assert!(session.get_all_tools().is_empty());
        assert!(session.system_prompt().contains("Available tools:\n(none)"));
        assert!(!session.system_prompt().contains("my_tool"));
        session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Disabling default built-ins does not disable extension registrations.
#[test]
fn disabled_builtin_defaults_keep_extension_tools_active() -> Result<(), Error> {
    support::execute(|sample| async move {
        let session = selected_tools(&sample, Some(NoTools::Builtin), None).await?;
        assert_eq!(session.get_active_tool_names(), ["my_tool"]);
        assert_eq!(session.get_all_tools().len(), 8);
        let prompt = session.system_prompt();
        assert!(prompt.contains("- my_tool:"));
        assert!(!prompt.contains("- read:"));
        assert!(!prompt.contains("- bash:"));
        session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Disabling every tool remains stronger than later extension registration.
#[test]
fn disabled_tools_exclude_late_extension_registrations() -> Result<(), Error> {
    support::execute(|sample| async move {
        let session = selected_tools(&sample, Some(NoTools::All), None).await?;
        assert!(session.get_active_tool_names().is_empty());
        assert!(session.get_all_tools().is_empty());
        assert!(session.system_prompt().contains("Available tools:\n(none)"));
        session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

/// Service-based construction forwards the caller's built-in selection unchanged.
#[test]
fn services_forward_disabled_builtin_defaults() -> Result<(), Error> {
    use maestro_app::{
        CreateAgentSessionFromServicesOptions, CreateAgentSessionServicesOptions,
        create_agent_session_from_services, create_agent_session_services,
    };
    support::execute(|sample| async move {
        let options = sample.options()?;
        let SessionResources::Default(resources) = options.resources else {
            return Err("missing default resources".into());
        };
        let cwd = sample.root.path().display().to_string();
        let services = Rc::new(
            create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: cwd.clone(),
                agent_dir: Some(cwd),
                auth_storage: options.auth_storage,
                settings_manager: options.settings_manager,
                model_registry: None,
                extension_flag_values: None,
                resource_loader_options: *resources,
                offline: Some(true),
            })
            .await?,
        );
        let created = create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
            services,
            session_manager: options.session_manager.unwrap(),
            session_start_event: None,
            model: options.model,
            thinking_level: None,
            scoped_models: Vec::new(),
            tools: None,
            no_tools: Some(NoTools::Builtin),
            custom_tools: Vec::new(),
            theme: maestro_theme::LiveTheme::default(),
        })
        .await?;
        assert!(created.session.get_active_tool_names().is_empty());
        assert!(
            created
                .session
                .system_prompt()
                .contains("Available tools:\n(none)")
        );
        assert!(!created.session.system_prompt().contains("- read:"));
        created.session.dispose();
        Ok(Value::Null)
    })?;
    Ok(())
}

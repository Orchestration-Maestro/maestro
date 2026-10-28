//! Replace the active session and rebind its local subscriptions after each change.
mod support;
use maestro_app::{
    AgentSession, AgentSessionEvent, CreateAgentSessionRuntimeFactory,
    CreateAgentSessionRuntimeOptions, CreateAgentSessionRuntimeResult, ExtensionBindings,
    SessionManager,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use support::{Error, Sample};

/// Run the runtime-replacement sample over disposable histories.
fn main() -> Result<(), Error> {
    support::launch(run)
}

/// Create, replace and resume the session while observing its live queues.
async fn run(sample: Sample) -> Result<Value, Error> {
    use maestro_app::{PromptOptions, create_agent_session_runtime};
    use maestro_extensions::types::{NewSessionCommandOptions, SwitchSessionOptions};
    use maestro_models::{FauxAssistantMessageOptions, FauxResponseStep, faux_assistant_message};
    let sample = Rc::new(sample);
    let retained = Rc::clone(&sample);
    let factory: CreateAgentSessionRuntimeFactory =
        Rc::new(move |options| Box::pin(create_runtime(Rc::clone(&retained), options)));
    let cwd = sample.root.path().display().to_string();
    let runtime = create_agent_session_runtime(
        factory,
        CreateAgentSessionRuntimeOptions {
            cwd: cwd.clone(),
            agent_dir: cwd.clone(),
            session_start_event: None,
            session_manager: Rc::new(RefCell::new(SessionManager::create(&cwd, None, &cwd)?)),
        },
    )
    .await?;
    sample
        .provider
        .set_responses(vec![FauxResponseStep::Message(Box::new(
            faux_assistant_message("Saved.", FauxAssistantMessageOptions::default()),
        ))]);
    runtime
        .session()
        .prompt("Remember this runtime.", PromptOptions::default())
        .await?;
    let first = runtime
        .session()
        .session_file()
        .ok_or("missing session file")?;
    let queues = Rc::new(RefCell::new(Vec::new()));
    let unsubscribe = bind(runtime.session(), Rc::clone(&queues)).await?;
    runtime.session().steer("first queue", &[])?;
    unsubscribe();
    runtime
        .new_session(NewSessionCommandOptions::default())
        .await?;
    let different = runtime.session().session_file().as_ref() != Some(&first);
    let unsubscribe = bind(runtime.session(), Rc::clone(&queues)).await?;
    runtime.session().follow_up("new queue", &[])?;
    unsubscribe();
    runtime
        .switch_session(&first, None, SwitchSessionOptions::default())
        .await?;
    let unsubscribe = bind(runtime.session(), Rc::clone(&queues)).await?;
    runtime.session().steer("resumed queue", &[])?;
    let result = json!({"newIsDifferent": different,
        "resumedOriginal": runtime.session().session_file().as_ref() == Some(&first),
        "queues": *queues.borrow(),
        "messages": runtime.session().messages().read().map_err(|error| error.to_string())?.len()});
    unsubscribe();
    runtime.dispose().await?;
    Ok(result)
}

/// Rebind extensions and subscribe only to the current session's queue updates.
async fn bind(
    session: Rc<AgentSession>,
    queues: Rc<RefCell<Vec<usize>>>,
) -> Result<Box<dyn Fn()>, Error> {
    session
        .bind_extensions(ExtensionBindings::default())
        .await?;
    Ok(Box::new(session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::QueueUpdate {
            steering,
            follow_up,
        } = event
        {
            queues.borrow_mut().push(steering.len() + follow_up.len());
        }
    }))))
}

/// Build real directory-bound services for each replacement, using the same controlled model.
async fn create_runtime(
    sample: Rc<Sample>,
    input: CreateAgentSessionRuntimeOptions,
) -> Result<CreateAgentSessionRuntimeResult, maestro_models::DiagnosticErrorInfo> {
    use maestro_app::{
        CreateAgentSessionFromServicesOptions, CreateAgentSessionServicesOptions, SessionResources,
        create_agent_session_from_services, create_agent_session_services,
    };
    let setup = sample
        .options()
        .map_err(|error| maestro_models::DiagnosticErrorInfo::new(None, error.to_string()))?;
    let SessionResources::Default(resources) = setup.resources else {
        return Err(maestro_models::DiagnosticErrorInfo::new(
            None,
            "The sample requires default resource discovery",
        ));
    };
    let services = Rc::new(
        create_agent_session_services(CreateAgentSessionServicesOptions {
            cwd: input.cwd,
            agent_dir: Some(input.agent_dir),
            auth_storage: setup.auth_storage,
            settings_manager: setup.settings_manager,
            model_registry: None,
            extension_flag_values: None,
            resource_loader_options: *resources,
            offline: Some(true),
        })
        .await
        .map_err(|error| maestro_models::DiagnosticErrorInfo::new(None, error.to_string()))?,
    );
    let created = create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
        services: Rc::clone(&services),
        session_manager: input.session_manager,
        session_start_event: input.session_start_event,
        model: setup.model,
        thinking_level: None,
        scoped_models: Vec::new(),
        tools: None,
        no_tools: None,
        custom_tools: Vec::new(),
        theme: maestro_theme::LiveTheme::default(),
    })
    .await?;
    Ok(CreateAgentSessionRuntimeResult {
        session: created.session,
        diagnostics: services.diagnostics.clone(),
        services,
        extensions_result: created.extensions_result,
        model_fallback_message: created.model_fallback_message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// New and resumed sessions get fresh subscriptions rather than retaining the old session.
    #[test]
    fn session_runtime_rebinds_after_new_and_resumed_sessions() -> Result<(), Error> {
        let value = support::execute(run)?;
        assert_eq!(value["newIsDifferent"], true);
        assert_eq!(value["resumedOriginal"], true);
        assert_eq!(value["queues"], json!([1, 1, 1]));
        assert_eq!(value["messages"], 2);
        Ok(())
    }
}

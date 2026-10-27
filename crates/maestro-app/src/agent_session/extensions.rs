//! The session operations a runner binds behind the extension API and every context.
use super::AgentSession;
use super::state::thrown;
use crate::utils::lock::read;
use maestro_extensions::types::{
    AppendEntryHandler, CompactOptions, ExtensionActions, ExtensionContextActions, ProviderActions,
    SendMessageHandler, SendUserMessageHandler, SetLabelHandler, SetModelHandler,
};
use maestro_extensions::{
    EventError, ExtensionError, ExtensionRunner, SlashCommandInfo, SlashCommandSource,
};
use maestro_models::{DiagnosticErrorInfo, Execution, Local};
use maestro_session::Presence;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The failure of an operation whose session was dropped.
const SESSION_GONE: &str = "The session is no longer available";

/// Runs `operation` on the session behind `weak`, or fails once it is gone.
fn with<T>(
    weak: &Weak<AgentSession<Local>>,
    operation: impl FnOnce(&Rc<AgentSession<Local>>) -> Result<T, DiagnosticErrorInfo>,
) -> Result<T, EventError> {
    let session = weak
        .upgrade()
        .ok_or_else(|| EventError::from(SESSION_GONE))?;
    operation(&session).map_err(Into::into)
}

/// Starts `work` on the session's task facility; a failure is reported to the extensions'
/// error listeners as a runtime failure of `event`.
fn detach(
    session: &Rc<AgentSession<Local>>,
    event: &'static str,
    work: impl Future<Output = Result<(), DiagnosticErrorInfo>> + 'static,
) {
    let runner = Rc::downgrade(&session.extension_runner());
    drop(session.start_operation(async move {
        if let (Err(error), Some(runner)) = (work.await, runner.upgrade()) {
            drop(runner.emit_error(&ExtensionError {
                extension_path: "<runtime>".into(),
                event: event.into(),
                error: error.message,
                stack: None,
            }));
        }
    }));
}

impl AgentSession<Local> {
    /// Binds the session's messaging, history, tool, command, model and thinking operations,
    /// its context reads and its provider routes behind `runner`.
    pub(super) fn bind_extension_core(
        self: &Rc<Self>,
        runner: &ExtensionRunner<Local>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let weak = Rc::downgrade(self);
        runner
            .bind_core(
                extension_actions(&weak),
                context_actions(&weak),
                provider_actions(&weak),
            )
            .map_err(thrown)
    }

    /// The extension commands, then the prompt templates, then the skills, as commands.
    pub(crate) fn get_commands(&self) -> Vec<SlashCommandInfo> {
        let commands = self.extension_runner().get_registered_commands();
        let commands = commands.into_iter().map(|command| SlashCommandInfo {
            name: command.invocation_name,
            description: command.command.description,
            source: SlashCommandSource(SlashCommandSource::EXTENSION.into()),
            source_info: read(&command.command.source_info).clone(),
        });
        let templates = self.prompt_templates();
        let templates = templates.prompts.iter().map(|template| SlashCommandInfo {
            name: template.name.clone(),
            description: Some(template.description.clone()),
            source: SlashCommandSource(SlashCommandSource::PROMPT.into()),
            source_info: template.source_info.clone(),
        });
        let skills = self.resource_loader.get_skills();
        let skills = skills.skills.iter().map(|skill| SlashCommandInfo {
            name: format!("skill:{}", skill.name),
            description: Some(skill.description.clone()),
            source: SlashCommandSource(SlashCommandSource::SKILL.into()),
            source_info: skill.source_info.clone(),
        });
        commands.chain(templates).chain(skills).collect()
    }

    /// Selects the catalog's current copy of the selected model, when it has another one.
    fn refresh_current_model_from_registry(&self) {
        let current = self.model();
        let (provider, id) = {
            let model = read(&current);
            (model.provider.clone(), model.id.clone())
        };
        if let Some(refreshed) = self.model_registry.find(&provider, &id)
            && !Arc::ptr_eq(&refreshed, &current)
        {
            crate::utils::lock::write(self.agent.state()).model = refreshed;
        }
    }
}

/// The messaging, history, tool, command, model and thinking operations of the extension API.
fn extension_actions(weak: &Weak<AgentSession<Local>>) -> ExtensionActions<Local> {
    let w = weak.clone();
    let set_session_name =
        Rc::new(move |name: String| with(&w, |session| session.set_session_name(&name)));
    let w = weak.clone();
    let get_session_name =
        Rc::new(move || with(&w, |session| session.session_name().map_err(thrown)));
    let w = weak.clone();
    let get_active_tools = Rc::new(move || with(&w, |session| Ok(session.get_active_tool_names())));
    let w = weak.clone();
    let get_all_tools = Rc::new(move || with(&w, |session| Ok(session.get_all_tools())));
    let w = weak.clone();
    let set_active_tools = Rc::new(move |names: Vec<String>| {
        with(&w, |session| session.set_active_tools_by_name(&names))
    });
    let w = weak.clone();
    let refresh_tools = Rc::new(move || {
        with(&w, |session| {
            session.refresh_tool_registry(super::tools::RefreshToolRegistryOptions::default())
        })
    });
    let w = weak.clone();
    let get_commands = Rc::new(move || with(&w, |session| Ok(session.get_commands())));
    let w = weak.clone();
    let get_thinking_level = Rc::new(move || with(&w, |session| Ok(session.thinking_level())));
    let w = weak.clone();
    let set_thinking_level =
        Rc::new(move |level| with(&w, |session| session.set_thinking_level(level)));
    ExtensionActions {
        send_message: send_message_action(weak),
        send_user_message: send_user_message_action(weak),
        append_entry: append_entry_action(weak),
        set_session_name,
        get_session_name,
        set_label: set_label_action(weak),
        get_active_tools,
        get_all_tools,
        set_active_tools,
        refresh_tools,
        get_commands,
        set_model: set_model_action(weak),
        get_thinking_level,
        set_thinking_level,
    }
}

/// Starts sending a custom message; a failure is reported as a `send_message` failure.
fn send_message_action(weak: &Weak<AgentSession<Local>>) -> SendMessageHandler<Local> {
    let w = weak.clone();
    Rc::new(move |message, options: Option<_>| {
        with(&w, |session| {
            let messages = Rc::clone(session);
            let work = async move {
                messages
                    .send_custom_message(message, options.unwrap_or_default())
                    .await
            };
            detach(session, "send_message", work);
            Ok(())
        })
    })
}

/// Starts sending a user message; a failure is reported as a `send_user_message` failure.
fn send_user_message_action(weak: &Weak<AgentSession<Local>>) -> SendUserMessageHandler<Local> {
    let w = weak.clone();
    Rc::new(move |content, options: Option<_>| {
        with(&w, |session| {
            let users = Rc::clone(session);
            let work = async move {
                users
                    .send_user_message(content, options.unwrap_or_default())
                    .await
            };
            detach(session, "send_user_message", work);
            Ok(())
        })
    })
}

/// Appends a custom entry with its data to the history.
fn append_entry_action(weak: &Weak<AgentSession<Local>>) -> AppendEntryHandler<Local> {
    let w = weak.clone();
    Rc::new(
        move |custom_type: String, data: Option<serde_json::Value>| {
            with(&w, |session| {
                let data = data.map_or(Presence::Missing, Presence::Present);
                let mut manager = session.session_manager.borrow_mut();
                manager
                    .append_custom_entry(&custom_type, data)
                    .map_err(thrown)?;
                Ok(())
            })
        },
    )
}

/// Sets or clears the label of a history entry.
fn set_label_action(weak: &Weak<AgentSession<Local>>) -> SetLabelHandler<Local> {
    let w = weak.clone();
    Rc::new(move |entry_id: String, label: Option<String>| {
        with(&w, |session| {
            let label = label
                .as_deref()
                .map_or(Presence::Missing, Presence::Present);
            let mut manager = session.session_manager.borrow_mut();
            manager
                .append_label_change(&entry_id, label)
                .map_err(thrown)?;
            Ok(())
        })
    })
}

/// Selects a model that has credentials; whether it was selected.
fn set_model_action(weak: &Weak<AgentSession<Local>>) -> SetModelHandler<Local> {
    let w = weak.clone();
    Rc::new(move |model| {
        let session = w.upgrade();
        Box::pin(async move {
            let session = session.ok_or_else(|| EventError::from(SESSION_GONE))?;
            if !session
                .model_registry
                .has_configured_auth(&model)
                .map_err(thrown)?
            {
                return Ok(false);
            }
            match session.set_model_checked(model).await {
                Ok(()) => Ok(true),
                Err(super::decisions::SelectionError::Refused(_)) => Ok(false),
                Err(super::decisions::SelectionError::Failed(error)) => Err(error.into()),
            }
        }) as <Local as Execution>::Future<'static, Result<bool, EventError>>
    })
}

/// The session reads and operations behind every context. Compaction and completion
/// callback failures reach the request's error callback when supplied.
fn context_actions(weak: &Weak<AgentSession<Local>>) -> ExtensionContextActions {
    let w = weak.clone();
    let get_model = Rc::new(move || with(&w, |session| Ok(Some(session.model()))));
    let w = weak.clone();
    let is_idle = Rc::new(move || with(&w, |session| Ok(!session.is_streaming())));
    let w = weak.clone();
    let get_signal = Rc::new(move || with(&w, |session| Ok(session.agent.signal())));
    let w = weak.clone();
    let abort = Rc::new(move || {
        with(&w, |session| {
            let aborting = Rc::clone(session);
            drop(session.start_operation(async move { aborting.abort().await }));
            Ok(())
        })
    });
    let w = weak.clone();
    let has_pending_messages =
        Rc::new(move || with(&w, |session| Ok(session.pending_message_count() > 0)));
    let w = weak.clone();
    let shutdown = Rc::new(move || {
        let handler = with(&w, |session| {
            Ok(session.state.borrow().bindings.shutdown_handler.clone())
        })?;
        handler.map_or(Ok(()), |handler| handler())
    });
    let w = weak.clone();
    let get_context_usage = Rc::new(move || with(&w, |session| session.get_context_usage()));
    let w = weak.clone();
    let compact = Rc::new(move |options: Option<CompactOptions>| {
        with(&w, |session| {
            drop(session.start_operation(complete_compaction(
                Rc::clone(session),
                options.unwrap_or_default(),
            )));
            Ok(())
        })
    });
    let w = weak.clone();
    let get_system_prompt = Rc::new(move || with(&w, |session| Ok(session.system_prompt())));
    ExtensionContextActions {
        get_model,
        is_idle,
        get_signal,
        abort,
        has_pending_messages,
        shutdown,
        get_context_usage,
        compact,
        get_system_prompt,
    }
}

/// Runs compaction and completion under one error boundary, forwarding either failure.
async fn complete_compaction(session: Rc<AgentSession<Local>>, options: CompactOptions) {
    let result = session
        .compact(options.custom_instructions.as_deref())
        .await
        .map_err(EventError::from)
        .and_then(|result| {
            options
                .on_complete
                .map_or(Ok(()), |complete| complete(result))
        });
    if let (Err(error), Some(failed)) = (result, options.on_error) {
        drop(failed(error));
    }
}

/// The provider routes: each registration change is followed by the current model's refresh.
fn provider_actions(weak: &Weak<AgentSession<Local>>) -> ProviderActions<Local> {
    let w = weak.clone();
    let register = Rc::new(move |name: String, config| {
        with(&w, |session| {
            session
                .model_registry
                .register_provider(&name, config)
                .map_err(thrown)?;
            session.refresh_current_model_from_registry();
            Ok(())
        })
    });
    let w = weak.clone();
    let unregister = Rc::new(move |name: String| {
        with(&w, |session| {
            session
                .model_registry
                .unregister_provider(&name)
                .map_err(thrown)?;
            session.refresh_current_model_from_registry();
            Ok(())
        })
    });
    ProviderActions {
        register_provider: Some(register),
        unregister_provider: Some(unregister),
    }
}

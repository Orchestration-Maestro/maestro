//! Session construction over directory-bound startup services.
use super::bootstrap::collect_settings_diagnostics;
use super::options::{
    BuiltSessionOptions, SessionOptionInputs, SessionOptions, build_session_options,
};
use crate::input::args::Args;
use maestro_app::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, AgentSessionServices,
    AuthStorage, CreateAgentSessionFromServicesOptions, CreateAgentSessionRuntimeResult,
    DiagnosticErrorInfo, Local, ModelResolverOutput, SessionManager, SessionStartEvent,
    SettingsListEntry, create_agent_session_from_services, resolve_model_scope,
};
use maestro_theme::{Collator, LiveTheme};
use std::{borrow::Cow, cell::RefCell, rc::Rc};

/// The retained owners one startup session is created over.
pub struct RuntimeCreation {
    /// Services to use without reloading.
    pub services: Rc<AgentSessionServices<Local>>,
    /// The chosen history owner.
    pub session_manager: Rc<RefCell<SessionManager>>,
    /// The event used when this session starts.
    pub session_start_event: Option<SessionStartEvent>,
    /// The theme view exposed to extensions.
    pub theme: LiveTheme,
    /// The locale comparator prepared for this startup.
    pub model_id_collator: Rc<Collator>,
}

/// Take the string prefix, retaining whether the next stored member was invalid.
fn model_patterns(entries: impl IntoIterator<Item = SettingsListEntry>) -> (Vec<String>, bool) {
    let mut patterns = Vec::new();
    for entry in entries {
        match entry {
            SettingsListEntry::String(pattern) => patterns.push(pattern),
            SettingsListEntry::Unknown(_) => return (patterns, true),
        }
    }
    (patterns, false)
}

/// Resolve patterns, reporting prefix warnings before an invalid stored member.
fn scoped_models(
    parsed: &Args,
    creation: &RuntimeCreation,
    output: &mut ModelResolverOutput<'_>,
) -> Result<Vec<maestro_app::ScopedModel>, DiagnosticErrorInfo> {
    let (patterns, invalid) = if let Some(models) = &parsed.models {
        (Cow::Borrowed(models.as_slice()), false)
    } else {
        let stored = creation
            .services
            .settings_manager
            .borrow()
            .get_enabled_models()
            .unwrap_or_default();
        if stored.is_empty() {
            return Ok(Vec::new());
        }
        let (patterns, invalid) = model_patterns(stored);
        (Cow::Owned(patterns), invalid)
    };
    if patterns.is_empty() && !invalid {
        return Ok(Vec::new());
    }
    let models = resolve_model_scope(
        &patterns,
        &creation.services.model_registry,
        &|left, right| creation.model_id_collator.compare(left, right),
        output,
    )
    .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
    if invalid {
        return Err(DiagnosticErrorInfo::error("Model pattern must be a string"));
    }
    Ok(models)
}

/// Collect service, settings and extension failures in their startup order.
fn runtime_diagnostics(creation: &RuntimeCreation) -> Vec<AgentSessionRuntimeDiagnostic> {
    let mut diagnostics = creation.services.diagnostics.clone();
    diagnostics.extend(collect_settings_diagnostics(
        &mut creation.services.settings_manager.borrow_mut(),
        "runtime creation",
    ));
    diagnostics.extend(
        creation
            .services
            .resource_loader
            .get_extensions()
            .errors
            .iter()
            .map(|failure| AgentSessionRuntimeDiagnostic {
                r#type: AgentSessionRuntimeDiagnosticType::Error,
                message: format!(
                    "Failed to load extension \"{}\": {}",
                    failure.path, failure.error
                ),
            }),
    );
    diagnostics
}

/// Select options after reading the actual selected history.
fn session_options(
    parsed: &Args,
    creation: &RuntimeCreation,
    scoped: &[maestro_app::ScopedModel],
) -> Result<BuiltSessionOptions, DiagnosticErrorInfo> {
    let has_existing_session = !creation
        .session_manager
        .borrow()
        .build_session_context()
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?
        .messages
        .is_empty();
    let settings = creation.services.settings_manager.borrow();
    Ok(build_session_options(SessionOptionInputs {
        parsed,
        scoped_models: scoped,
        has_existing_session,
        model_registry: &creation.services.model_registry,
        settings: &settings,
        compare_model_ids: &|left, right| creation.model_id_collator.compare(left, right),
    }))
}

/// Install a supplied key only for a selected command-line or scoped model.
fn apply_api_key(
    parsed: &Args,
    options: &SessionOptions,
    auth: &AuthStorage,
    diagnostics: &mut Vec<AgentSessionRuntimeDiagnostic>,
) {
    let Some(key) = parsed.api_key.as_deref().filter(|key| !key.is_empty()) else {
        return;
    };
    if let Some(model) = &options.model {
        let model = model
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        auth.set_runtime_api_key(&model.provider, key);
    } else {
        diagnostics.push(AgentSessionRuntimeDiagnostic {
            r#type: AgentSessionRuntimeDiagnosticType::Error,
            message: "--api-key requires a model to be specified via --model, --provider/--model, or --models".into(),
        });
    }
}

/// Construct the real session and reapply only command-line thinking overrides.
async fn create_session(
    parsed: &Args,
    creation: RuntimeCreation,
    built: BuiltSessionOptions,
    diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
) -> Result<CreateAgentSessionRuntimeResult, DiagnosticErrorInfo> {
    let created = create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
        services: Rc::clone(&creation.services),
        session_manager: creation.session_manager,
        session_start_event: creation.session_start_event,
        model: built.options.model,
        thinking_level: built.options.thinking_level,
        scoped_models: built.options.scoped_models.unwrap_or_default(),
        tools: built.options.tools,
        no_tools: built.options.no_tools,
        custom_tools: Vec::new(),
        theme: creation.theme,
    })
    .await?;
    if parsed.thinking.is_some() || built.cli_thinking_from_model {
        created
            .session
            .set_thinking_level(created.session.thinking_level())?;
    }
    Ok(CreateAgentSessionRuntimeResult {
        session: created.session,
        services: creation.services,
        diagnostics,
        extensions_result: created.extensions_result,
        model_fallback_message: created.model_fallback_message,
    })
}

/// Select startup options and create a session over the supplied services.
///
/// # Errors
/// Returns catalog, stored-pattern, session-creation or thinking-selection failures.
pub async fn create_runtime(
    parsed: &Args,
    creation: RuntimeCreation,
    output: &mut ModelResolverOutput<'_>,
) -> Result<CreateAgentSessionRuntimeResult, DiagnosticErrorInfo> {
    let mut diagnostics = runtime_diagnostics(&creation);
    let scoped_models = scoped_models(parsed, &creation, output)?;
    let mut built = session_options(parsed, &creation, &scoped_models)?;
    diagnostics.append(&mut built.diagnostics);
    apply_api_key(
        parsed,
        &built.options,
        &creation.services.auth_storage,
        &mut diagnostics,
    );
    create_session(parsed, creation, built, diagnostics).await
}

#[cfg(test)]
mod tests;

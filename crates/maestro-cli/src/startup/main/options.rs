//! Model, effort, tool and resource-path selections of a created session.

use crate::input::args::Args;
use maestro_app::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, Model, ModelRegistry,
    NoTools, ResolveCliModelOptions, ScopedModel, SettingsManager, ThinkingLevel,
    resolve_cli_model,
};
use std::cmp::Ordering;
use std::io;
use std::sync::{Arc, PoisonError, RwLock};

/// The observations one session-option selection reads.
#[derive(Clone, Copy)]
pub struct SessionOptionInputs<'a> {
    /// The authored arguments.
    pub parsed: &'a Args,
    /// The models selected by the resolved scope, in their cycling order.
    pub scoped_models: &'a [ScopedModel],
    /// Whether the selected session already holds messages.
    pub has_existing_session: bool,
    /// The catalog the selection resolves through.
    pub model_registry: &'a ModelRegistry,
    /// The settings holding the saved default model.
    pub settings: &'a SettingsManager,
    /// The prepared locale comparator of model identifiers.
    pub compare_model_ids: &'a dyn Fn(&str, &str) -> Ordering,
}

/// The selections a session is created with; every absent value takes its default.
#[derive(Default)]
pub struct SessionOptions {
    /// The selected model.
    pub model: Option<Arc<RwLock<Model>>>,
    /// The selected reasoning level.
    pub thinking_level: Option<ThinkingLevel>,
    /// The models to cycle through, when a scope was resolved.
    pub scoped_models: Option<Vec<ScopedModel>>,
    /// The only tools enabled, when the arguments name any.
    pub tools: Option<Vec<String>>,
    /// The default tools to leave off.
    pub no_tools: Option<NoTools>,
}

/// The selected options, the shorthand's origin and their diagnostics.
pub struct BuiltSessionOptions {
    /// The selections the session is created with.
    pub options: SessionOptions,
    /// Whether the model shorthand alone supplied the thinking level.
    pub cli_thinking_from_model: bool,
    /// Diagnostics in their reported order.
    pub diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
}

/// A diagnostic of one severity with its exact message.
fn diagnostic(
    message: String,
    r#type: AgentSessionRuntimeDiagnosticType,
) -> AgentSessionRuntimeDiagnostic {
    AgentSessionRuntimeDiagnostic { r#type, message }
}

/// The shared descriptor read under its own lock.
fn read(model: &RwLock<Model>) -> std::sync::RwLockReadGuard<'_, Model> {
    model.read().unwrap_or_else(PoisonError::into_inner)
}

/// The saved default model of the scope, when the catalog holds it.
///
/// The catalog finds the saved descriptor by its exact provider and identifier, so the
/// lookup only proves it exists and the scope comparison reads the scoped descriptors
/// alone. No read guard survives into the traversal, whose entry may share the saved
/// descriptor's lock.
fn saved_scope_model<'a>(
    scoped_models: &'a [ScopedModel],
    registry: &ModelRegistry,
    settings: &SettingsManager,
) -> Option<&'a ScopedModel> {
    let provider = settings.get_default_provider()?;
    let model_id = settings.get_default_model()?;
    registry.find(&provider, &model_id)?;
    scoped_models.iter().find(|scoped| {
        let model = read(&scoped.model);
        model.provider == provider && model.id == model_id
    })
}

/// Select the model, effort, tool and cycling options of a session.
#[must_use]
pub fn build_session_options(inputs: SessionOptionInputs<'_>) -> BuiltSessionOptions {
    let mut built = BuiltSessionOptions {
        options: SessionOptions {
            scoped_models: (!inputs.scoped_models.is_empty())
                .then(|| inputs.scoped_models.to_vec()),
            ..SessionOptions::default()
        },
        cli_thinking_from_model: false,
        diagnostics: Vec::new(),
    };
    select_explicit_model(inputs, &mut built);
    if built.options.model.is_none() && !inputs.has_existing_session {
        select_scoped_model(inputs, &mut built);
    }
    if let Some(thinking) = inputs.parsed.thinking.clone() {
        built.options.thinking_level = Some(thinking);
    }
    select_tools(&inputs, &mut built.options);
    built
}

/// Select the model an explicit `--model` names, resolved under `--provider` when one is
/// given, with its diagnostics. `--provider` alone selects nothing.
fn select_explicit_model(inputs: SessionOptionInputs<'_>, built: &mut BuiltSessionOptions) {
    let Some(cli_model) = inputs.parsed.model.as_deref() else {
        return;
    };
    let resolved = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: inputs.parsed.provider.as_deref(),
            cli_model: Some(cli_model),
            model_registry: inputs.model_registry,
        },
        inputs.compare_model_ids,
    );
    built.diagnostics.extend(
        resolved
            .warning
            .map(|message| diagnostic(message, AgentSessionRuntimeDiagnosticType::Warning)),
    );
    built.diagnostics.extend(resolved.error.map(|failure| {
        diagnostic(
            failure.to_string(),
            AgentSessionRuntimeDiagnosticType::Error,
        )
    }));
    let Some(model) = resolved.model else {
        return;
    };
    built.options.model = Some(model);
    let shorthand = resolved
        .thinking_level
        .filter(|_| inputs.parsed.thinking.is_none());
    if let Some(level) = shorthand {
        built.options.thinking_level = Some(level);
        built.cli_thinking_from_model = true;
    }
}

/// Select the saved default of the scope, or its first model, for a message-empty session.
fn select_scoped_model(inputs: SessionOptionInputs<'_>, built: &mut BuiltSessionOptions) {
    let saved = saved_scope_model(inputs.scoped_models, inputs.model_registry, inputs.settings);
    let Some(selected) = saved.or_else(|| inputs.scoped_models.first()) else {
        return;
    };
    built.options.model = Some(Arc::clone(&selected.model));
    if inputs.parsed.thinking.is_none() {
        built
            .options
            .thinking_level
            .clone_from(&selected.thinking_level);
    }
}

/// Select the disabled defaults and the explicit allowlist the arguments name.
fn select_tools(inputs: &SessionOptionInputs<'_>, options: &mut SessionOptions) {
    if inputs.parsed.no_tools == Some(true) {
        options.no_tools = Some(NoTools::All);
    } else if inputs.parsed.no_builtin_tools == Some(true) {
        options.no_tools = Some(NoTools::Builtin);
    }
    if let Some(tools) = &inputs.parsed.tools {
        options.tools = Some(tools.clone());
    }
}

/// Resolve each authored local resource path against the original working directory.
///
/// Package and URL sources keep their authored spelling.
///
/// # Errors
/// Returns the resolution failure of a local path.
pub fn resolve_cli_paths(cwd: &str, paths: Option<&[String]>) -> io::Result<Option<Vec<String>>> {
    paths
        .map(|paths| {
            paths
                .iter()
                .map(|value| {
                    if maestro_app::is_local_path(value) {
                        maestro_app::resolve(&[cwd, value])
                    } else {
                        Ok(value.clone())
                    }
                })
                .collect()
        })
        .transpose()
}

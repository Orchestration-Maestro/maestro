//! The retained runtime factory and the owned startup choices it rebuilds services from.
use super::options::resolve_cli_paths;
use super::runtime::{RuntimeCreation, create_runtime};
use super::writers::SharedWriter;
use crate::input::args::Args;
use maestro_app::{
    AuthStorage, CreateAgentSessionRuntimeFactory, CreateAgentSessionServicesOptions,
    DiagnosticErrorInfo, ExtensionFactory, Local, ModelResolverOutput, ModuleLoader, ResourceKind,
    ResourceLoaderOptions, create_agent_session_services,
};
use maestro_theme::{Collator, LiveTheme};
use std::{cell::RefCell, io, io::Write, rc::Rc, sync::Arc};
use tokio::task::LocalSet;

/// The authored resource paths, resolved once against the startup directory.
pub(super) struct CliPaths {
    /// Extension operands.
    extensions: Vec<String>,
    /// Skill operands.
    skills: Vec<String>,
    /// Prompt template operands.
    templates: Vec<String>,
    /// Theme operands.
    themes: Vec<String>,
}

impl CliPaths {
    /// Resolve all four lists over `cwd` before the selected session changes directory.
    ///
    /// # Errors
    /// Returns the path resolver's failure.
    pub(super) fn new(parsed: &Args, cwd: &str) -> io::Result<Self> {
        let resolve = |paths: &Option<Vec<String>>| {
            resolve_cli_paths(cwd, paths.as_deref()).map(Option::unwrap_or_default)
        };
        Ok(Self {
            extensions: resolve(&parsed.extensions)?,
            skills: resolve(&parsed.skills)?,
            templates: resolve(&parsed.prompt_templates)?,
            themes: resolve(&parsed.themes)?,
        })
    }
}

/// The owned choices and handles every created runtime shares.
pub(super) struct RuntimeInputs {
    /// Parsed choices, without the prompt queue and file arguments startup consumes.
    pub(super) parsed: Rc<Args>,
    /// Resource paths resolved against the startup directory.
    pub(super) paths: CliPaths,
    /// Factories supplied by the embedding caller, in order.
    pub(super) extension_factories: Vec<ExtensionFactory<Local>>,
    /// The one credential owner every replacement reuses.
    pub(super) auth: Arc<AuthStorage>,
    /// The task set driving local extension work.
    pub(super) local: Rc<LocalSet>,
    /// The supplied executable module adapter.
    pub(super) module_loader: ModuleLoader<Local>,
    /// The shared current theme.
    pub(super) theme: LiveTheme,
    /// The locale comparator prepared once at startup.
    pub(super) collator: Rc<Collator>,
    /// Ordinary output, routed by the caller.
    pub(super) stdout: Rc<RefCell<dyn Write>>,
    /// Diagnostic output.
    pub(super) stderr: Rc<RefCell<dyn Write>>,
    /// Whether terminal styles are enabled.
    pub(super) color: bool,
    /// The startup offline selection.
    pub(super) offline: bool,
}

/// Rebuild the resource inputs from the retained choices without resolving paths again.
fn resources(inputs: &RuntimeInputs) -> ResourceLoaderOptions<Local> {
    let mut resources =
        ResourceLoaderOptions::new(Rc::clone(&inputs.local), Rc::clone(&inputs.module_loader));
    let paths = &inputs.paths;
    for (target, source) in [
        (&mut resources.additional_extension_paths, &paths.extensions),
        (&mut resources.additional_skill_paths, &paths.skills),
        (
            &mut resources.additional_prompt_template_paths,
            &paths.templates,
        ),
        (&mut resources.additional_theme_paths, &paths.themes),
    ] {
        target.clone_from(source);
    }
    resources
        .extension_factories
        .clone_from(&inputs.extension_factories);
    let parsed = &inputs.parsed;
    resources.system_prompt.clone_from(&parsed.system_prompt);
    resources
        .append_system_prompt
        .clone_from(&parsed.append_system_prompt);
    resources.offline = Some(inputs.offline);
    for (disabled, kind) in [
        (parsed.no_extensions, ResourceKind::Extensions),
        (parsed.no_skills, ResourceKind::Skills),
        (parsed.no_prompt_templates, ResourceKind::PromptTemplates),
        (parsed.no_themes, ResourceKind::Themes),
        (parsed.no_context_files, ResourceKind::ContextFiles),
    ] {
        if disabled == Some(true) {
            resources.disabled.insert(kind);
        }
    }
    resources
}

/// The application's replacement factory over owned startup inputs.
///
/// Each request rebuilds the directory-bound services for its own working directory and
/// session, then selects options and creates the session through the shared helper.
pub(super) fn runtime_factory(inputs: Rc<RuntimeInputs>) -> CreateAgentSessionRuntimeFactory {
    Rc::new(move |request| {
        let inputs = Rc::clone(&inputs);
        Box::pin(async move {
            let services = create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: request.cwd,
                agent_dir: Some(request.agent_dir),
                auth_storage: Some(Arc::clone(&inputs.auth)),
                settings_manager: None,
                model_registry: None,
                extension_flag_values: Some(inputs.parsed.unknown_flags.clone()),
                resource_loader_options: resources(&inputs),
                offline: Some(inputs.offline),
            })
            .await
            .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
            let creation = RuntimeCreation {
                services: Rc::new(services),
                session_manager: request.session_manager,
                session_start_event: request.session_start_event,
                theme: inputs.theme.clone(),
                model_id_collator: Rc::clone(&inputs.collator),
            };
            let mut stdout = SharedWriter(Rc::clone(&inputs.stdout));
            let mut stderr = SharedWriter(Rc::clone(&inputs.stderr));
            let mut output = ModelResolverOutput {
                stdout: &mut stdout,
                stderr: &mut stderr,
                color: inputs.color,
            };
            create_runtime(&inputs.parsed, creation, &mut output).await
        })
    })
}

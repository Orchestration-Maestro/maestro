//! The extension runtime a session builds and rebuilds: its runner, bindings, discovered
//! resources and tool registry, and the reload that replaces them.
use super::state::thrown;
use super::tools::RefreshToolRegistryOptions;
use super::{AgentSession, ExtensionBindings};
use crate::resources::resource_loader::{
    ResourceExtensionPath, ResourceExtensionPaths, ResourceLoader, report,
};
use crate::utils::lock::write;
use indexmap::IndexMap;
use maestro_catalog::ModelRegistry;
use maestro_extensions::runner::{
    ExtensionRunnerSession, ResourcePath, emit_session_shutdown_event,
};
use maestro_extensions::{
    ExtensionRunner, ResourcesDiscoverReason, SessionShutdownEvent, SessionShutdownReason,
    SessionStartReason, types::FlagValue,
};
use maestro_models::{DiagnosticErrorInfo, Execution, Local};
use maestro_resources::{PathMetadata, SourceOrigin, SourceScope};
use maestro_session::{Presence, SessionManager};
use maestro_theme::LiveTheme;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// A runner over the loaded extensions and their shared runtime, serving `cwd`.
pub(super) fn create_runner(
    resource_loader: &dyn ResourceLoader<Local>,
    cwd: &str,
    session_manager: &Rc<RefCell<SessionManager>>,
    model_registry: &Arc<ModelRegistry>,
    theme: &LiveTheme,
) -> Rc<ExtensionRunner<Local>> {
    let extensions = resource_loader.get_extensions();
    Rc::new(ExtensionRunner::new(
        Local::share(RwLock::new(extensions.extensions.clone())),
        extensions.runtime.clone(),
        ExtensionRunnerSession {
            cwd: cwd.to_owned(),
            session_manager: session_manager.clone(),
            model_registry: Arc::clone(model_registry),
        },
        Rc::new(report) as Rc<dyn Fn(&str)>,
        theme.clone(),
    ))
}

impl AgentSession<Local> {
    /// Rebuilds the runtime over the loaded extensions: sets `flag_values` on their shared
    /// runtime, replaces the runner, then binds it and refreshes the tool registry.
    fn build_runtime(
        self: &Rc<Self>,
        active_tool_names: Option<Vec<String>>,
        flag_values: IndexMap<String, FlagValue>,
    ) -> Result<(), DiagnosticErrorInfo> {
        let extensions = self.resource_loader.get_extensions();
        write(&extensions.runtime.flag_values).extend(flag_values);
        let runner = create_runner(
            &*self.resource_loader,
            &self.cwd,
            &self.session_manager,
            &self.model_registry,
            &self.theme,
        );
        if let Some(slot) = &self.extension_runner_ref {
            slot.borrow_mut().replace(Rc::clone(&runner));
        }
        *self.extension_runner.borrow_mut() = runner;
        self.finish_runtime_build(active_tool_names, true)
    }

    /// Rebuilds the built-in definitions, binds the current runner and refreshes the tool
    /// registry, activating `active_tool_names` or the default tools.
    pub(super) fn finish_runtime_build(
        self: &Rc<Self>,
        active_tool_names: Option<Vec<String>>,
        include_all_extension_tools: bool,
    ) -> Result<(), DiagnosticErrorInfo> {
        let base = super::tool_runtime::base_tool_definitions(
            &self.settings_manager,
            &self.cwd,
            &self.bin_dir,
            &self.tools_manager,
            self.base_tools_override.as_ref(),
        )?;
        self.state.borrow_mut().base_tool_definitions = base;
        let runner = self.extension_runner();
        self.bind_extension_core(&runner)?;
        self.apply_extension_bindings(&runner);
        let defaults = self.base_tools_override.as_ref().map_or_else(
            || {
                ["read", "bash", "edit", "write"]
                    .map(str::to_owned)
                    .to_vec()
            },
            |tools| tools.keys().cloned().collect(),
        );
        self.refresh_tool_registry(RefreshToolRegistryOptions {
            active_tool_names: Some(active_tool_names.unwrap_or(defaults)),
            include_all_extension_tools,
        })
    }

    /// Stores each supplied binding, applies them to the runner, sends the session start
    /// event and adds the resources the extensions discover.
    ///
    /// # Errors
    /// Returns an error listener's failure, a resource loading failure or the
    /// documentation-path failure of the rebuilt prompt.
    pub async fn bind_extensions(
        self: &Rc<Self>,
        bindings: ExtensionBindings,
    ) -> Result<(), DiagnosticErrorInfo> {
        {
            let mut state = self.state.borrow_mut();
            let bound = &mut state.bindings;
            bound.ui_context = bindings.ui_context.or(bound.ui_context.take());
            bound.command_context_actions = bindings
                .command_context_actions
                .or(bound.command_context_actions.take());
            bound.shutdown_handler = bindings.shutdown_handler.or(bound.shutdown_handler.take());
            bound.on_error = bindings.on_error.or(bound.on_error.take());
        }
        let runner = self.extension_runner();
        self.apply_extension_bindings(&runner);
        let mut start = self.session_start_event.clone();
        runner.emit(&mut start).await.map_err(thrown)?;
        let reason = if self.session_start_event.reason == SessionStartReason::Reload {
            ResourcesDiscoverReason::Reload
        } else {
            ResourcesDiscoverReason::Startup
        };
        self.extend_resources_from_extensions(reason).await
    }

    /// Hands the skill, prompt and theme paths the extensions discover to the resource
    /// loader and rebuilds the base system prompt.
    async fn extend_resources_from_extensions(
        &self,
        reason: ResourcesDiscoverReason,
    ) -> Result<(), DiagnosticErrorInfo> {
        let runner = self.extension_runner();
        if !runner.has_handlers("resources_discover") {
            return Ok(());
        }
        let found = runner
            .emit_resources_discover(self.cwd.clone(), reason)
            .await
            .map_err(thrown)?;
        if found.skill_paths.is_empty()
            && found.prompt_paths.is_empty()
            && found.theme_paths.is_empty()
        {
            return Ok(());
        }
        self.resource_loader
            .extend_resources(ResourceExtensionPaths {
                skill_paths: extension_resource_paths(found.skill_paths),
                prompt_paths: extension_resource_paths(found.prompt_paths),
                theme_paths: extension_resource_paths(found.theme_paths),
            })
            .map_err(thrown)?;
        self.install_base_system_prompt(&self.get_active_tool_names())
    }

    /// Applies the bound user interface, command operations and error listener to `runner`,
    /// moving the error listener off the runner it was on.
    fn apply_extension_bindings(&self, runner: &ExtensionRunner<Local>) {
        let (ui_context, command_actions, on_error) = {
            let state = self.state.borrow();
            let bound = &state.bindings;
            (
                bound.ui_context.clone(),
                bound.command_context_actions.clone(),
                bound.on_error.clone(),
            )
        };
        runner.set_ui_context_binding(ui_context);
        runner.bind_command_context(command_actions);
        let previous = self.state.borrow_mut().error_unsubscriber.take();
        if let Some(unsubscribe) = previous {
            unsubscribe();
        }
        let next = on_error.map(|listener| Box::new(runner.on_error(listener)) as Box<dyn Fn()>);
        self.state.borrow_mut().error_unsubscriber = next;
    }

    /// Reloads settings, providers and resources and rebuilds the runtime, keeping the flag
    /// values and the active tools. Extensions hear the reload shutdown first, and a fresh
    /// start and resource discovery afterwards when a binding is present.
    ///
    /// # Errors
    /// Returns an error listener's failure, a resource loading failure or the
    /// documentation-path failure of the rebuilt prompt.
    pub async fn reload(self: &Rc<Self>) -> Result<(), DiagnosticErrorInfo> {
        let runner = self.extension_runner();
        let previous_flag_values = runner.get_flag_values();
        let mut shutdown = SessionShutdownEvent {
            reason: SessionShutdownReason::Reload,
            target_session_file: Presence::Missing,
        };
        emit_session_shutdown_event(&runner, &mut shutdown)
            .await
            .map_err(thrown)?;
        let flush = self.settings_manager.borrow().flush();
        flush.await;
        self.settings_manager.borrow_mut().reload_from_storage();
        self.sync_queue_modes_from_settings();
        maestro_models::reset_api_providers();
        self.resource_loader.reload().await.map_err(thrown)?;
        self.model_registry
            .set_base_model_files(self.resource_loader.get_model_files())
            .map_err(thrown)?;
        self.build_runtime(Some(self.get_active_tool_names()), previous_flag_values)?;
        let has_bindings = {
            let state = self.state.borrow();
            let bound = &state.bindings;
            bound.ui_context.is_some()
                || bound.command_context_actions.is_some()
                || bound.shutdown_handler.is_some()
                || bound.on_error.is_some()
        };
        if has_bindings {
            let mut start = maestro_extensions::SessionStartEvent {
                reason: SessionStartReason::Reload,
                previous_session_file: Presence::Missing,
            };
            self.extension_runner()
                .emit(&mut start)
                .await
                .map_err(thrown)?;
            self.extend_resources_from_extensions(ResourcesDiscoverReason::Reload)
                .await?;
        }
        Ok(())
    }
}

/// Each discovered path with the provenance of the extension that contributed it.
fn extension_resource_paths(entries: Vec<ResourcePath>) -> Vec<ResourceExtensionPath> {
    entries
        .into_iter()
        .map(|entry| {
            let inline = entry.extension_path.starts_with('<');
            ResourceExtensionPath {
                path: entry.path,
                metadata: PathMetadata {
                    source: extension_source_label(&entry.extension_path),
                    scope: SourceScope::Temporary,
                    origin: SourceOrigin::TopLevel,
                    base_dir: (!inline).then(|| maestro_path::dirname(&entry.extension_path)),
                },
            }
        })
        .collect()
}

/// `extension:<name>` for an inline `<name>` path, else `extension:` and the file name
/// without a `.ts` or `.js` extension.
fn extension_source_label(extension_path: &str) -> String {
    if extension_path.starts_with('<') {
        return format!("extension:{}", extension_path.replace(['<', '>'], ""));
    }
    let base = maestro_path::basename(extension_path, None);
    let name = base
        .strip_suffix(".ts")
        .or_else(|| base.strip_suffix(".js"))
        .unwrap_or(&base);
    format!("extension:{name}")
}

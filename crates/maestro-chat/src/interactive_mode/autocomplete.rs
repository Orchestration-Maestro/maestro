//! Command, model, skill and file completion for the interactive mode.

use super::InteractiveMode;
use maestro_app::presentation_data::slash_commands::BUILTIN_SLASH_COMMANDS;
use maestro_app::{
    AgentSessionRuntime, Local, ReadonlySessionManager, RegisteredCommand, ResolvedCommand,
    SourceInfo, SourceScope, is_whitespace_scalar, parse_git_url,
};
use maestro_tui::autocomplete::{
    ArgumentCompletions, AutocompleteItem, Command, CompletionError, NativeAutocompleteOperations,
    SlashCommand,
};
use maestro_tui::{AutocompleteProvider, Cancellation, CombinedAutocompleteProvider, fuzzy_filter};
use std::{
    io::{self, IsTerminal},
    rc::Rc,
    sync::PoisonError,
};

/// The provenance marker a completion description carries, or `None` without provenance.
///
/// Authored source text is trimmed by the shared whitespace scalars, so a byte-order mark
/// is removed and U+0085 is kept, as the reference's `String.trim` does.
pub(super) fn get_autocomplete_source_tag(source_info: Option<&SourceInfo>) -> Option<String> {
    let source_info = source_info?;
    let prefix = match source_info.scope {
        SourceScope::User => "u",
        SourceScope::Project => "p",
        SourceScope::Temporary => "t",
    };
    let source = source_info.source.trim_matches(is_whitespace_scalar);
    if matches!(source, "auto" | "local" | "cli") {
        return Some(prefix.to_owned());
    }
    if source.starts_with("npm:") {
        return Some(format!("{prefix}:{source}"));
    }
    let Some(git) = parse_git_url(source) else {
        return Some(prefix.to_owned());
    };
    let reference = git
        .r#ref
        .map_or_else(String::new, |selected| format!("@{selected}"));
    Some(format!("{prefix}:git:{}/{}{reference}", git.host, git.path))
}

/// `description` behind its provenance tag; an absent tag keeps the description as it is.
///
/// Absent and empty descriptions stay distinct: an empty one yields the tag alone.
pub(super) fn prefix_autocomplete_description(
    description: Option<&str>,
    source_info: Option<&SourceInfo>,
) -> Option<String> {
    let Some(tag) = get_autocomplete_source_tag(source_info) else {
        return description.map(str::to_owned);
    };
    Some(match description.filter(|text| !text.is_empty()) {
        Some(text) => format!("[{tag}] {text}"),
        None => format!("[{tag}]"),
    })
}

/// One model candidate before the fuzzy query selects from it.
struct ModelChoice {
    /// The value applied when chosen, `provider/id`.
    value: String,
    /// The model identity shown.
    label: String,
    /// The provider shown beside the identity.
    description: String,
    /// The text the query matches: identity and provider.
    text: String,
}

/// The `/model` argument callback, resolving its candidates when the request arrives.
fn model_argument_completions(runtime: &Rc<AgentSessionRuntime>) -> ArgumentCompletions {
    let runtime = Rc::clone(runtime);
    Rc::new(move |prefix: &str| {
        let runtime = Rc::clone(&runtime);
        let prefix = prefix.to_owned();
        Box::pin(async move { model_candidates(&runtime, &prefix) })
    })
}

/// The candidates the fuzzy `query` selects from the session's scoped or available models.
fn model_candidates(
    runtime: &AgentSessionRuntime,
    query: &str,
) -> Result<Option<Vec<AutocompleteItem>>, CompletionError> {
    let session = runtime.session();
    let scoped = session.scoped_models();
    let models = if scoped.is_empty() {
        session.model_registry().get_available()?
    } else {
        scoped.into_iter().map(|scoped| scoped.model).collect()
    };
    if models.is_empty() {
        return Ok(None);
    }
    let choices: Vec<ModelChoice> = models
        .into_iter()
        .map(|model| {
            let model = model.read().unwrap_or_else(PoisonError::into_inner);
            ModelChoice {
                value: format!("{}/{}", model.provider, model.id),
                label: model.id.clone(),
                description: model.provider.clone(),
                text: format!("{} {}", model.id, model.provider),
            }
        })
        .collect();
    let filtered = fuzzy_filter(&choices, query, |choice| choice.text.as_str());
    Ok((!filtered.is_empty()).then(|| {
        filtered
            .into_iter()
            .map(|choice| AutocompleteItem {
                value: choice.value.clone(),
                label: choice.label.clone(),
                description: Some(choice.description.clone()),
            })
            .collect()
    }))
}

/// The argument callback of `command` in the provider's borrowed form, keeping its error.
fn extension_argument_completions(
    command: &RegisteredCommand<Local>,
) -> Option<ArgumentCompletions> {
    let callback = command.get_argument_completions.clone()?;
    Some(Rc::new(move |prefix: &str| {
        let future = callback(prefix.to_owned());
        Box::pin(async move { future.await.map_err(|error| -> CompletionError { error }) })
    }))
}

/// The commands extensions registered, offered under their invocation names.
///
/// A registration whose raw name is a builtin command is skipped, because the builtin
/// owns that name; duplicate names keep every registration in registration order.
fn extension_commands(commands: &[ResolvedCommand<Local>]) -> Vec<Command> {
    commands
        .iter()
        .filter(|command| {
            !BUILTIN_SLASH_COMMANDS
                .iter()
                .any(|builtin| builtin.name == command.command.name)
        })
        .map(|command| {
            let description = {
                let source_info = command
                    .command
                    .source_info
                    .read()
                    .unwrap_or_else(PoisonError::into_inner);
                prefix_autocomplete_description(
                    command.command.description.as_deref(),
                    Some(&source_info),
                )
            };
            Command::SlashCommand(SlashCommand {
                name: command.invocation_name.clone(),
                description,
                argument_hint: None,
                get_argument_completions: extension_argument_completions(&command.command),
            })
        })
        .collect()
}

/// One rebuild's commands in completion order: builtins, templates, extensions, skills.
fn base_commands(
    runtime: &Rc<AgentSessionRuntime>,
    extensions: &[ResolvedCommand<Local>],
) -> Vec<Command> {
    let session = runtime.session();
    let mut commands: Vec<Command> = BUILTIN_SLASH_COMMANDS
        .iter()
        .map(|command| {
            let completions =
                (command.name == "model").then(|| model_argument_completions(runtime));
            Command::SlashCommand(SlashCommand {
                name: command.name.clone(),
                description: Some(command.description.clone()),
                argument_hint: command.argument_hint.clone(),
                get_argument_completions: completions,
            })
        })
        .collect();
    commands.extend(session.prompt_templates().prompts.iter().map(|template| {
        Command::SlashCommand(SlashCommand {
            name: template.name.clone(),
            description: prefix_autocomplete_description(
                Some(&template.description),
                Some(&template.source_info),
            ),
            argument_hint: template.argument_hint.clone(),
            get_argument_completions: None,
        })
    }));
    commands.extend(extension_commands(extensions));
    let enabled = session
        .settings_manager()
        .borrow()
        .get_enable_skill_commands();
    if enabled {
        let skills = session.resource_loader().get_skills();
        commands.extend(skills.skills.iter().map(|skill| {
            Command::SlashCommand(SlashCommand {
                name: format!("skill:{}", skill.name),
                description: prefix_autocomplete_description(
                    Some(&skill.description),
                    Some(&skill.source_info),
                ),
                argument_hint: None,
                get_argument_completions: None,
            })
        }));
    }
    commands
}

impl InteractiveMode {
    /// The base provider over the current session, `extensions` and the search program
    /// prepared during startup.
    pub(super) fn create_base_autocomplete_provider(
        &self,
        extensions: &[ResolvedCommand<Local>],
    ) -> Rc<dyn AutocompleteProvider<Signal = Cancellation>> {
        let session = self.runtime_host.session();
        let cwd = session.session_manager().borrow().get_cwd().to_owned();
        let fd_path = self.fd_path.borrow().clone();
        Rc::new(CombinedAutocompleteProvider::new(
            base_commands(&self.runtime_host, extensions),
            cwd,
            fd_path,
            NativeAutocompleteOperations::default(),
        ))
    }

    /// Folds live registered wrappers over a fresh base, then installs the final provider
    /// on the default editor and a distinct current editor.
    ///
    /// # Errors
    /// Returns the original wrapper or current-editor failure.
    pub(super) fn setup_autocomplete_provider(&self) -> Result<(), maestro_app::EventError> {
        let extensions = self
            .runtime_host
            .session()
            .extension_runner()
            .get_registered_commands();
        let mut provider = self.create_base_autocomplete_provider(&extensions);
        let mut index = 0;
        loop {
            let factory = self
                .autocomplete_provider_wrappers
                .borrow()
                .get(index)
                .cloned();
            let Some(factory) = factory else { break };
            provider = factory.call(provider)?;
            index += 1;
        }
        drop(
            self.editor_state
                .autocomplete
                .replace(Some(Rc::clone(&provider))),
        );
        self.editor.set_autocomplete_provider(Rc::clone(&provider));
        if self.custom_editor_current() {
            self.current_editor()
                .try_set_autocomplete_provider(provider)?;
        }
        Ok(())
    }

    /// Ensures `fd` and `rg` are available before the first completion, both started before
    /// either finishes, and keeps `fd` for file completion.
    ///
    /// Progress keeps the search owner's own lines.
    ///
    /// # Errors
    /// Returns the failure of locating the managed binary directory.
    pub(super) async fn prepare_search_tools(&self) -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            maestro_app::configuration::config::paths::get_bin_dir(&maestro_app::get_agent_dir()?);
        let manager =
            maestro_app::agent_session::ToolsManager::new(directory, self.options.fetch.clone())
                .with_offline(self.runtime_host.services().offline);
        let color = io::stdout().is_terminal();
        let mut fd_output = io::stdout();
        let mut rg_output = io::stdout();
        let fd = manager.ensure_tool(
            maestro_app::SearchTool::Fd,
            Some(maestro_app::ToolsManagerOutput {
                stdout: &mut fd_output,
                color,
            }),
        );
        let rg = manager.ensure_tool(
            maestro_app::SearchTool::Rg,
            Some(maestro_app::ToolsManagerOutput {
                stdout: &mut rg_output,
                color,
            }),
        );
        let (fd, _) = tokio::join!(fd, rg);
        *self.fd_path.borrow_mut() = fd;
        Ok(())
    }
}

//! Tool registry, active tools and the rebuilt system prompt.
use super::{AgentSession, ToolDefinitionHandle, state::thrown};
use crate::resources::resource_loader::ResourceLoader;
use crate::resources::system_prompt::{BuildSystemPromptOptions, build_system_prompt};
use crate::utils::lock::{read, write};
use indexmap::{IndexMap, IndexSet};
use maestro_extensions::{
    ExtensionExecution, ExtensionRunner, RegisteredTool, wrap_registered_tool,
};
use maestro_models::{DiagnosticErrorInfo, ToolInfo};
use maestro_resources::{SyntheticSourceOptions, create_synthetic_source_info};
use maestro_session::Presence;
use std::{rc::Rc, sync::RwLock};

impl<E: ExtensionExecution> AgentSession<E> {
    /// Names of the active tools, in order.
    #[must_use]
    pub fn get_active_tool_names(&self) -> Vec<String> {
        let state = read(self.agent.state());
        let tools = read(state.tools());
        tools
            .iter()
            .map(|tool| read(tool).definition.name.clone())
            .collect()
    }

    /// Every registered tool with its description, schema and source.
    #[must_use]
    pub fn get_all_tools(
        &self,
    ) -> Vec<ToolInfo<maestro_models::ParameterSchema<maestro_models::SchemaCode>>> {
        self.state
            .borrow()
            .tool_definitions
            .values()
            .map(|tool| {
                let definition = tool.metadata().definition;
                ToolInfo {
                    name: definition.name,
                    description: definition.description,
                    parameters: definition.parameters,
                    source_info: read(&tool.source_info).clone(),
                }
            })
            .collect()
    }

    /// The registered definition of `name`.
    #[must_use]
    pub fn get_tool_definition(&self, name: &str) -> Option<RegisteredTool<E>> {
        self.state.borrow().tool_definitions.get(name).cloned()
    }

    /// Rebuilds the definitions from the built-in ones, then the extension and SDK tools by
    /// name, keeping only allowed names; replaces their normalized prompt snippets and
    /// guidelines and one executable tool per definition; then activates the tools `options`
    /// select and rebuilds the system prompt.
    ///
    /// Requested names replace the active tools, which otherwise stay active; unallowed names
    /// are dropped. With an allowlist every allowed registry tool is activated; otherwise,
    /// when `options` include every extension tool, the extension and SDK tools are added,
    /// and without requested names the tools new to the registry are. Repeated names are
    /// dropped.
    ///
    /// # Errors
    /// Returns the documentation-path failure of the base prompt.
    pub(super) fn refresh_tool_registry(
        &self,
        options: RefreshToolRegistryOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let previous_active_tool_names = self.get_active_tool_names();
        let runner = self.extension_runner();
        let (definitions, extension_tool_names) = self.tool_definitions(&runner);
        let mut snippets = IndexMap::new();
        let mut guidelines = IndexMap::new();
        let mut registry = IndexMap::new();
        for (name, tool) in &definitions {
            let metadata = tool.metadata();
            if let Some(snippet) = normalize_prompt_snippet(metadata.prompt_snippet.as_deref()) {
                snippets.insert(name.clone(), snippet);
            }
            let normalized = normalize_prompt_guidelines(metadata.prompt_guidelines.as_deref());
            if !normalized.is_empty() {
                guidelines.insert(name.clone(), normalized);
            }
            let wrapped = wrap_registered_tool(tool, Rc::clone(&runner));
            registry.insert(name.clone(), Rc::new(RwLock::new(wrapped)));
        }
        let next = self.refreshed_tool_names(
            options,
            &registry,
            extension_tool_names,
            previous_active_tool_names,
        );
        let mut state = self.state.borrow_mut();
        state.tool_registry = registry;
        state.tool_definitions = definitions;
        state.tool_prompt_snippets = snippets;
        state.tool_prompt_guidelines = guidelines;
        drop(state);
        self.activate_tools(&next)
    }

    /// Admits refresh candidates before publishing registry or active-state changes.
    fn refreshed_tool_names(
        &self,
        options: RefreshToolRegistryOptions,
        registry: &IndexMap<String, maestro_agent::SharedAgentTool<maestro_models::Local>>,
        extension_tool_names: Vec<String>,
        previous: Vec<String>,
    ) -> Vec<String> {
        let requested = options.active_tool_names.is_some();
        let mut next = options.active_tool_names.unwrap_or(previous);
        next.retain(|name| self.is_allowed_tool(name));
        if self.allowed_tool_names.is_some() {
            next.extend(registry.keys().cloned());
        } else if options.include_all_extension_tools {
            next.extend(extension_tool_names);
        } else if !requested {
            let state = self.state.borrow();
            next.extend(
                registry
                    .keys()
                    .filter(|name| !state.tool_registry.contains_key(*name))
                    .cloned(),
            );
        }
        let names: IndexSet<String> = next.into_iter().collect();
        names
            .into_iter()
            .filter(|name| self.admit_tools(std::slice::from_ref(name)).is_ok())
            .collect()
    }

    /// The allowed built-in definitions with their built-in sources, then the allowed
    /// extension and SDK tools replacing them by name; with the names of those tools.
    fn tool_definitions(
        &self,
        runner: &ExtensionRunner<E>,
    ) -> (IndexMap<String, RegisteredTool<E>>, Vec<String>) {
        let state = self.state.borrow();
        let custom_tools: Vec<RegisteredTool<E>> = runner
            .get_all_registered_tools()
            .into_iter()
            .chain(
                state
                    .custom_tools
                    .iter()
                    .map(|definition| sourced(definition, "sdk")),
            )
            .filter(|tool| self.is_allowed_tool(&tool.metadata().definition.name))
            .collect();
        let mut definitions: IndexMap<String, RegisteredTool<E>> = state
            .base_tool_definitions
            .iter()
            .filter(|(name, _)| self.is_allowed_tool(name))
            .map(|(name, definition)| (name.clone(), sourced(definition, "builtin")))
            .collect();
        drop(state);
        let names: Vec<String> = custom_tools
            .iter()
            .map(|tool| tool.metadata().definition.name)
            .collect();
        definitions.extend(names.iter().cloned().zip(custom_tools));
        (definitions, names)
    }

    /// Whether `name` passes the allowlist, when there is one.
    fn is_allowed_tool(&self, name: &str) -> bool {
        self.allowed_tool_names
            .as_ref()
            .is_none_or(|allowed| allowed.contains(name))
    }

    /// Activates the registered tools among `tool_names`, in order, and rebuilds the system
    /// prompt for them; unknown names are skipped.
    ///
    /// # Errors
    /// Returns publisher admission refusals or the documentation-path failure of
    /// the default prompt.
    pub fn set_active_tools_by_name(
        &self,
        tool_names: &[String],
    ) -> Result<(), DiagnosticErrorInfo> {
        self.admit_tools(tool_names)?;
        self.activate_tools(tool_names)
    }

    /// Installs an already admitted active set at the original prompt rebuild position.
    fn activate_tools(&self, tool_names: &[String]) -> Result<(), DiagnosticErrorInfo> {
        let (tools, valid): (Vec<_>, Vec<_>) = {
            let state = self.state.borrow();
            tool_names
                .iter()
                .filter_map(|name| {
                    state
                        .tool_registry
                        .get(name)
                        .map(|tool| (Rc::clone(tool), name.clone()))
                })
                .unzip()
        };
        write(self.agent.state()).set_tools(tools);
        self.install_base_system_prompt(&valid)
    }

    /// Rebuilds the base system prompt for the registered tools among `tool_names` and makes
    /// it the agent's prompt.
    ///
    /// # Errors
    /// Returns the documentation-path failure of the base prompt.
    pub(super) fn install_base_system_prompt(
        &self,
        tool_names: &[String],
    ) -> Result<(), DiagnosticErrorInfo> {
        let prompt = self.rebuild_system_prompt(tool_names).map_err(thrown)?;
        let effective = {
            let mut state = self.state.borrow_mut();
            state.base_system_prompt = prompt;
            state.effective_system_prompt()
        };
        write(self.agent.state()).system_prompt = effective;
        Ok(())
    }

    /// The system prompt for the registered tools among `tool_names`; its options become the
    /// base options.
    fn rebuild_system_prompt(&self, tool_names: &[String]) -> std::io::Result<String> {
        let options = self.system_prompt_options(tool_names);
        let prompt = build_system_prompt(&options);
        self.state.borrow_mut().base_system_prompt_options = options;
        prompt
    }

    /// The options that build the system prompt for the registered tools among `tool_names`.
    fn system_prompt_options(&self, tool_names: &[String]) -> BuildSystemPromptOptions {
        let state = self.state.borrow();
        let selected: Vec<String> = tool_names
            .iter()
            .filter(|name| state.tool_registry.contains_key(*name))
            .cloned()
            .collect();
        let mut snippets = IndexMap::new();
        let mut prompt_guidelines = Vec::new();
        for name in &selected {
            if let Some(snippet) = state.tool_prompt_snippets.get(name) {
                snippets.insert(name.clone(), snippet.clone());
            }
            if let Some(lines) = state.tool_prompt_guidelines.get(name) {
                prompt_guidelines.extend(lines.iter().cloned());
            }
        }
        drop(state);
        prompt_options(
            &*self.resource_loader,
            &self.cwd,
            selected,
            snippets,
            prompt_guidelines,
        )
    }
}

/// The options that build the system prompt in `cwd` from the loaded resources and the
/// selected tools' snippets and guidelines.
pub(super) fn prompt_options<E: ExtensionExecution>(
    resource_loader: &dyn ResourceLoader<E>,
    cwd: &str,
    selected: Vec<String>,
    snippets: IndexMap<String, String>,
    prompt_guidelines: Vec<String>,
) -> BuildSystemPromptOptions {
    let append = resource_loader.get_append_system_prompt();
    BuildSystemPromptOptions {
        cwd: cwd.to_owned(),
        skills: Presence::Present(resource_loader.get_skills().skills.clone()),
        context_files: Presence::Present(resource_loader.get_agents_files().to_vec()),
        custom_prompt: resource_loader
            .get_system_prompt()
            .map_or(Presence::Missing, |text| {
                Presence::Present(text.to_string())
            }),
        append_system_prompt: if append.is_empty() {
            Presence::Missing
        } else {
            Presence::Present(append.join("\n\n"))
        },
        selected_tools: Presence::Present(selected),
        tool_snippets: Presence::Present(snippets),
        prompt_guidelines: Presence::Present(prompt_guidelines),
    }
}

/// One line of snippet text: whitespace runs become one space; blank text is absent.
fn normalize_prompt_snippet(text: Option<&str>) -> Option<String> {
    let words: Vec<&str> = text?
        .split(maestro_text::is_whitespace_scalar)
        .filter(|word| !word.is_empty())
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Trimmed, non-empty guidelines without repeats, in first-seen order.
fn normalize_prompt_guidelines(guidelines: Option<&[String]>) -> Vec<String> {
    guidelines
        .unwrap_or_default()
        .iter()
        .map(|line| line.trim_matches(maestro_text::is_whitespace_scalar))
        .filter(|line| !line.is_empty())
        .collect::<IndexSet<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// What a tool-registry refresh activates.
#[derive(Default)]
pub(super) struct RefreshToolRegistryOptions {
    /// The tools to activate in place of the active ones.
    pub(super) active_tool_names: Option<Vec<String>>,
    /// Whether every extension and SDK tool is activated as well.
    pub(super) include_all_extension_tools: bool,
}

/// `definition` with the synthetic source `<source:name>` of the kind `source`.
fn sourced<E: ExtensionExecution>(
    definition: &ToolDefinitionHandle<E>,
    source: &str,
) -> RegisteredTool<E> {
    let tool = RegisteredTool {
        definition: definition.clone(),
        source_info: synthetic(String::new(), source),
    };
    let name = tool.metadata().definition.name;
    RegisteredTool {
        source_info: synthetic(format!("<{source}:{name}>"), source),
        ..tool
    }
}

/// The synthetic source `path` of the kind `source`.
fn synthetic(path: String, source: &str) -> std::sync::Arc<RwLock<maestro_resources::SourceInfo>> {
    let info = create_synthetic_source_info(
        path,
        SyntheticSourceOptions {
            source: source.to_owned(),
            scope: None,
            origin: None,
            base_dir: None,
        },
    );
    std::sync::Arc::new(RwLock::new(info))
}

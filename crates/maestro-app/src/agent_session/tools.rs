//! Tool registry, active tools and the rebuilt system prompt.
use super::{AgentSession, RefreshToolRegistryOptions, state::thrown};
use crate::resources::resource_loader::ResourceLoader;
use crate::resources::system_prompt::{BuildSystemPromptOptions, build_system_prompt};
use crate::utils::lock::{read, write};
use indexmap::{IndexMap, IndexSet};
use maestro_extensions::{ExtensionExecution, RegisteredTool, wrap_registered_tool};
use maestro_models::{DiagnosticErrorInfo, ToolInfo};
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
    pub fn get_all_tools(&self) -> Vec<ToolInfo> {
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

    /// The session's tool-registry refresh: replaces the definitions by name in registry
    /// order, their normalized prompt snippets and guidelines and one executable tool per
    /// definition, then activates the tools `options` select and rebuilds the system prompt.
    ///
    /// Requested names replace the active tools, which otherwise stay active. When `options`
    /// include every extension tool, the tools named in `extension_tool_names` are added;
    /// otherwise, without requested names, the tools new to the registry are. Repeated
    /// names are dropped.
    ///
    /// # Errors
    /// Returns the documentation-path failure of the default prompt.
    pub fn refresh_tool_registry(
        &self,
        definitions: IndexMap<String, RegisteredTool<E>>,
        extension_tool_names: &[String],
        options: RefreshToolRegistryOptions,
    ) -> Result<(), DiagnosticErrorInfo> {
        let previous_active_tool_names = self.get_active_tool_names();
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
            let wrapped = wrap_registered_tool(tool, Rc::clone(&self.extension_runner));
            registry.insert(name.clone(), Rc::new(RwLock::new(wrapped)));
        }
        let mut state = self.state.borrow_mut();
        let previous_registry = std::mem::replace(&mut state.tool_registry, registry);
        state.tool_definitions = definitions;
        state.tool_prompt_snippets = snippets;
        state.tool_prompt_guidelines = guidelines;
        let requested = options.active_tool_names.is_some();
        let mut next = options
            .active_tool_names
            .unwrap_or(previous_active_tool_names);
        if options.include_all_extension_tools {
            next.extend_from_slice(extension_tool_names);
        } else if !requested {
            let added = state.tool_registry.keys();
            next.extend(
                added
                    .filter(|name| !previous_registry.contains_key(*name))
                    .cloned(),
            );
        }
        drop(state);
        let next: IndexSet<String> = next.into_iter().collect();
        self.set_active_tools_by_name(&Vec::from_iter(next))
    }

    /// Activates the registered tools among `tool_names`, in order, and rebuilds the system
    /// prompt for them; unknown names are skipped.
    ///
    /// # Errors
    /// Returns the documentation-path failure of the default prompt.
    pub fn set_active_tools_by_name(
        &self,
        tool_names: &[String],
    ) -> Result<(), DiagnosticErrorInfo> {
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
        let prompt = self.rebuild_system_prompt(&valid).map_err(thrown)?;
        self.state
            .borrow_mut()
            .base_system_prompt
            .clone_from(&prompt);
        write(self.agent.state()).system_prompt = prompt;
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

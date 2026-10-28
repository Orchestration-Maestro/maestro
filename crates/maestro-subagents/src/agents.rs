//! Named Markdown definitions from the user directory and nearest project directory.

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, config,
    frontmatter::{FrontmatterValue, parse_frontmatter},
    tools::path_utils::{guest_path, resolve},
};
use serde::{Deserialize, Serialize};

/// Directories admitted for one discovery operation.
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentScope {
    /// Read only the user's definitions.
    #[default]
    User,
    /// Read only the nearest project's definitions.
    Project,
    /// Read both, with project definitions replacing names from the user directory.
    Both,
}

/// An authored prompt with its original provenance.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    /// Name the tool accepts.
    pub name: String,
    /// Description shown in diagnostics.
    pub description: String,
    /// Explicit tool selection, absent when all defaults apply.
    pub tools: Option<Vec<String>>,
    /// Explicit model selection.
    pub model: Option<String>,
    /// Prompt body appended to the child's instructions.
    pub system_prompt: String,
    /// User or project origin.
    pub source: String,
    /// Authored native path, including any symbolic-link spelling.
    pub file_path: String,
}

/// Fresh definitions and the nearest project directory, even in user scope.
pub struct AgentDiscoveryResult {
    /// Definitions in first-name admission order.
    pub agents: Vec<AgentConfig>,
    /// Closest existing project agents directory.
    pub project_agents_dir: Option<String>,
}

/// Reads definitions afresh, preserving names' positions when a project replaces them.
///
/// # Errors
/// Returns host path/configuration failures or malformed metadata errors.
pub fn discover_agents(
    api: &ExtensionAPI,
    cwd: &str,
    scope: AgentScope,
) -> ExtensionResult<AgentDiscoveryResult> {
    let user = resolve(api, &[&config::get_agent_dir(api)?, "agents"])?;
    let project_agents_dir = nearest_project(api, cwd)?;
    let mut agents = Vec::<AgentConfig>::new();
    let mut directories = Vec::new();
    if !matches!(scope, AgentScope::Project) {
        directories.push((user.as_str(), "user"));
    }
    if !matches!(scope, AgentScope::User)
        && let Some(project) = &project_agents_dir
    {
        directories.push((project.as_str(), "project"));
    }
    for (directory, source) in directories {
        for agent in load(api, directory, source)? {
            if let Some(position) = agents.iter().position(|old| old.name == agent.name) {
                agents[position] = agent;
            } else {
                agents.push(agent);
            }
        }
    }
    Ok(AgentDiscoveryResult {
        agents,
        project_agents_dir,
    })
}

/// Walks lexical parents while following links when observing a candidate directory.
fn nearest_project(api: &ExtensionAPI, cwd: &str) -> ExtensionResult<Option<String>> {
    let mut current = cwd.to_owned();
    loop {
        let candidate = resolve(api, &[&current, ".maestro", "agents"])?;
        if std::fs::metadata(guest_path(api, &candidate)?).is_ok_and(|metadata| metadata.is_dir()) {
            return Ok(Some(candidate));
        }
        let parent = resolve(api, &[&current, ".."])?;
        if parent == current {
            return Ok(None);
        }
        current = parent;
    }
}

/// Ignores unreadable directory entries and follows authored file links.
fn load(api: &ExtensionAPI, directory: &str, source: &str) -> ExtensionResult<Vec<AgentConfig>> {
    let Ok(entries) = std::fs::read_dir(guest_path(api, directory)?) else {
        return Ok(Vec::new());
    };
    let mut agents = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if std::path::Path::new(&name)
            .extension()
            .is_none_or(|extension| extension != "md")
            || !entry
                .file_type()
                .is_ok_and(|kind| kind.is_file() || kind.is_symlink())
        {
            continue;
        }
        let file_path = resolve(api, &[directory, &name])?;
        let Ok(bytes) = std::fs::read(guest_path(api, &file_path)?) else {
            continue;
        };
        let parsed = parse_frontmatter(api, &String::from_utf8_lossy(&bytes))?;
        if let Some(agent) = definition(parsed, source, file_path) {
            agents.push(agent);
        }
    }
    Ok(agents)
}

/// Accepts named, described definitions and the optional comma-separated tool selection.
fn definition(
    parsed: maestro_extensions_wasm::frontmatter::ParsedFrontmatter,
    source: &str,
    file_path: String,
) -> Option<AgentConfig> {
    let FrontmatterValue::Mapping(fields) = parsed.frontmatter else {
        return None;
    };
    let name = text(&fields, "name").filter(|value| !value.is_empty())?;
    let description = text(&fields, "description").filter(|value| !value.is_empty())?;
    let tools = text(&fields, "tools")
        .map(|tools| {
            tools
                .split(',')
                .map(str::trim)
                .filter(|tool| !tool.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|tools| !tools.is_empty());
    Some(AgentConfig {
        name,
        description,
        tools,
        model: text(&fields, "model"),
        system_prompt: parsed.body,
        source: source.into(),
        file_path,
    })
}

/// Reads a text member without treating malformed scalar types as agent names.
fn text(fields: &[(String, FrontmatterValue)], name: &str) -> Option<String> {
    fields.iter().find_map(|(key, value)| match value {
        FrontmatterValue::String(value) if key == name => Some(value.clone()),
        _ => None,
    })
}

/// Formats an ordered, bounded list of agent descriptions.
#[must_use]
pub fn format_agent_list(agents: &[AgentConfig], max_items: usize) -> (String, usize) {
    if agents.is_empty() {
        return ("none".into(), 0);
    }
    let text = agents
        .iter()
        .take(max_items)
        .map(|agent| format!("{} ({}): {}", agent.name, agent.source, agent.description))
        .collect::<Vec<_>>()
        .join("; ");
    (text, agents.len().saturating_sub(max_items))
}

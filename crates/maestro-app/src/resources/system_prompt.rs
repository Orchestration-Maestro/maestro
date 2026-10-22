//! System prompt assembly from tools, guidelines, context files and skills.

pub use maestro_models::system_prompt::BuildSystemPromptOptions;

use crate::configuration::config::paths::{get_docs_path, get_examples_path, get_readme_path};
use maestro_resources::format_skills_for_prompt;
use maestro_session::Presence;
use std::io;

/// The tools listed when the caller selects none.
const DEFAULT_TOOLS: [&str; 4] = ["read", "bash", "edit", "write"];

/// The present value of an optional field.
fn present<T>(value: &Presence<T>) -> Option<&T> {
    match value {
        Presence::Present(value) => Some(value),
        Presence::Missing | Presence::Null => None,
    }
}

/// The present, non-empty text of an optional field.
fn text(value: &Presence<String>) -> Option<&str> {
    present(value)
        .map(String::as_str)
        .filter(|text| !text.is_empty())
}

/// Assembles the system prompt; documentation paths come from `package_dir`.
///
/// A non-empty custom prompt replaces the default one, which lists the selected tools
/// that have a snippet and the guidelines. Both are followed by the appended text, the
/// context files, the skills when the `read` tool is available, the local date and the
/// working directory.
///
/// # Errors
/// Returns the working-directory error when a relative package directory needs it; only
/// the default prompt reads documentation paths.
pub fn build_system_prompt(
    options: &BuildSystemPromptOptions,
    package_dir: &str,
) -> io::Result<String> {
    let selected = present(&options.selected_tools);
    let (mut prompt, has_read) = if let Some(custom) = text(&options.custom_prompt) {
        let has_read = selected.is_none_or(|tools| tools.iter().any(|tool| tool == "read"));
        (custom.to_owned(), has_read)
    } else {
        let tools: Vec<&str> = selected.map_or_else(
            || DEFAULT_TOOLS.to_vec(),
            |tools| tools.iter().map(String::as_str).collect(),
        );
        let has_read = tools.contains(&"read");
        (default_prompt(options, &tools, package_dir)?, has_read)
    };
    if let Some(append) = text(&options.append_system_prompt) {
        prompt.push_str("\n\n");
        prompt.push_str(append);
    }
    let context_files = present(&options.context_files).map_or(&[][..], Vec::as_slice);
    if !context_files.is_empty() {
        prompt
            .push_str("\n\n# Project Context\n\nProject-specific instructions and guidelines:\n\n");
        prompt.extend(
            context_files
                .iter()
                .map(|file| format!("## {}\n\n{}\n\n", file.path, file.content)),
        );
    }
    let skills = present(&options.skills).map_or(&[][..], Vec::as_slice);
    if has_read && !skills.is_empty() {
        prompt.push_str(&format_skills_for_prompt(skills));
    }
    let date = chrono::Local::now().format("%Y-%m-%d");
    let cwd = options.cwd.replace('\\', "/");
    Ok(format!(
        "{prompt}\nCurrent date: {date}\nCurrent working directory: {cwd}"
    ))
}

/// Guidelines in first-seen order without repeats: the exploration guideline the tools
/// call for, the trimmed caller guidelines, then the two standing ones.
fn guidelines(options: &BuildSystemPromptOptions, tools: &[&str]) -> String {
    let has = |name: &str| tools.contains(&name);
    let searches = has("grep") || has("find") || has("ls");
    let exploration = match (has("bash"), searches) {
        (true, false) => Some("Use bash for file operations like ls, rg, find"),
        (true, true) => Some(
            "Prefer grep/find/ls tools over bash for file exploration (faster, respects .gitignore)",
        ),
        (false, _) => None,
    };
    let extra = present(&options.prompt_guidelines)
        .into_iter()
        .flatten()
        .map(|guideline| guideline.trim_matches(maestro_text::is_whitespace_scalar))
        .filter(|guideline| !guideline.is_empty());
    let mut list: Vec<&str> = Vec::new();
    for guideline in exploration.into_iter().chain(extra).chain([
        "Be concise in your responses",
        "Show file paths clearly when working with files",
    ]) {
        if !list.contains(&guideline) {
            list.push(guideline);
        }
    }
    list.iter()
        .map(|guideline| format!("- {guideline}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The default prompt: identity, the tools with a snippet, the guidelines and where the
/// documentation lives.
fn default_prompt(
    options: &BuildSystemPromptOptions,
    tools: &[&str],
    package_dir: &str,
) -> io::Result<String> {
    let snippets = present(&options.tool_snippets);
    let listed: Vec<String> = tools
        .iter()
        .filter_map(|name| {
            let snippet = snippets?.get(*name).filter(|snippet| !snippet.is_empty())?;
            Some(format!("- {name}: {snippet}"))
        })
        .collect();
    let tools_list = if listed.is_empty() {
        "(none)".to_owned()
    } else {
        listed.join("\n")
    };
    let guidelines = guidelines(options, tools);
    let readme = get_readme_path(package_dir)?;
    let docs = get_docs_path(package_dir)?;
    let examples = get_examples_path(package_dir)?;
    Ok(format!(
        "You are an expert coding assistant operating inside maestro, a coding agent harness. You help users by reading files, executing commands, editing code, and writing new files.

Available tools:
{tools_list}

In addition to the tools above, you may have access to other custom tools depending on the project.

Guidelines:
{guidelines}

Maestro documentation (read only when the user asks about maestro itself, its SDK, extensions, themes, skills, or TUI):
- Main documentation: {readme}
- Additional docs: {docs}
- Examples: {examples} (extensions, custom tools, SDK)
- When asked about: extensions (docs/extensions.md, examples/extensions/), themes (docs/themes.md), skills (docs/skills.md), prompt templates (docs/prompt-templates.md), TUI components (docs/tui.md), keybindings (docs/keybindings.md), SDK integrations (docs/sdk.md), custom providers (docs/custom-provider.md), adding models (docs/models.md), maestro packages (docs/packages.md)
- When working on maestro topics, read the docs and examples, and follow .md cross-references before implementing
- Always read maestro .md files completely and follow links to related docs (e.g., tui.md for TUI API details)"
    ))
}

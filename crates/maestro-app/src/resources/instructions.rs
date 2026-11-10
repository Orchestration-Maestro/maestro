//! System prompt assembly from tools, guidelines, context files and skills.

pub use maestro_models::system_prompt::BuildSystemPromptOptions;

use crate::configuration::config::{get_docs_path, get_examples_path, get_readme_path};
use maestro_resources::format_skills_for_prompt;
use maestro_session::Presence;
use std::{collections::HashSet, io};

/// The tools listed when the caller selects none.
const DEFAULT_TOOLS: [&str; 4] = ["read", "bash", "edit", "write"];

/// The present, non-empty text of an optional field.
fn text(value: &Presence<String>) -> Option<&str> {
    value
        .present()
        .map(String::as_str)
        .filter(|text| !text.is_empty())
}

/// Assembles the system prompt; documentation paths come from the running process's
/// package directory.
///
/// A non-empty custom prompt replaces the default one, which lists the selected tools
/// that have a snippet and the guidelines. Both are followed by the appended text, the
/// context files, the skills when the `read` tool is available, the local date and the
/// working directory.
///
/// # Errors
/// Returns the package-directory error, or the working-directory error when a relative
/// package directory needs it; only the default prompt reads documentation paths.
pub fn build_system_prompt(options: &BuildSystemPromptOptions) -> io::Result<String> {
    let selected = options.selected_tools.present();
    let (mut prompt, has_read) = if let Some(custom) = text(&options.custom_prompt) {
        let has_read = selected.is_none_or(|tools| tools.iter().any(|tool| tool == "read"));
        (custom.to_owned(), has_read)
    } else {
        let tools: Vec<&str> = selected.map_or_else(
            || DEFAULT_TOOLS.to_vec(),
            |tools| tools.iter().map(String::as_str).collect(),
        );
        let has_read = tools.contains(&"read");
        (default_prompt(options, &tools)?, has_read)
    };
    if let Some(append) = text(&options.append_system_prompt) {
        prompt.push_str("\n\n");
        prompt.push_str(append);
    }
    let context_files = options
        .context_files
        .present()
        .map_or(&[][..], Vec::as_slice);
    if !context_files.is_empty() {
        prompt
            .push_str("\n\n# Project Context\n\nProject-specific instructions and guidelines:\n\n");
        prompt.extend(
            context_files
                .iter()
                .map(|file| format!("## {}\n\n{}\n\n", file.path, file.content)),
        );
    }
    let skills = options.skills.present().map_or(&[][..], Vec::as_slice);
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
    let exploration = (has("bash") && !["grep", "find", "ls"].into_iter().any(has))
        .then_some("Use bash to explore files, for example ls, rg or find");
    let extra = options
        .prompt_guidelines
        .present()
        .into_iter()
        .flatten()
        .map(|guideline| guideline.trim_matches(maestro_text::is_whitespace_scalar))
        .filter(|guideline| !guideline.is_empty());
    let mut seen = HashSet::new();
    exploration
        .iter()
        .copied()
        .chain(extra)
        .chain([
            "Be concise in your responses",
            "Name file paths explicitly when you work with files",
        ])
        .filter(|guideline| seen.insert(*guideline))
        .map(|guideline| format!("- {guideline}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The default prompt: identity, the tools with a snippet, the guidelines and where the
/// documentation lives.
fn default_prompt(options: &BuildSystemPromptOptions, tools: &[&str]) -> io::Result<String> {
    let snippets = options.tool_snippets.present();
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
    let readme = get_readme_path()?;
    let docs = get_docs_path()?;
    let examples = get_examples_path()?;
    Ok(format!(
        "You are an expert coding assistant working inside maestro, a coding-agent harness. You serve users by reading files, running commands, editing code and creating new files; the tools at your disposal follow.

Available tools:
{tools_list}

Beyond the tools above, the project may supply further custom tools.

Guidelines:
{guidelines}

Maestro documentation (consult it only for questions about maestro, including its SDK, extensions, themes, skills and TUI):
- Main documentation: {readme}
- Additional docs: {docs}
- Examples: {examples} (extensions, custom tools, SDK)
- When reading maestro docs or examples, resolve docs/... under Additional docs and examples/... under Examples, not the current working directory
- Where to look by topic: themes (docs/theme.md), extensions (docs/extensions.md and examples/extensions/), skills (docs/resources/skills.md), prompt templates (see docs/resources/prompt-templates.md), TUI components (see docs/terminal/components.md), keybindings (docs/terminal/keybindings.md), SDK integrations (docs/application/sdk.md), custom providers (docs/extensions/guest_streams.md), adding models (docs/catalog.md), maestro packages (docs/packages.md)
- For maestro work, read the docs and examples first and follow .md cross-references before you implement anything
- Read each maestro .md file in full and follow its links to related docs, such as docs/terminal/components.md for the TUI API"
    ))
}

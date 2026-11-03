//! A prompt-guidance extension that reads the supplied prompt options.
// maestro-component: MAESTRO_PROMPT_CUSTOMIZER_COMPONENT
#![cfg(any(test, target_arch = "wasm32"))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::borrow::Cow;
use std::rc::Rc;

use maestro_extensions_wasm::{
    BeforeAgentStartEventResult, BuildSystemPromptOptions, ExtensionAPI, ExtensionContext,
    ExtensionEvent, ExtensionEventResult, ExtensionFuture, Presence,
};

/// The context this extension contributes to every prompt.
const EXTENSION_SPECIFIC: &str = "\n## Extension-Added Context\n\nThis prompt includes tool guidance and skill information loaded dynamically.\nIf you have additional requirements, configure them via --append-system-prompt or project context files.\n";

/// The guidance each built-in tool earns, in the order it is written.
const GUIDANCE: [(&str, &str); 4] = [
    (
        "read",
        "• Use the `read` tool for file contents (supports text and images).\n  - For large files, use `offset` and `limit` to read in chunks.",
    ),
    (
        "bash",
        "• Execute commands with the `bash` tool. Use it for file operations like `ls`, `find`, `grep`.",
    ),
    (
        "edit",
        "• Use the `edit` tool for precise text replacements in files. Match exact content including whitespace.",
    ),
    (
        "write",
        "• Use the `write` tool to create new files or overwrite existing ones completely.",
    ),
];

/// Adds guidance for the supplied tools and skills to an assembled prompt.
fn add_tool_guidance(options: &BuildSystemPromptOptions, base_prompt: &str) -> String {
    let selected = options.selected_tools.present();
    let mut parts: Vec<Cow<'static, str>> = GUIDANCE
        .iter()
        .filter(|(name, _)| selected.is_some_and(|tools| tools.iter().any(|tool| tool == name)))
        .map(|(_, guidance)| Cow::Borrowed(*guidance))
        .collect();
    if let Some(skills) = options.skills.present().filter(|skills| !skills.is_empty()) {
        let names: Vec<&str> = skills.iter().map(|skill| skill.name.as_str()).collect();
        parts.push(Cow::Owned(format!(
            "\nAvailable skills: {}",
            names.join(", ")
        )));
        parts.push(Cow::Borrowed(
            "Use skill documentation for best practices on specific tools.",
        ));
    }
    if parts.is_empty() {
        return base_prompt.to_owned();
    }
    format!(
        "{base_prompt}\n\n## Tool Guidance\n\n{}\n",
        parts.join("\n")
    )
}

/// Returns the guidance and this extension's context over the supplied base.
fn before_agent_start(
    event: &mut ExtensionEvent,
    _context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::BeforeAgentStart(event) = event else {
            return Ok(None);
        };
        let guidance = add_tool_guidance(&event.system_prompt_options, &event.system_prompt);
        Ok(Some(ExtensionEventResult::BeforeAgentStart(
            BeforeAgentStartEventResult {
                message: Presence::Missing,
                system_prompt: Presence::Present(format!("{guidance}{EXTENSION_SPECIFIC}")),
            },
        )))
    })
}

/// Registers the guidance handler through the supplied extension host.
pub(crate) fn prompt_customizer(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("before_agent_start", Rc::new(before_agent_start)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(prompt_customizer);

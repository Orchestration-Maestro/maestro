//! Lists the project's rule files in the system prompt.
//!
//! At each session start the extension scans `.claude/rules/` in the working directory for
//! Markdown files at any depth. Every agent run then gets their paths in its system prompt, so
//! the agent can read the ones that apply. The files are never read here.
// maestro-component: MAESTRO_PROJECT_RULES_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::RefCell, io, path::Path, rc::Rc};

use maestro_extensions_wasm::{
    BeforeAgentStartEventResult, ExtensionAPI, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionResult, NotificationType, Presence,
    tools::path_utils::guest_path,
};

/// The folder below the working directory that holds the rule files.
const RULES_DIR: &str = ".claude/rules";

/// Collects the Markdown files below `dir`, folder by folder in name order, each path led by `prefix`.
///
/// # Errors
/// Returns an error if metadata or directory entries cannot be read, except for missing paths.
fn find_markdown_files(dir: &Path, prefix: &str, found: &mut Vec<String>) -> io::Result<()> {
    match std::fs::metadata(dir) {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    for entry in maestro_listing::read_dir(dir)? {
        let relative = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            find_markdown_files(&entry.path(), &format!("{relative}/"), found)?;
        } else if kind.is_file() && relative.strip_suffix(".md").is_some() {
            found.push(relative);
        }
    }
    Ok(())
}

/// Appends the rule list to the event's system prompt, or adds nothing without rules.
fn append_rules(rules: &[String], event: &ExtensionEvent) -> Option<ExtensionEventResult> {
    let ExtensionEvent::BeforeAgentStart(event) = event else {
        return None;
    };
    if rules.is_empty() {
        return None;
    }
    let list: Vec<_> = rules
        .iter()
        .map(|rule| format!("- {RULES_DIR}/{rule}"))
        .collect();
    Some(ExtensionEventResult::BeforeAgentStart(
        BeforeAgentStartEventResult {
            message: Presence::Missing,
            system_prompt: Presence::Present(format!(
                "{}\n\n## Project Rules\n\nProject rule files in {RULES_DIR}/:\n\n{}\n\nUse the read tool to load any rule files that apply to your task for guidance.\n",
                event.system_prompt,
                list.join("\n")
            )),
        },
    ))
}

/// Scans the rule folder again, keeps what it finds and announces a nonempty result.
///
/// # Errors
/// Returns an error if the host cannot resolve the working directory, read the rule folder,
/// access the UI or send the notification.
fn scan(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    rules: &RefCell<Vec<String>>,
) -> ExtensionResult<()> {
    let dir = Path::new(&guest_path(api, &ctx.cwd()?)?).join(RULES_DIR);
    let mut found = Vec::new();
    find_markdown_files(&dir, "", &mut found).map_err(|error| error.to_string())?;
    let count = found.len();
    *rules.borrow_mut() = found;
    if count > 0 {
        ctx.ui()?.notify(
            &format!("Project rules: {count} file(s) in {RULES_DIR}/"),
            Some(NotificationType::Info),
        )?;
    }
    Ok(())
}

/// Registers the session-start scan and the prompt hook that share one rule list.
///
/// # Errors
/// The returned future fails if the host rejects either handler registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let rules = Rc::new(RefCell::new(Vec::<String>::new()));
        let (scanned, sender) = (rules.clone(), api.clone());
        api.on(
            "session_start",
            Rc::new(move |_, ctx| {
                let (rules, api) = (scanned.clone(), sender.clone());
                Box::pin(async move {
                    scan(&api, &ctx, &rules)?;
                    Ok(None)
                })
            }),
        )?;
        api.on(
            "before_agent_start",
            Rc::new(move |event, _| {
                let result = append_rules(&rules.borrow(), event);
                Box::pin(async move { Ok(result) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

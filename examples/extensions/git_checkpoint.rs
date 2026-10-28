//! Opt-in Git stash checkpoints for restoring code when forking a conversation.
// maestro-component: MAESTRO_GIT_CHECKPOINT_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, NotificationType, SessionEntry, SessionEvent, utils::is_whitespace_char,
};

/// Checkpoint objects and the last leaf observed after a tool result.
#[derive(Default)]
struct Checkpoints {
    /// Object IDs keyed by session entry, replaced on each capture for that entry.
    refs: BTreeMap<String, String>,
    /// Last observed entry, retained when no leaf exists or the agent ends.
    current_entry_id: Option<String>,
}

/// Takes the identity from any of the host's session-entry variants.
fn entry_id(entry: SessionEntry) -> String {
    match entry {
        SessionEntry::Message(value) => value.base.id,
        SessionEntry::ThinkingLevelChange(value) => value.base.id,
        SessionEntry::ModelChange(value) => value.base.id,
        SessionEntry::Compaction(value) => value.base.id,
        SessionEntry::BranchSummary(value) => value.base.id,
        SessionEntry::Custom(value) => value.base.id,
        SessionEntry::CustomMessage(value) => value.base.id,
        SessionEntry::Label(value) => value.base.id,
        SessionEntry::SessionInfo(value) => value.base.id,
    }
}

/// Trims only scalars the host's text utility classifies as whitespace.
fn trim<'a>(api: &ExtensionAPI, text: &'a str) -> ExtensionResult<&'a str> {
    let mut start = 0;
    for (offset, character) in text.char_indices() {
        if !is_whitespace_char(api, &character.to_string())? {
            break;
        }
        start = offset + character.len_utf8();
    }
    let mut end = text.len();
    for (offset, character) in text[start..].char_indices().rev() {
        if !is_whitespace_char(api, &character.to_string())? {
            break;
        }
        end = start + offset;
    }
    Ok(&text[start..end])
}

/// Applies an approved checkpoint and announces success only after a clean completion.
async fn restore(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    reference: String,
) -> ExtensionResult<()> {
    let result = api
        .exec("git", &["stash".into(), "apply".into(), reference], None)
        .await?;
    if result.code != 0 || result.killed {
        return Err(format!(
            "Could not restore checkpoint (exit code {}, killed: {}): {}",
            result.code, result.killed, result.stderr
        ));
    }
    ctx.ui()?
        .notify("Code restored to checkpoint", Some(NotificationType::Info))
}

/// Captures and restores objects without keeping a state borrow across a host await.
fn on_event(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    api: ExtensionAPI,
    checkpoints: Rc<RefCell<Checkpoints>>,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        match event {
            ExtensionEvent::ToolResult(_) => {
                if let Some(leaf) = ctx.session_manager()?.get_leaf_entry()? {
                    checkpoints.borrow_mut().current_entry_id = Some(entry_id(leaf));
                }
            }
            ExtensionEvent::TurnStart(_) => {
                let result = api
                    .exec("git", &["stash".into(), "create".into()], None)
                    .await?;
                let reference = trim(&api, &result.stdout)?;
                let mut checkpoints = checkpoints.borrow_mut();
                if let Some(id) = checkpoints
                    .current_entry_id
                    .clone()
                    .filter(|_| !reference.is_empty())
                {
                    checkpoints.refs.insert(id, reference.to_owned());
                }
            }
            ExtensionEvent::Session(SessionEvent::BeforeFork(event)) => {
                let reference = checkpoints.borrow().refs.get(&event.entry_id).cloned();
                let Some(reference) = reference else {
                    return Ok(None);
                };
                if !ctx.has_ui()? {
                    return Ok(None);
                }
                let choice = ctx
                    .ui()?
                    .select(
                        "Restore code state?".into(),
                        vec![
                            "Yes, restore code to that point".into(),
                            "No, keep current code".into(),
                        ],
                        None,
                    )
                    .await?;
                if choice.is_some_and(|choice| choice.starts_with("Yes")) {
                    restore(&api, &ctx, reference).await?;
                }
            }
            ExtensionEvent::AgentEnd(_) => checkpoints.borrow_mut().refs.clear(),
            _ => {}
        }
        Ok(None)
    })
}

/// Registers the four checkpoint lifecycle hooks through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let checkpoints = Rc::new(RefCell::new(Checkpoints::default()));
        for name in [
            "tool_result",
            "turn_start",
            "session_before_fork",
            "agent_end",
        ] {
            let state = Rc::clone(&checkpoints);
            let registered = api.clone();
            api.on(
                name,
                Rc::new(move |event, ctx| {
                    on_event(event, ctx, registered.clone(), Rc::clone(&state))
                }),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

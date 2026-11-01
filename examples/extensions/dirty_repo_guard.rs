//! Opt-in guard that asks before leaving or forking a repository with uncommitted changes.
// maestro-component: MAESTRO_DIRTY_REPO_GUARD_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, NotificationType, Presence, SessionBeforeForkResult,
    SessionBeforeSwitchReason, SessionBeforeSwitchResult, SessionEvent,
};

/// The selector choice that lets the session change go ahead.
const CONTINUE: &str = "Continue";

/// Reports whether `action` must be cancelled because the working tree has uncommitted files.
async fn refuse(api: &ExtensionAPI, ctx: &ExtensionContext, action: &str) -> ExtensionResult<bool> {
    let status = api
        .exec("git", &["status".into(), "--porcelain".into()], None)
        .await?;
    // A failed status means there is no repository to guard.
    if status.code != 0 {
        return Ok(false);
    }
    let changed = status
        .stdout
        .trim()
        .lines()
        .filter(|line| !line.is_empty())
        .count();
    if changed == 0 {
        return Ok(false);
    }
    if !ctx.has_ui()? {
        return Ok(true);
    }
    let choice = ctx
        .ui()?
        .select(
            format!("{action} with {changed} uncommitted file(s)?"),
            vec![CONTINUE.into(), "Cancel and commit first".into()],
            None,
        )
        .await?;
    if choice.as_deref() == Some(CONTINUE) {
        return Ok(false);
    }
    ctx.ui()?.notify(
        "Cancelled. Commit the changes, then try again.",
        Some(NotificationType::Warning),
    )?;
    Ok(true)
}

/// Cancels a new session, a session switch or a fork the user does not confirm.
fn guard(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    api: ExtensionAPI,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let (action, cancelled) = match event {
            ExtensionEvent::Session(SessionEvent::BeforeSwitch(event)) => (
                match event.reason {
                    SessionBeforeSwitchReason::New => "Start a new session",
                    SessionBeforeSwitchReason::Resume => "Switch sessions",
                },
                ExtensionEventResult::SessionBeforeSwitch(SessionBeforeSwitchResult {
                    cancel: Presence::Present(true),
                }),
            ),
            ExtensionEvent::Session(SessionEvent::BeforeFork(_)) => (
                "Fork",
                ExtensionEventResult::SessionBeforeFork(SessionBeforeForkResult {
                    cancel: Presence::Present(true),
                    skip_conversation_restore: Presence::Missing,
                }),
            ),
            _ => return Ok(None),
        };
        Ok(refuse(&api, &ctx, action).await?.then_some(cancelled))
    })
}

/// Registers the session guards with the supplied host.
///
/// # Errors
///
/// The returned future fails if the host rejects a guard registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        for name in ["session_before_switch", "session_before_fork"] {
            let registered = api.clone();
            api.on(
                name,
                Rc::new(move |event, ctx| guard(event, ctx, registered.clone())),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

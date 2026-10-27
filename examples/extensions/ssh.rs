//! Opt-in SSH operations for the native coding tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BeforeAgentStartEventResult, ExtensionAPI, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionResult, FlagOptions, FlagType, FlagValue,
    NotificationType, Presence, SessionEvent, ToolsOptions, UserBashEventResult,
};
use std::{cell::RefCell, rc::Rc};

#[path = "ssh/operations.rs"]
mod operations;
#[path = "ssh/transport.rs"]
mod transport;
use operations::Remote;

/// Destination resolved after the host supplies its command-line flags.
type State = Rc<RefCell<Option<Remote>>>;

/// Keeps each built-in definition while selecting operations at execution time.
fn register(api: &ExtensionAPI, name: &'static str, state: State) -> ExtensionResult<()> {
    let mut definition = api.create_tool_definition(name, ".".into(), ToolsOptions::default())?;
    let registered = api.clone();
    definition.execute = Rc::new(move |id, args, signal, update, ctx| {
        let api = registered.clone();
        let remote = state.borrow().clone();
        Box::pin(async move {
            let options = remote.as_ref().map_or_else(
                || Ok(ToolsOptions::default()),
                |remote| remote.options(name),
            )?;
            let tool = api.create_tool(name, ctx.cwd()?, options)?;
            (tool.execute)(id, args, signal, update).await
        })
    });
    api.register_tool(definition)
}

/// Resolves the optional destination before reporting its directory to the user.
async fn start(api: &ExtensionAPI, ctx: &ExtensionContext, state: &State) -> ExtensionResult<()> {
    let was_remote = state.borrow_mut().take().is_some();
    let Some(FlagValue::String(value)) = api
        .get_flag("ssh")?
        .filter(|value| matches!(value, FlagValue::String(text) if !text.is_empty()))
    else {
        if was_remote {
            ctx.ui()?.set_status("ssh", None)?;
        }
        return Ok(());
    };
    let (host, cwd) = if let Some((host, cwd)) = value.split_once(':') {
        (host.to_owned(), cwd.to_owned())
    } else {
        let bytes = transport::execute(api, &value, "pwd".into(), None).await?;
        (value, String::from_utf8_lossy(&bytes).trim().to_owned())
    };
    if host.is_empty() || cwd.is_empty() {
        return Err("SSH destination and working directory must not be empty".into());
    }
    let destination = format!("{host}:{cwd}");
    *state.borrow_mut() = Some(Remote {
        api: api.clone(),
        host,
        cwd,
        local_cwd: ctx.cwd()?,
    });
    let ui = ctx.ui()?;
    let status = ui.theme()?.fg("accent", &format!("SSH: {destination}"))?;
    ui.set_status("ssh", Some(&status))?;
    ui.notify(
        &format!("SSH mode: {destination}"),
        Some(NotificationType::Info),
    )
}

/// Shares the resolved operations with user commands and the supplied system prompt.
fn on_event(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
    api: ExtensionAPI,
    state: State,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if matches!(event, ExtensionEvent::Session(SessionEvent::Start(_))) {
            start(&api, &ctx, &state).await?;
            return Ok(None);
        }
        let Some(remote) = state.borrow().clone() else {
            return Ok(None);
        };
        match event {
            ExtensionEvent::UserBash(_) => {
                Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
                    operations: Some(remote.bash()?),
                    result: Presence::Missing,
                })))
            }
            ExtensionEvent::BeforeAgentStart(event) => {
                let prompt = event.system_prompt.replacen(
                    &format!("Current working directory: {}", remote.local_cwd),
                    &format!(
                        "Current working directory: {} (via SSH: {})",
                        remote.cwd, remote.host
                    ),
                    1,
                );
                Ok(Some(ExtensionEventResult::BeforeAgentStart(
                    BeforeAgentStartEventResult {
                        message: Presence::Missing,
                        system_prompt: Presence::Present(prompt),
                    },
                )))
            }
            _ => Ok(None),
        }
    })
}

/// Registers remote coding tools, a flag and session hooks through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_flag(
            "ssh",
            FlagOptions {
                description: Some("SSH remote: user@host or user@host:/path".into()),
                type_: FlagType::String,
                default: None,
            },
        )?;
        let state = Rc::new(RefCell::new(None));
        for name in ["read", "write", "edit", "bash"] {
            register(&api, name, Rc::clone(&state))?;
        }
        for name in ["session_start", "user_bash", "before_agent_start"] {
            let (registered, state) = (api.clone(), Rc::clone(&state));
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

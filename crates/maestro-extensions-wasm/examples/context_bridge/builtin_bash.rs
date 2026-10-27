//! Shell construction and interception exercised through retained native factories.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    BashExecResult, BashOperations, BashSpawnContext, BashSpawnHook, BashToolOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionResult,
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};

/// Independently selected shell controls and execution operands.
#[derive(Deserialize)]
struct Request {
    /// Literal factory directory.
    cwd: String,
    /// Explicit executable for the real local backend.
    shell: Option<String>,
    /// Literal command prefix.
    prefix: Option<String>,
    /// Replaces the backend without probing the configured shell.
    supplied: bool,
    /// Selects the context-free executable rather than the definition.
    executable: bool,
    /// Literal command.
    command: String,
    /// Optional synchronous interception mode.
    hook: Option<String>,
    /// Directory selected by the hook.
    hook_cwd: Option<String>,
    /// Invokes the same retained tool twice to observe fresh per-call operands.
    #[serde(default)]
    repeat: bool,
}
/// Complete reached callback operands in invocation order.
type Calls = Rc<RefCell<Vec<serde_json::Value>>>;

/// Observes the consuming backend rather than only constructor operands.
pub(super) async fn execute(
    api: &ExtensionAPI,
    context: ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<String> {
    let request: Request = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let calls = Calls::default();
    let signal = if request.hook.as_deref() == Some("abort") {
        context
            .signal()?
            .map(|signal| super::exec::relay(signal, None).0)
    } else {
        None
    };
    let options = BashToolOptions {
        command_prefix: request.prefix,
        shell_path: request.shell,
        operations: request.supplied.then(|| supplied(calls.clone())),
        spawn_hook: request
            .hook
            .map(|mode| hook(mode, request.hook_cwd, context.clone(), calls.clone())),
    };
    let args = serde_json::json!({"command":request.command});
    let result = if request.executable {
        let tool = api.create_bash_tool(request.cwd, options)?;
        if request.repeat {
            (tool.execute)("first".into(), args.clone(), signal.clone(), None).await?;
        }
        (tool.execute)("bash".into(), args, signal, None).await
    } else {
        let tool = api.create_bash_tool_definition(request.cwd, options)?;
        if request.repeat {
            (tool.execute)(
                "first".into(),
                args.clone(),
                signal.clone(),
                None,
                (*context).clone(),
            )
            .await?;
        }
        (tool.execute)("bash".into(), args, signal, None, (*context).clone()).await
    };
    Ok(serde_json::json!({"calls":*calls.borrow(),"result":result}).to_string())
}

/// Captures the actual final native exec operands, with raw byte output.
fn supplied(calls: Calls) -> BashOperations {
    BashOperations {
        exec: Rc::new(move |command, cwd, options| {
            calls.borrow_mut().push(serde_json::json!({
                "command":command,"cwd":cwd,"timeout":options.timeout,
                "hookValue":options.env.as_ref().and_then(|env| env.get("MAESTRO_BUILTIN_HOOK")).cloned(),
                "home":options.env.as_ref().is_some_and(|env| env.contains_key("HOME")),
                "unset":options.env.as_ref().and_then(|env| env.get("UNSET_BY_HOOK")).cloned(),
                "unsetPresent":options.env.as_ref().is_some_and(|env| env.contains_key("UNSET_BY_HOOK")),
                "aborted":options.signal.as_ref().is_some_and(maestro_extensions_wasm::AbortSignal::aborted),
                "bin":options.env.as_ref().and_then(|env| env.get("PATH")).and_then(Option::as_ref).is_some_and(|path| path.contains("/unused-builtin-bin"))
            }));
            Box::pin(async move {
                (options.on_data)("supplied\0é😀".as_bytes())?;
                Ok(BashExecResult {
                    exit_code: Some(0.0),
                })
            })
        }),
    }
}

/// Replaces owned operands or fails synchronously before the backend starts.
fn hook(
    mode: String,
    cwd: Option<String>,
    context: ExtensionCommandContext,
    calls: Calls,
) -> BashSpawnHook {
    Rc::new(move |mut spawn, emitter| {
        calls.borrow_mut().push(serde_json::json!({"hook":spawn.command,"cwd":spawn.cwd,
            "fresh":!spawn.env.contains_key("MAESTRO_BUILTIN_HOOK"),"home":spawn.env.contains_key("HOME")}));
        emitter.emit("builtin-bash-spawn", serde_json::json!("hook"))?;
        if mode == "error" {
            return Err("hook rejected é".into());
        }
        if mode == "abort" {
            context.abort()?;
            return Ok(spawn);
        }
        let cwd = cwd.clone().ok_or("missing hook directory")?;
        if mode == "replace" {
            return Ok(BashSpawnContext {
                command: "printf replacement".into(),
                cwd,
                env: [("MAESTRO_BUILTIN_HOOK".into(), Some("replacement".into()))]
                    .into_iter()
                    .collect(),
            });
        }
        spawn.command =
            "printf '%s:%s' \"$MAESTRO_BUILTIN_HOOK\" \"${UNSET_BY_HOOK-unset}\"".into();
        spawn.cwd = cwd;
        spawn.env.shift_remove("HOME");
        spawn
            .env
            .insert("MAESTRO_BUILTIN_HOOK".into(), Some("edited".into()));
        spawn.env.insert("UNSET_BY_HOOK".into(), None);
        Ok(spawn)
    })
}

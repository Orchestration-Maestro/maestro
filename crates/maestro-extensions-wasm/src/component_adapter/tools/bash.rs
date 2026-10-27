//! Native shell factories reuse the returned-shell and synchronous preparation transports.
use super::super::super::{
    callbacks::Identity,
    generated::{Generated, host},
};
use crate::{BashExecResult, BashOperations, BashToolOptions, ExtensionResult};
use std::rc::Rc;

/// Retains the original native shell definition, including its render state.
pub(crate) fn bash(cwd: &str, options: BashToolOptions) -> ExtensionResult<crate::ToolDefinition> {
    super::definition(native(cwd, options)?, false)
}

/// Projects only executable metadata and context-free execution from that same owner.
pub(crate) fn executable_bash(cwd: &str, options: BashToolOptions) -> ExtensionResult<crate::Tool> {
    super::executable(native(cwd, options)?)
}

/// Admits independently optional callbacks using the original table and rollback policy.
fn native(cwd: &str, options: BashToolOptions) -> ExtensionResult<Rc<host::BuiltinTool>> {
    let operations = options.operations.map(|operations| {
        let identity = Identity::new(&Generated);
        identity.stage(operations);
        identity
    });
    let hook = options.spawn_hook.map(spawn_hook);
    let controls = host::BashToolControls {
        command_prefix: options.command_prefix,
        shell_path: options.shell_path,
    };
    let tool = host::create_bash_tool(
        cwd,
        &controls,
        operations.as_ref().map(|identity| &identity.handle),
        hook.as_ref().map(|identity| &identity.handle),
    );
    for identity in operations.into_iter().chain(hook) {
        identity.settle(tool.is_ok());
    }
    Ok(Rc::new(tool?))
}

/// Retains one original native local backend and lends each execution its own observers.
#[cfg(target_arch = "wasm32")]
pub(crate) fn local_operations(shell_path: Option<&str>) -> ExtensionResult<BashOperations> {
    use super::super::super::{callbacks::Scoped, signals::Mirror};
    let operations = Rc::new(host::create_local_bash_operations(shell_path)?);
    Ok(BashOperations {
        exec: Rc::new(move |command, cwd, options| {
            let operations = operations.clone();
            Box::pin(async move {
                let mirror = Mirror::new(&Generated, options.signal.as_ref())?;
                let on_data: crate::EventListener = Rc::new(move |bytes, _| {
                    let bytes: Vec<u8> =
                        serde_json::from_value(bytes).map_err(|error| error.to_string())?;
                    (options.on_data)(&bytes)?;
                    Ok(None)
                });
                let output = Scoped::new(&Generated, on_data);
                let request = host::BashRequest {
                    command,
                    cwd,
                    timeout: options.timeout,
                    env: options.env.map(|env| env.into_iter().collect()),
                };
                let result = operations
                    .exec(request, mirror.signal(), &output.handle)
                    .await;
                mirror
                    .settle(result)
                    .map(|exit_code| BashExecResult { exit_code })
            })
        }),
    })
}

/// Stages synchronous interception through the existing preparation callback frame.
pub(super) fn spawn_hook(hook: crate::BashSpawnHook) -> Identity<Generated> {
    let identity = Identity::new(&Generated);
    let prepare: crate::PrepareArguments = Rc::new(move |args, emitter| {
        let context = serde_json::from_value(args.clone()).map_err(|error| error.to_string())?;
        let context = hook(context, emitter)?;
        serde_json::to_value(context).map_err(|error| error.to_string())
    });
    identity.stage(prepare);
    identity
}

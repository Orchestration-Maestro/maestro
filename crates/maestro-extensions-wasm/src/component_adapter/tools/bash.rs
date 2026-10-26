//! Native shell factories reuse the returned-shell and synchronous preparation transports.
use super::super::super::{
    callbacks::Identity,
    generated::{Generated, host},
};
use crate::{BashToolOptions, ExtensionResult};
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
    let hook = options.spawn_hook.map(|hook| {
        let identity = Identity::new(&Generated);
        let prepare: crate::PrepareArguments = Rc::new(move |args, emitter| {
            let context =
                serde_json::from_value(args.clone()).map_err(|error| error.to_string())?;
            let context = hook(context, emitter)?;
            serde_json::to_value(context).map_err(|error| error.to_string())
        });
        identity.stage(prepare);
        identity
    });
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

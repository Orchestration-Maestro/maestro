//! Named factory selection keeps native metadata, preparation and owned callbacks.
use super::super::super::{
    callbacks::{Identity, Stored},
    generated::{Generated, host},
};
use crate::{ExtensionResult, ToolsOptions};
use std::rc::Rc;

/// Projects a context-free executable from the native selector's retained definition.
pub(crate) fn executable(
    name: &str,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<crate::Tool> {
    let tool = native(name, cwd, options)?;
    let prepare = (name == "edit").then(|| super::prepare(tool.clone()));
    let mut executable = super::executable(tool)?;
    executable.prepare_arguments = prepare;
    Ok(executable)
}

/// Keeps the native selector's definition callbacks and context requirements.
pub(crate) fn definition(
    name: &str,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<crate::ToolDef> {
    let tool = native(name, cwd, options)?;
    let prepare = (name == "edit").then(|| super::prepare(tool.clone()));
    let mut definition = super::definition(tool, name == "read")?;
    definition.prepare_arguments = prepare;
    Ok(definition)
}

/// Borrows independently staged capabilities for one native factory admission.
fn native(name: &str, cwd: &str, options: ToolsOptions) -> ExtensionResult<Rc<host::BuiltinTool>> {
    let callbacks = [
        stage(options.read.operations),
        stage(options.bash.operations),
        stage(options.edit.operations),
        stage(options.write.operations),
        stage(options.grep.operations),
        stage(options.find.operations),
        stage(options.ls.operations),
        options.bash.spawn_hook.map(super::bash::spawn_hook),
    ];
    let handles = callbacks
        .each_ref()
        .map(|identity| identity.as_ref().map(|identity| &identity.handle));
    let controls = host::ToolFactoryOptions {
        read_auto_resize_images: options.read.auto_resize_images,
        bash_controls: host::BashToolControls {
            command_prefix: options.bash.command_prefix,
            shell_path: options.bash.shell_path,
        },
        read_operations: handles[0],
        bash_operations: handles[1],
        edit_operations: handles[2],
        write_operations: handles[3],
        grep_operations: handles[4],
        find_operations: handles[5],
        ls_operations: handles[6],
        spawn_hook: handles[7],
    };
    let tool = host::create_tool(name, cwd, &controls);
    for identity in callbacks.into_iter().flatten() {
        identity.settle(tool.is_ok());
    }
    Ok(Rc::new(tool?))
}

/// Uses the callback table's existing rollback and release policy for each supplied effect.
fn stage(value: Option<impl Stored>) -> Option<Identity<Generated>> {
    value.map(|value| {
        let identity = Identity::new(&Generated);
        identity.stage(value);
        identity
    })
}

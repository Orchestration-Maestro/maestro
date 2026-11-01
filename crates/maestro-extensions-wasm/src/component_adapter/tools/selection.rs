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
    Ok(Rc::new(with_options(options, |controls| {
        host::create_tool(name, cwd, controls)
    })?))
}

/// Stages one set of options for either native factory-return shape.
fn with_options<T>(
    options: ToolsOptions,
    call: impl FnOnce(&host::ToolFactoryOptions<'_>) -> ExtensionResult<T>,
) -> ExtensionResult<T> {
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
    let tool = call(&controls);
    for identity in callbacks.into_iter().flatten() {
        identity.settle(tool.is_ok());
    }
    tool
}

/// Uses the callback table's existing rollback and release policy for each supplied effect.
fn stage(value: Option<impl Stored>) -> Option<Identity<Generated>> {
    value.map(|value| {
        let identity = Identity::new(&Generated);
        identity.stage(value);
        identity
    })
}

/// Keeps native insertion order while projecting context-free execution.
pub(crate) fn tools(
    preset: host::ToolPreset,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::Tool>> {
    with_options(options, |controls| {
        host::create_tool_preset(preset, cwd, controls)
    })?
    .into_iter()
    .map(|(name, tool)| {
        let name = crate::ToolName::from(name);
        let tool = Rc::new(tool);
        let prepare = (name == crate::ToolName::Edit).then(|| super::prepare(tool.clone()));
        let mut executable = super::executable(tool)?;
        executable.prepare_arguments = prepare;
        Ok((name, executable))
    })
    .collect()
}

/// Keeps native insertion order, model observation and preparation on each definition.
pub(crate) fn definitions(
    preset: host::ToolPreset,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::ToolDef>> {
    with_options(options, |controls| {
        host::create_tool_preset(preset, cwd, controls)
    })?
    .into_iter()
    .map(|(name, tool)| {
        let name = crate::ToolName::from(name);
        let tool = Rc::new(tool);
        let prepare = (name == crate::ToolName::Edit).then(|| super::prepare(tool.clone()));
        let mut definition = super::definition(tool, name == crate::ToolName::Read)?;
        definition.prepare_arguments = prepare;
        Ok((name, definition))
    })
    .collect()
}

/// Projects ordered vectors from the same native keyed return, without a membership list.
pub(crate) fn tool_list(
    preset: host::ToolPreset,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<Vec<crate::Tool>> {
    Ok(tools(preset, cwd, options)?.into_values().collect())
}

/// Projects ordered definition vectors without reconstructing preset membership.
pub(crate) fn definition_list(
    preset: host::ToolPreset,
    cwd: &str,
    options: ToolsOptions,
) -> ExtensionResult<Vec<crate::ToolDef>> {
    Ok(definitions(preset, cwd, options)?.into_values().collect())
}

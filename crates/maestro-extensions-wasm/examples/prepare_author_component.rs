//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components, warms the compilation cache of the shared
//! and context components and exports every component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

/// The controlled message provider component supplied to the tests.
const MESSAGE_COMPONENT: &str = "MAESTRO_CUSTOM_MESSAGE_PROVIDER_COMPONENT";

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let message = build::component("custom_message_provider", MESSAGE_COMPONENT)?;
    let bash_spawn_hook = build::component("bash_spawn_hook", "MAESTRO_BASH_SPAWN_HOOK_COMPONENT")?;
    let gateway = build::component("custom_gateway_provider", "MAESTRO_GATEWAY_COMPONENT")?;
    let summarize = build::component("summarize", "MAESTRO_SUMMARIZE_COMPONENT")?;
    let qna = build::component("qna", "MAESTRO_QNA_COMPONENT")?;
    let tool_override = build::component("tool_override", "MAESTRO_TOOL_OVERRIDE_COMPONENT")?;
    let handoff = build::component("handoff", "MAESTRO_HANDOFF_COMPONENT")?;
    let tools = build::component("tools", "MAESTRO_TOOLS_COMPONENT")?;
    let shell = build::component("local_shell_component", "MAESTRO_LOCAL_SHELL_COMPONENT")?;
    let native_io = build::component("native_io_component", "MAESTRO_NATIVE_IO_COMPONENT")?;
    let file_trigger = build::component("file_trigger", "MAESTRO_FILE_TRIGGER_COMPONENT")?;
    let checkpoint = build::component("git_checkpoint", "MAESTRO_GIT_CHECKPOINT_COMPONENT")?;
    let location = build::component("module_location_component", "MAESTRO_LOCATION_COMPONENT")?;
    let wasi = build::component("wasi_capabilities", "MAESTRO_WASI_COMPONENT")?;
    let helpers = build::component("helper_capabilities", "MAESTRO_HELPER_COMPONENT")?;
    let hooks = build::component("hook_component", "MAESTRO_HOOK_COMPONENT")?;
    let resources = build::component("resource_component", "MAESTRO_RESOURCE_COMPONENT")?;
    let reloaded = build::component(
        "resource_component_reloaded",
        "MAESTRO_RESOURCE_RELOADED_COMPONENT",
    )?;
    let truncated_tool = build::component("truncated_tool", "MAESTRO_TRUNCATED_TOOL_COMPONENT")?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(
            environment,
            format!(
                "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\nMAESTRO_GATEWAY_COMPONENT={}\n\
                 MAESTRO_LOCAL_SHELL_COMPONENT={}\nMAESTRO_NATIVE_IO_COMPONENT={}\nMAESTRO_FILE_TRIGGER_COMPONENT={}\nMAESTRO_LOCATION_COMPONENT={}\nMAESTRO_WASI_COMPONENT={}\nMAESTRO_HELPER_COMPONENT={}\nMAESTRO_HOOK_COMPONENT={}\nMAESTRO_RESOURCE_COMPONENT={}\nMAESTRO_RESOURCE_RELOADED_COMPONENT={}\nMAESTRO_TOOLS_COMPONENT={}\nMAESTRO_SUMMARIZE_COMPONENT={}\nMAESTRO_GIT_CHECKPOINT_COMPONENT={}\nMAESTRO_TOOL_OVERRIDE_COMPONENT={}\nMAESTRO_HANDOFF_COMPONENT={}\nMAESTRO_QNA_COMPONENT={}\nMAESTRO_BASH_SPAWN_HOOK_COMPONENT={}\n\
                 MAESTRO_TRUNCATED_TOOL_COMPONENT={}\nMAESTRO_CUSTOM_MESSAGE_PROVIDER_COMPONENT={}\n",
                path.display(),
                context.display(),
                gateway.display(),
                shell.display(),
                native_io.display(),
                file_trigger.display(),
                location.display(),
                wasi.display(),
                helpers.display(),
                hooks.display(),
                resources.display(),
                reloaded.display(),
                tools.display(),
                summarize.display(),
                checkpoint.display(),
                tool_override.display(),
                handoff.display(),
                qna.display(),
                bash_spawn_hook.display(),
                truncated_tool.display(),
                message.display()
            ),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

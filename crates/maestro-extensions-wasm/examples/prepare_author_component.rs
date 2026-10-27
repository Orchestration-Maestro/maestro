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

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let rg = pinned_search_tool("rg")?;
    let fd = pinned_search_tool("fd")?;
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
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
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(
            environment,
            format!(
                "MAESTRO_TEST_RG={rg}\nMAESTRO_TEST_FD={fd}\n\
                 MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n\
                 MAESTRO_LOCAL_SHELL_COMPONENT={}\nMAESTRO_NATIVE_IO_COMPONENT={}\nMAESTRO_FILE_TRIGGER_COMPONENT={}\nMAESTRO_LOCATION_COMPONENT={}\nMAESTRO_WASI_COMPONENT={}\nMAESTRO_HELPER_COMPONENT={}\nMAESTRO_HOOK_COMPONENT={}\nMAESTRO_RESOURCE_COMPONENT={}\nMAESTRO_RESOURCE_RELOADED_COMPONENT={}\nMAESTRO_TOOLS_COMPONENT={}\nMAESTRO_GIT_CHECKPOINT_COMPONENT={}\nMAESTRO_TOOL_OVERRIDE_COMPONENT={}\nMAESTRO_HANDOFF_COMPONENT={}\nMAESTRO_QNA_COMPONENT={}\n",
                path.display(),
                context.display(),
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
                checkpoint.display(),
                tool_override.display(),
                handoff.display(),
                qna.display()
            ),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Resolves a pinned development tool before tests replace their search PATH.
///
/// # Errors
///
/// Returns an error if mise cannot resolve the tool, its output is not UTF-8,
/// or the returned path is not absolute or does not name a file.
fn pinned_search_tool(name: &str) -> Result<String, String> {
    let output = std::process::Command::new("mise")
        .args(["which", name])
        .output()
        .map_err(|error| format!("{name}: cannot resolve pinned tool with mise: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{name}: mise could not resolve pinned tool: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let path = String::from_utf8(output.stdout).map_err(|error| format!("{name}: {error}"))?;
    let path = path.trim();
    if !std::path::Path::new(path).is_absolute() || !std::path::Path::new(path).is_file() {
        return Err(format!(
            "{name}: mise returned no absolute executable path: {path}"
        ));
    }
    Ok(path.to_owned())
}

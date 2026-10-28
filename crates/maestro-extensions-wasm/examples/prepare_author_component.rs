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

use std::fmt::Write as _;

#[path = "../src/tests/build.rs"]
mod build;

/// The component besides the shared author component whose compilation the tests share through
/// the cache.
const COMPILED: &str = "context_bridge";

// `just example-index` renders this table from the component markers in the example sources,
// and formatting keeps it as rendered.
#[rustfmt::skip]
// example-index:start
/// Each prepared example and the variable that hands its component path to the tests.
const COMPONENTS: &[(&str, &str)] = &[
    ("bash_spawn_hook", "MAESTRO_BASH_SPAWN_HOOK_COMPONENT"),
    ("context_bridge", "MAESTRO_CONTEXT_COMPONENT"),
    ("dynamic_resources", "MAESTRO_DYNAMIC_RESOURCES_COMPONENT"),
    ("file_trigger", "MAESTRO_FILE_TRIGGER_COMPONENT"),
    ("git_checkpoint", "MAESTRO_GIT_CHECKPOINT_COMPONENT"),
    ("handoff", "MAESTRO_HANDOFF_COMPONENT"),
    ("helper_capabilities", "MAESTRO_HELPER_COMPONENT"),
    ("hook_component", "MAESTRO_HOOK_COMPONENT"),
    ("local_shell_component", "MAESTRO_LOCAL_SHELL_COMPONENT"),
    ("module_location_component", "MAESTRO_LOCATION_COMPONENT"),
    ("native_io_component", "MAESTRO_NATIVE_IO_COMPONENT"),
    ("qna", "MAESTRO_QNA_COMPONENT"),
    ("resource_component", "MAESTRO_RESOURCE_COMPONENT"),
    ("resource_component_reloaded", "MAESTRO_RESOURCE_RELOADED_COMPONENT"),
    ("summarize", "MAESTRO_SUMMARIZE_COMPONENT"),
    ("tic_tac_toe", "MAESTRO_TIC_TAC_TOE_COMPONENT"),
    ("tool_override", "MAESTRO_TOOL_OVERRIDE_COMPONENT"),
    ("tools", "MAESTRO_TOOLS_COMPONENT"),
    ("truncated_tool", "MAESTRO_TRUNCATED_TOOL_COMPONENT"),
    ("wasi_capabilities", "MAESTRO_WASI_COMPONENT"),
];
// example-index:end

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let author = build::author_component()?;
    build::compile(author).map_err(|error| error.to_string())?;
    let mut environment = format!("MAESTRO_AUTHOR_COMPONENT={}\n", author.display());
    for &(example, variable) in COMPONENTS {
        let path = build::component(example, variable)?;
        if example == COMPILED {
            build::compile(&path).map_err(|error| error.to_string())?;
        }
        writeln!(environment, "{variable}={}", path.display())
            .map_err(|error| error.to_string())?;
    }
    if let Some(file) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(file, environment).map_err(|error| error.to_string())?;
    }
    Ok(())
}

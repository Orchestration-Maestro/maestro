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

#[cfg(not(target_arch = "wasm32"))]
#[path = "../src/tests/build.rs"]
mod build;

/// Preparation runs the native host, which a WebAssembly build does not have.
#[cfg(target_arch = "wasm32")]
fn main() {}

/// Builds and compiles the components, then hands their paths to nextest's environment file.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let location = build::component("module_location_component", "MAESTRO_LOCATION_COMPONENT")?;
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
                "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n\
                 MAESTRO_LOCATION_COMPONENT={}\nMAESTRO_HELPER_COMPONENT={}\nMAESTRO_HOOK_COMPONENT={}\nMAESTRO_RESOURCE_COMPONENT={}\nMAESTRO_RESOURCE_RELOADED_COMPONENT={}\n",
                path.display(),
                context.display(),
                location.display(),
                helpers.display(),
                hooks.display(),
                resources.display(),
                reloaded.display()
            ),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

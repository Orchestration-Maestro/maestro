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
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let location = build::component("module_location_component", "MAESTRO_LOCATION_COMPONENT")?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(
            environment,
            format!(
                "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n\
                 MAESTRO_LOCATION_COMPONENT={}\n",
                path.display(),
                context.display(),
                location.display()
            ),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

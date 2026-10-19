//! Prepares the author component before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the component
//! compiles no second host. It builds the component, warms its compilation cache and exports
//! the component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

/// Builds and compiles the component, then hands its path to nextest's environment file.
fn main() -> Result<(), String> {
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(
            environment,
            format!(
                "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n",
                path.display(),
                context.display()
            ),
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components, warms their compilation cache
//! and exports every component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

// The preparation module below reaches source discovery through this import.
use build::cargo::components;

#[path = "../../maestro-tooling/src/example_files/preparation.rs"]
mod example_files;

/// Builds the components, warms their compilation cache, then hands their paths to nextest's
/// environment file.
fn main() -> Result<(), String> {
    let path = build::author_component()?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut entries = format!(
        "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n",
        path.display(),
        context.display()
    );
    let mut components = vec![path.to_owned(), context];
    entries.push_str(
        &example_files::prepare(
            &[
                root.join("examples"),
                root.join("../../examples/extensions"),
            ],
            |name, _| {
                let path = build::example_component(name).map_err(std::io::Error::other)?;
                components.push(path.clone());
                Ok(path)
            },
        )
        .map_err(|error| error.to_string())?,
    );
    build::warm(&components)?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
    }
    Ok(())
}

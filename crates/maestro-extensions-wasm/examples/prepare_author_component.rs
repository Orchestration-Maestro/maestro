//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components, warms the compilation cache of the shared
//! and context components, compaction example and subagent package, then exports their paths.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

#[path = "../../maestro-tooling/src/example_files.rs"]
mod example_files;

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let compact = build::component("trigger_compact", "MAESTRO_TRIGGER_COMPACT_COMPONENT")?;
    build::compile(&compact).map_err(|error| error.to_string())?;
    let subagent = build::example_component("maestro-subagents", "component")?;
    build::compile(&subagent).map_err(|error| error.to_string())?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut entries = format!(
        "MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\nMAESTRO_TRIGGER_COMPACT_COMPONENT={}\nMAESTRO_SUBAGENT_COMPONENT={}\n",
        path.display(),
        context.display(),
        compact.display(),
        subagent.display()
    );
    entries.push_str(
        &example_files::prepare(
            &[
                root.join("examples"),
                root.join("../../examples/extensions"),
            ],
            |name, variable| build::component(name, variable).map_err(std::io::Error::other),
        )
        .map_err(|error| error.to_string())?,
    );
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
    }
    Ok(())
}

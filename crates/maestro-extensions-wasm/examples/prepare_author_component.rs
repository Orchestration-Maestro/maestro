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

#[path = "../../maestro-tooling/src/example_files.rs"]
mod example_files;

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let rg = pinned_search_tool("rg")?;
    let fd = pinned_search_tool("fd")?;
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut entries = format!(
        "MAESTRO_TEST_RG={rg}\nMAESTRO_TEST_FD={fd}\n\
         MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n",
        path.display(),
        context.display()
    );
    for folder in [
        root.join("examples"),
        root.join("../../examples/extensions"),
    ] {
        entries.push_str(
            &example_files::prepare(&folder, |name, variable| {
                build::component(name, variable).map_err(std::io::Error::other)
            })
            .map_err(|error| error.to_string())?,
        );
    }
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
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

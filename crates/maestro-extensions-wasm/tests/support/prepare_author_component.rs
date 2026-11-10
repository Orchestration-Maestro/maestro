//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components in one Cargo invocation, warms their
//! compilation cache and exports every component's path to the tests. With `--cache-directory`,
//! it writes the engine's cache directory below the agent directory as one JSON path and
//! performs no preparation.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[cfg(not(target_arch = "wasm32"))]
use std::fmt::Write as _;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../src/tests/build.rs"]
mod build;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../../maestro-tooling/src/search_tools.rs"]
mod search_tools;

// The preparation module below reaches source discovery through this import, and so does the
// listing of component names.
#[cfg(not(target_arch = "wasm32"))]
use build::cargo::components;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../../maestro-tooling/src/example_files/preparation.rs"]
mod example_files;

/// Preparation runs the native host, which a WebAssembly build does not have.
#[cfg(target_arch = "wasm32")]
fn main() {}

/// Reports the native engine cache directory or prepares the components for nextest.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [argument] if argument == "--cache-directory" => {
            let agent = maestro_test_process::run_agent_directory().ok_or_else(|| {
                format!(
                    "{} must name an absolute directory",
                    maestro_test_process::AGENT_DIRECTORY
                )
            })?;
            let cache = maestro_extensions_wasmtime::component_cache(&agent)
                .ok_or("the component cache directory cannot be used")?;
            println!(
                "{}",
                serde_json::to_string(cache.directory()).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        [root, component, package, fixed] => {
            let tools = search_tools::resolve(std::path::Path::new(root))?;
            prepare(root, component, package, fixed, tools)
        }
        [root, component, package, fixed, mode] if mode == "--prepared" => {
            prepare(root, component, package, fixed, supplied_tools()?)
        }
        _ => Err(
            "usage: prepare_author_component <workspace> <component crate> <component \
                  package> <fixed components>"
                .into(),
        ),
    }
}

/// Read the prepared search paths without resolving any program from PATH.
#[cfg(not(target_arch = "wasm32"))]
fn supplied_tools() -> Result<Vec<(&'static str, String)>, String> {
    search_tools::PROGRAMS
        .into_iter()
        .filter_map(|(variable, name, _)| {
            std::env::var_os(variable).map(|path| {
                search_tools::resolution::executable_text(name, std::path::Path::new(&path))
                    .map(|path| (variable, path))
            })
        })
        .collect()
}

/// The fixed components whose packages the workspace holds, from the names preparation passes.
///
/// # Errors
///
/// Returns a name that is not one of the fixed component targets.
#[cfg(not(target_arch = "wasm32"))]
fn fixed_components(names: &std::ffi::OsStr) -> Result<Vec<components::Fixed>, String> {
    let names = names
        .to_str()
        .ok_or("the fixed component names are not UTF-8")?;
    names
        .split(',')
        .filter(|name| !name.is_empty())
        .map(|name| {
            components::FIXED_COMPONENTS
                .iter()
                .find(|(target, _, _)| *target == name)
                .copied()
                .ok_or_else(|| format!("unknown fixed component {name}"))
        })
        .collect()
}

/// Builds the components, warms their cache and writes their paths to nextest's environment file.
#[cfg(not(target_arch = "wasm32"))]
fn prepare(
    workspace: &std::ffi::OsStr,
    component: &std::ffi::OsStr,
    package: &std::ffi::OsStr,
    fixed: &std::ffi::OsStr,
    tools: Vec<(&str, String)>,
) -> Result<(), String> {
    let workspace = std::path::Path::new(workspace);
    let root = std::path::Path::new(component);
    let package = package
        .to_str()
        .ok_or("the component package name is not UTF-8")?;
    let fixed = fixed_components(fixed)?;
    let folders = components::folders(workspace, root);
    let mut names: Vec<String> = fixed
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    components::visit(&folders, &fixed, |name, _| {
        names.push(name.to_owned());
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    let targets: Vec<&str> = names.iter().map(String::as_str).collect();
    let built = build::cargo::example_components(root, package, &targets)?;
    // Source discovery visits the markers in the order that produced the names above.
    let Some((supplied, marked)) = built.split_at_checked(fixed.len()) else {
        return Err("the build returned fewer components than requested".to_owned());
    };
    let mut entries = String::new();
    for (variable, path) in tools {
        writeln!(entries, "{variable}={path}").map_err(|error| error.to_string())?;
    }
    for ((_, _, variable), path) in fixed.iter().zip(supplied) {
        writeln!(entries, "{variable}={}", path.display()).map_err(|error| error.to_string())?;
    }
    let mut marked = marked.iter();
    entries.push_str(
        &example_files::prepare(&folders, &fixed, |_, _| {
            marked
                .next()
                .cloned()
                .ok_or_else(|| std::io::Error::other("a marked component has no build"))
        })
        .map_err(|error| error.to_string())?,
    );
    build::warm(&built)?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
    }
    Ok(())
}

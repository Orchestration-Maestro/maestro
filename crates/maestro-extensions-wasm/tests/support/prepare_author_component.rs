//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components in one Cargo invocation, warms their
//! compilation cache and exports every component's path to the tests. With `--cache-directory`,
//! it writes the engine's native cache directory as one JSON path and performs no preparation.
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
#[path = "../../src/tests/search_tools.rs"]
mod search_tools;

/// The fixture components of the component crate, relative to that crate.
#[cfg(not(target_arch = "wasm32"))]
const FIXTURES: &str = "tests/support";

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
            let cache = wasmtime::Cache::from_file(None).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(cache.directory()).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        [root, component, fixed] => prepare(root, component, fixed),
        _ => Err(
            "usage: prepare_author_component <workspace> <component crate> <fixed \
                  components>"
                .into(),
        ),
    }
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
    fixed: &std::ffi::OsStr,
) -> Result<(), String> {
    let workspace = std::path::Path::new(workspace);
    let root = std::path::Path::new(component);
    let fixed = fixed_components(fixed)?;
    let shipped = workspace.join("examples/extensions");
    let folders: Vec<std::path::PathBuf> = [
        Some(root.join(FIXTURES)),
        shipped.is_dir().then_some(shipped),
    ]
    .into_iter()
    .flatten()
    .collect();
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
    let built = build::cargo::example_components(root, &targets)?;
    // Source discovery visits the markers in the order that produced the names above.
    let Some((supplied, marked)) = built.split_at_checked(fixed.len()) else {
        return Err("the build returned fewer components than requested".to_owned());
    };
    let mut entries = search_tool_entries(workspace)?;
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

/// Names the pinned `rg` and `fd` executables the native search tests copy.
///
/// The pins belong to the workspace's own tool configuration, so a workspace without them has
/// no search tests to serve and exports nothing.
///
/// # Errors
///
/// Returns an error if a pin cannot be read or the search PATH holds no executable at that pin.
#[cfg(not(target_arch = "wasm32"))]
fn search_tool_entries(workspace: &std::path::Path) -> Result<String, String> {
    let config = workspace.join("mise.toml");
    if !config.is_file() {
        return Ok(String::new());
    }
    let config = config
        .to_str()
        .ok_or("the tool configuration path is not UTF-8")?;
    let rg_pin = mise_output(&["config", "get", "--file", config, "tools.ripgrep"])?;
    let fd_pin = mise_output(&["config", "get", "--file", config, "tools.fd"])?;
    let search = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    let rg = search_tools::pinned_search_tool("rg", "ripgrep", &rg_pin, &search)?;
    let fd = search_tools::pinned_search_tool("fd", "fd", &fd_pin, &search)?;
    Ok(format!("MAESTRO_TEST_RG={rg}\nMAESTRO_TEST_FD={fd}\n"))
}

/// Reads stored tool pins without evaluating project configuration.
///
/// # Errors
///
/// Returns an error if mise cannot run, exits unsuccessfully or returns invalid UTF-8.
#[cfg(not(target_arch = "wasm32"))]
fn mise_output(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("mise")
        // Reading stored pins needs no trust and must not evaluate project hooks.
        .env("MISE_NO_CONFIG", "1")
        .args(args)
        .output()
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "mise {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))
}

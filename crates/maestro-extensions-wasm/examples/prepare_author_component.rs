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
#[path = "../src/tests/build.rs"]
mod build;

// The preparation module below reaches source discovery through this import, and so does the
// listing of component names.
#[cfg(not(target_arch = "wasm32"))]
use build::cargo::components;

#[cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-tooling/src/example_files/preparation.rs"]
mod example_files;

/// Preparation runs the native host, which a WebAssembly build does not have.
#[cfg(target_arch = "wasm32")]
fn main() {}

/// Reports the native engine cache directory or prepares the components for nextest.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match arguments.as_slice() {
        [] => prepare(),
        [argument] if argument == "--cache-directory" => {
            let cache = wasmtime::Cache::from_file(None).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(cache.directory()).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        _ => Err("unknown component preparation arguments".into()),
    }
}

/// Builds the components, warms their cache and writes their paths to nextest's environment file.
#[cfg(not(target_arch = "wasm32"))]
fn prepare() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let folders = [
        root.join("examples"),
        root.join("../../examples/extensions"),
    ];
    let mut names: Vec<String> = components::FIXED_COMPONENTS
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    components::visit(&folders, |name, _| {
        names.push(name.to_owned());
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    let targets: Vec<&str> = names.iter().map(String::as_str).collect();
    let built = build::cargo::example_components(root, &targets)?;
    // Source discovery visits the markers in the order that produced the names above.
    let Some((fixed, marked)) = built.split_at_checked(components::FIXED_COMPONENTS.len()) else {
        return Err("the build returned fewer components than requested".to_owned());
    };
    let mut entries = String::new();
    for ((_, _, variable), path) in components::FIXED_COMPONENTS.iter().zip(fixed) {
        writeln!(entries, "{variable}={}", path.display()).map_err(|error| error.to_string())?;
    }
    let mut marked = marked.iter();
    entries.push_str(
        &example_files::prepare(&folders, |_, _| {
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

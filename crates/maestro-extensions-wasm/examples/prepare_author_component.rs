//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components in one Cargo invocation, warms their
//! compilation cache and exports every component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::fmt::Write as _;

#[path = "../src/tests/build.rs"]
mod build;

// The preparation module below reaches source discovery through this import, and so does the
// listing of component names.
use build::cargo::components;

#[path = "../../maestro-tooling/src/example_files/preparation.rs"]
mod example_files;

/// Builds the components in one Cargo invocation, warms their compilation cache, then hands their
/// paths to nextest's environment file.
fn main() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let folders = [
        root.join("examples"),
        root.join("../../examples/extensions"),
    ];
    let mut names = components::FIXED_EXAMPLES.map(str::to_owned).to_vec();
    components::visit(&folders, |name, _| {
        names.push(name.to_owned());
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    let targets: Vec<&str> = names.iter().map(String::as_str).collect();
    let built = build::cargo::example_components(root, &targets)?;
    // Source discovery visits the markers in the order that produced the names above.
    let Some((fixed, marked)) = built.split_at_checked(components::FIXED_VARIABLES.len()) else {
        return Err("the build returned fewer components than requested".to_owned());
    };
    let mut entries = String::new();
    for (variable, path) in components::FIXED_VARIABLES.iter().zip(fixed) {
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

//! Prepare the browser import witness and the independent extension-guide build before tests.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::inputs::Inputs;
use super::plan::Artifacts;
use super::{at, copy, digest, remove_directory, verify};
use crate::nextest_setup::browser_graph;

/// The two cross-target outputs exported to their native test consumers.
const VARIABLES: [&str; 2] = ["MAESTRO_BROWSER_IMPORT_CHECK", "MAESTRO_GUIDE_COMPONENT"];
/// The build-check receipt within the existing preparation bundle.
const RECEIPT: &str = "cargo-checks.json";
/// The independent author's manifest and lock inputs.
const GUIDE: &str = "crates/maestro-extensions-wasmtime/tests/fixtures/hello-extension";

/// Source fingerprints and staged bytes of the two real Cargo builds.
#[derive(Serialize, Deserialize)]
struct Prepared {
    /// The existing tool-input closure, including the checked examples and their locks.
    inputs: Inputs,
    /// Content digests indexed by the fixed export names.
    files: BTreeMap<String, String>,
}

/// Validate the complete pair before returning paths that nextest can export.
pub(crate) fn prepared(checkout: &Path, output: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let output = std::fs::canonicalize(output)?;
    if output.to_string_lossy().contains(['\r', '\n']) {
        return Err(at(&output, "the path must stay on one line"));
    }
    let path = output.join(RECEIPT);
    let record: Prepared = serde_json::from_slice(&std::fs::read(&path)?)?;
    record.inputs.verify(checkout, &output)?;
    if record.files.len() != VARIABLES.len() {
        return Err(at(&output, "incomplete prepared Cargo checks"));
    }
    VARIABLES
        .into_iter()
        .map(|variable| {
            let expected = record
                .files
                .get(variable)
                .ok_or_else(|| at(&output, format!("missing prepared Cargo check {variable}")))?;
            let path = output.join(format!("{variable}.wasm"));
            verify(&path, expected)?;
            Ok((variable.to_owned(), path))
        })
        .collect()
}

/// Reuse current outputs or perform both locked builds once in repository setup.
pub(crate) fn prepare(
    cargo: &Path,
    checkout: &Path,
    output: &Path,
    target: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    if let Ok(paths) = prepared(checkout, output) {
        return Ok(paths);
    }
    let inputs = Artifacts::read(cargo, checkout)?.tool;
    let project = target.join("guide-project");
    project_files(checkout, &project)?;
    let mut browser = command(cargo, checkout, target);
    browser.args([
        "--target",
        "wasm32-unknown-unknown",
        "-p",
        "maestro-models",
        "--example",
        "browser_import_check",
        "--profile",
        "dev",
    ]);
    let mut guide = command(cargo, &project, target);
    guide.args(["--target", "wasm32-wasip2", "--lib", "--profile", "release"]);
    let browser = build(browser, "browser_import_check")?;
    let guide = build(guide, "hello_extension")?;
    inputs.verify(checkout, output)?;
    std::fs::create_dir_all(output)?;
    let mut files = BTreeMap::new();
    for (variable, source) in VARIABLES.into_iter().zip([browser, guide]) {
        let staged = output.join(format!("{variable}.wasm"));
        copy(&source, &staged)?;
        files.insert(variable.to_owned(), digest(&std::fs::read(staged)?));
    }
    let record = Prepared { inputs, files };
    std::fs::write(output.join(RECEIPT), serde_json::to_vec(&record)?)?;
    prepared(checkout, output)
}

/// Create a clean independent author project from the checked manifest, lock and shipped source.
fn project_files(checkout: &Path, root: &Path) -> io::Result<()> {
    remove_directory(root)?;
    std::fs::create_dir_all(root.join("src"))?;
    let manifest = std::fs::read_to_string(checkout.join(GUIDE).join("Cargo.toml.in"))?;
    let manifest = manifest.replace(
        "\"../maestro/crates/maestro-extensions-wasm\"",
        &serde_json::to_string(&checkout.join("crates/maestro-extensions-wasm"))?,
    );
    std::fs::write(root.join("Cargo.toml"), manifest)?;
    for (source, destination) in [
        (format!("{GUIDE}/Cargo.lock"), "Cargo.lock"),
        ("examples/extensions/hello.rs".to_owned(), "src/lib.rs"),
        ("rust-toolchain.toml".to_owned(), "rust-toolchain.toml"),
    ] {
        copy(&checkout.join(source), &root.join(destination))?;
    }
    Ok(())
}

/// Use the browser setup's instrumentation policy for cross-target Cargo compilation.
fn command(cargo: &Path, directory: &Path, target: &Path) -> Command {
    let mut command = browser_graph::command(cargo, directory, target);
    command
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args(["build", "--locked", "--message-format=json", "--target-dir"])
        .arg(target);
    command
}

/// Require a successful build and use only the artifact path Cargo reports for the target.
fn build(mut command: Command, name: &str) -> io::Result<PathBuf> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{name} build failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    for line in output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let message: serde_json::Value = serde_json::from_slice(line)?;
        if message["reason"] != "compiler-artifact" || message["target"]["name"] != name {
            continue;
        }
        if let Some(files) = message["filenames"].as_array()
            && let Some(path) = files
                .iter()
                .filter_map(serde_json::Value::as_str)
                .find(|file| {
                    Path::new(file)
                        .extension()
                        .is_some_and(|extension| extension == "wasm")
                })
        {
            return Ok(path.into());
        }
    }
    Err(io::Error::other(format!(
        "Cargo reported no WebAssembly artifact for {name}"
    )))
}

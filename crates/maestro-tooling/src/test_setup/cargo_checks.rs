//! Prepare the browser import witness and the independent extension-guide build before tests.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::inputs::Inputs;
use super::plan::closure_directories;
use super::{SHARED_INPUTS, at, copy, digest, remove_directory, verify};
use crate::nextest_setup::browser_graph;
use crate::workspace::{GUIDE, Workspace};

/// The browser import witness exported to its native test consumer.
const BROWSER: &str = "MAESTRO_BROWSER_IMPORT_CHECK";
/// The independent author's guide component exported to its native test consumer.
const GUIDE_COMPONENT: &str = "MAESTRO_GUIDE_COMPONENT";
/// The build-check receipt within the existing preparation bundle.
const RECEIPT: &str = "cargo-checks.json";
/// The crate whose example is the browser import witness.
const MODELS: &str = "maestro-models";
/// The example that witnesses the browser import.
const BROWSER_EXAMPLE: &str = "browser_import_check";
/// The crate that supplies the guest SDK to the independent author's guide.
const GUEST: &str = "maestro-extensions-wasm";
/// The shipped extension the independent author's guide builds.
const GUIDE_SOURCE: &str = "examples/extensions/hello.rs";

/// Source fingerprints and staged bytes of the two real Cargo builds.
#[derive(Serialize, Deserialize)]
struct Prepared {
    /// The checked packages' input closure, shared configuration and independent guide sources.
    inputs: Inputs,
    /// Content digests indexed by the fixed export names.
    files: BTreeMap<String, String>,
}

/// Validate the complete pair before returning paths that nextest can export.
pub(crate) fn prepared(checkout: &Path, output: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let output = maestro_realpath::canonicalize(output)?;
    if output.to_string_lossy().contains(['\r', '\n']) {
        return Err(at(&output, "the path must stay on one line"));
    }
    let path = output.join(RECEIPT);
    let record: Prepared =
        serde_json::from_slice(&std::fs::read(&path).map_err(|error| at(&path, error))?)
            .map_err(|error| at(&path, error))?;
    record.inputs.verify(checkout, &output)?;
    record
        .files
        .iter()
        .map(|(variable, expected)| {
            let path = output.join(format!("{variable}.wasm"));
            verify(&path, expected)?;
            Ok((variable.clone(), path))
        })
        .collect()
}

/// Reuse current outputs or perform the locked builds once in repository setup.
///
/// A check whose package or input this workspace does not hold is skipped, so a workspace
/// that builds on Maestro prepares the checks it has and no others.
pub(crate) fn prepare(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    output: &Path,
    target: &Path,
) -> io::Result<Vec<(String, PathBuf)>> {
    if let Ok(paths) = prepared(checkout, output) {
        return Ok(paths);
    }
    let inputs = inputs(workspace)?;
    let checks = checks(cargo, checkout, workspace, target)?;
    let built = checks
        .into_iter()
        .map(|(variable, command, name)| Ok((variable, build(command, name)?)))
        .collect::<io::Result<Vec<_>>>()?;
    inputs.verify(checkout, output)?;
    std::fs::create_dir_all(output)?;
    let mut files = BTreeMap::new();
    for (variable, source) in built {
        let staged = output.join(format!("{variable}.wasm"));
        copy(&source, &staged)?;
        files.insert(variable.to_owned(), digest(&std::fs::read(staged)?));
    }
    let record = Prepared { inputs, files };
    std::fs::write(output.join(RECEIPT), serde_json::to_vec(&record)?)?;
    prepared(checkout, output)
}

/// The builds this workspace runs, as each export name, its command and its artifact name.
fn checks(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    target: &Path,
) -> io::Result<Vec<(&'static str, Command, &'static str)>> {
    let mut checks = Vec::new();
    if workspace.member(MODELS).is_some() {
        let mut browser = command(cargo, checkout, target);
        browser.args([
            "--target",
            "wasm32-unknown-unknown",
            "-p",
            MODELS,
            "--example",
            BROWSER_EXAMPLE,
            "--profile",
            "dev",
        ]);
        checks.push((BROWSER, browser, BROWSER_EXAMPLE));
    }
    if let (Some(guide), Some(guest)) = (workspace.guide(), workspace.package(GUEST)?) {
        let project = target.join("guide-project");
        project_files(checkout, &project, &guide, &guest)?;
        let mut author = command(cargo, &project, target);
        author.args(["--target", "wasm32-wasip2", "--lib", "--profile", "release"]);
        checks.push((GUIDE_COMPONENT, author, "hello_extension"));
    }
    Ok(checks)
}

/// Fingerprint the checked package closures and the independent guide's authored inputs.
fn inputs(workspace: &Workspace) -> io::Result<Inputs> {
    let named = workspace.holds(&[MODELS, GUEST]);
    let mut paths = if named.is_empty() {
        BTreeSet::new()
    } else {
        closure_directories(workspace.metadata(), &named)?
    };
    paths.extend(
        SHARED_INPUTS
            .into_iter()
            .chain(["Cargo.lock"])
            .map(str::to_owned),
    );
    if let Some(guide) = workspace.guide() {
        paths.insert(format!("{}/{GUIDE}", workspace.relative(&guide)?));
        paths.insert(GUIDE_SOURCE.to_owned());
    }
    Inputs::new(workspace.root(), paths, BTreeSet::new(), BTreeSet::new())
}

/// Create a clean independent author project from the checked manifest, lock and shipped source.
fn project_files(checkout: &Path, root: &Path, guide: &Path, guest: &Path) -> io::Result<()> {
    remove_directory(root)?;
    std::fs::create_dir_all(root.join("src"))?;
    let fixture = guide.join(GUIDE);
    let manifest = std::fs::read_to_string(fixture.join("Cargo.toml.in"))?;
    let manifest = manifest.replace(
        "\"../maestro/crates/maestro-extensions-wasm\"",
        &serde_json::to_string(guest)?,
    );
    std::fs::write(root.join("Cargo.toml"), manifest)?;
    for (source, destination) in [
        (fixture.join("Cargo.lock"), "Cargo.lock"),
        (checkout.join(GUIDE_SOURCE), "src/lib.rs"),
        (checkout.join("rust-toolchain.toml"), "rust-toolchain.toml"),
    ] {
        copy(&source, &root.join(destination))?;
    }
    Ok(())
}

/// Use the browser setup's instrumentation policy for cross-target Cargo compilation.
fn command(cargo: &Path, directory: &Path, target: &Path) -> Command {
    let mut command = browser_graph::command(cargo, directory, target);
    command
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .args([
            "build",
            "--locked",
            "--message-format=json-render-diagnostics",
            "--target-dir",
        ])
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Failed Cargo checks retain the compiler's code and offending import in their error.
    #[test]
    fn failed_cargo_check_reports_compiler_diagnostics() {
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\n[package]\nname = \"broken-import\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[lib]\npath = \"lib.rs\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.lock"),
            "version = 4\n[[package]]\nname = \"broken-import\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        std::fs::write(root.join("lib.rs"), "use std::maestro_missing_import;\n").unwrap();
        let mut cargo = command(Path::new(env!("CARGO")), root, &root.join("target"));
        cargo.args(["--lib", "--offline"]);

        let error = build(cargo, "broken_import").unwrap_err().to_string();

        assert!(error.contains("E0432"), "{error}");
        assert!(error.contains("maestro_missing_import"), "{error}");
    }
}

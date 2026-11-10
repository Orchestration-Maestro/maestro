//! Build the in-frame sandbox runtime with Emscripten and audit what it links.
//!
//! The runtime package builds as one self-starting classic script. The command
//! writes that script under `target/`, where the web crate's build script finds
//! and embeds it; the script is never committed. It refuses output that the audit
//! rejects: a linked library outside the audited list, or script text an HTML
//! document cannot carry verbatim.

use std::{
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

/// Emscripten release the bundle is built and audited with.
const EMSCRIPTEN: &str = "6.0.10";
/// Binding generator release the runtime's locked bindings require.
const BINDGEN: &str = "0.2.129";
/// The runtime package manifest, relative to the checkout.
pub(crate) const MANIFEST: &str = "crates/maestro-web/runtime/Cargo.toml";
/// The runtime binary the package builds.
const BINARY: &str = "maestro-sandbox-runtime";
/// The generated script, relative to the checkout. The web crate's build script names the same file.
pub(crate) const ASSET: &str = "target/sandbox-runtime/maestro-sandbox-runtime.js";
/// The workspace member whose build script embeds the generated script.
pub(crate) const OWNER: &str = "maestro-web";
/// Rust target the runtime is compiled for.
pub(crate) const TARGET: &str = "wasm32-unknown-emscripten";

/// Generate the asset for the web crate to embed.
///
/// # Errors
/// Returns a missing or wrong tool, a failed build or a rejected audit.
pub(super) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    if !args.is_empty() {
        return Err(io::Error::other("usage: sandbox-runtime"));
    }
    tool("emcc", EMSCRIPTEN)?;
    tool("wasm-bindgen", BINDGEN)?;
    let directory = checkout.join("target/sandbox-runtime");
    fs::create_dir_all(&directory)?;
    let (script, map) = build(cargo, checkout, &directory)?;
    let bytes = scoped(&fs::read(&script)?);
    audit::script(&bytes).map_err(io::Error::other)?;
    let libraries = audit::libraries(&fs::read_to_string(&map)?).map_err(io::Error::other)?;
    println!("LINKED {}", libraries.join(" "));
    publish(&checkout.join(ASSET), &bytes)?;
    Ok(ExitCode::SUCCESS)
}

/// Write the script unless the file already holds these bytes, so an unchanged build leaves the
/// web crate's embedded input alone.
fn publish(asset: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(asset).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    if let Some(parent) = asset.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(asset, bytes)
}

/// Require a tool on the path to report exactly the pinned release.
fn tool(name: &str, release: &str) -> io::Result<()> {
    let output = Command::new(name)
        .arg("--version")
        .output()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => {
                io::Error::other(format!("{name} release {release} is not on the PATH"))
            }
            _ => error,
        })?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next().unwrap_or_default();
    if output.status.success() && first.split_whitespace().any(|word| word == release) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{name} must be release {release}, found: {first}"
        )))
    }
}

/// The compiler flags of the runtime build, as Cargo's separator-delimited encoded list.
///
/// Cargo splits the plain flags variables at whitespace; the encoded form keeps a checkout
/// path that contains spaces in one argument. Each of `dependencies` pairs a package's source
/// root with the name it is recorded under, so no builder's Cargo home reaches the script.
/// The package's build script supplies the Emscripten link settings; only the linker map,
/// which the audit reads, is added here.
fn rustflags(checkout: &Path, map: &Path, dependencies: &[(String, String)]) -> String {
    let mut flags = vec![
        "-Cpanic=abort".to_owned(),
        "-Cllvm-args=-enable-emscripten-cxx-exceptions=0".to_owned(),
        "-Crelocation-model=static".to_owned(),
        format!("--remap-path-prefix={}=/checkout", checkout.display()),
    ];
    for (root, name) in dependencies {
        flags.push(format!("--remap-path-prefix={root}={name}"));
    }
    flags.push(format!("-Clink-arg=-Wl,-Map={}", map.display()));
    flags.join("\x1f")
}

/// Wrap the generated script in a function so its internals stay out of the page's global scope.
///
/// The function receives the page's `Module`, if any, as its program arguments, so a script
/// that precedes the bundle can still pass them. Authored scripts that follow it may declare
/// every name the generated code uses.
fn scoped(script: &[u8]) -> Vec<u8> {
    let mut wrapped = b"(function(Module){".to_vec();
    wrapped.extend_from_slice(script);
    wrapped.extend_from_slice(b"\n})(typeof Module==\"undefined\"?void 0:Module);");
    wrapped
}

/// Each dependency package's source root, paired with `<name>-<version>`.
///
/// That is the name Cargo's path trimming gives a registry package, and it is the same on
/// every machine, while the root sits under the builder's Cargo home.
fn dependencies(cargo: &Path, checkout: &Path) -> io::Result<Vec<(String, String)>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo)
        .current_dir(checkout)
        .manifest_path(checkout.join(MANIFEST))
        .other_options(vec!["--locked".into()])
        .exec()
        .map_err(io::Error::other)?;
    Ok(metadata
        .packages
        .into_iter()
        .filter(|package| package.source.is_some())
        .filter_map(|package| {
            let root = package.manifest_path.parent()?.to_string();
            Some((root, format!("{}-{}", package.name, package.version)))
        })
        .collect())
}

/// Compile the runtime and return the generated script and the linker map.
fn build(cargo: &Path, checkout: &Path, directory: &Path) -> io::Result<(PathBuf, PathBuf)> {
    let map = directory.join("runtime.map");
    let flags = rustflags(checkout, &map, &dependencies(cargo, checkout)?);
    let output = Command::new(cargo)
        .current_dir(checkout)
        .env("CARGO_TARGET_DIR", directory)
        .env("CARGO_ENCODED_RUSTFLAGS", flags)
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            TARGET,
            "--bin",
            BINARY,
        ])
        .args(["--manifest-path", MANIFEST, "--message-format=json"])
        .stderr(Stdio::inherit())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "runtime build failed ({})",
            output.status
        )));
    }
    let text = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    let executable = text
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|message| message["executable"].as_str().map(PathBuf::from))
        .ok_or_else(|| io::Error::other("the build reported no executable"))?;
    Ok((executable.with_extension("js"), map))
}

mod audit;
#[cfg(test)]
mod tests;

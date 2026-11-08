//! Build the in-frame sandbox runtime with Emscripten and audit what it links.
//!
//! The runtime package builds as one self-starting classic script. `generate`
//! writes that script to the asset the web crate embeds, and `--check` compares
//! it byte for byte. Both refuse output that the audit rejects: a linked
//! library outside the audited list, or script text an HTML document cannot
//! carry verbatim.

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
const MANIFEST: &str = "crates/maestro-web/runtime/Cargo.toml";
/// The runtime binary the package builds.
const BINARY: &str = "maestro-sandbox-runtime";
/// The generated script, relative to the checkout.
pub(crate) const ASSET: &str = "crates/maestro-web/assets/maestro-sandbox-runtime.js";
/// Rust target the runtime is compiled for.
const TARGET: &str = "wasm32-unknown-emscripten";
/// The allocator setting and the one archive that carries it, audited together.
///
/// The binding integration runs its start function before the static constructors,
/// so an allocator that initialises itself in a constructor would be used before it
/// is ready. This one initialises on first use.
pub(crate) const ALLOCATOR: (&str, &str) = ("-sMALLOC=dlmalloc", "libdlmalloc.a");
/// Linker settings that make the script start itself without help from the page.
const LINK_SETTINGS: [&str; 9] = [
    "-sWASM_BINDGEN",
    "-sSINGLE_FILE=1",
    "-sSINGLE_FILE_BINARY_ENCODE=0",
    "-sWASM_ASYNC_COMPILATION=0",
    "-sDYNAMIC_EXECUTION=0",
    "-sENVIRONMENT=web",
    ALLOCATOR.0,
    "-Wno-experimental",
    "-Wl,-Map",
];

/// Generate the asset, or with `--check` compare it with a fresh build.
///
/// # Errors
/// Returns a missing or wrong tool, a failed build, a rejected audit or a stale asset.
pub(super) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err(io::Error::other("usage: sandbox-runtime [--check]")),
    };
    tool("emcc", EMSCRIPTEN)?;
    tool("wasm-bindgen", BINDGEN)?;
    let directory = checkout.join("target/sandbox-runtime");
    fs::create_dir_all(&directory)?;
    let (script, map) = build(cargo, checkout, &directory)?;
    let bytes = scoped(&fs::read(&script)?);
    audit::script(&bytes).map_err(io::Error::other)?;
    let libraries = audit::libraries(&fs::read_to_string(&map)?).map_err(io::Error::other)?;
    println!("LINKED {}", libraries.join(" "));
    let asset = checkout.join(ASSET);
    if check {
        if fs::read(&asset)? != bytes {
            return Err(io::Error::other(format!("{} is stale", asset.display())));
        }
    } else {
        fs::write(&asset, &bytes)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Require a tool on the path to report exactly the pinned release.
fn tool(name: &str, release: &str) -> io::Result<()> {
    let output = Command::new(name).arg("--version").output()?;
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
/// path that contains spaces in one argument.
fn rustflags(checkout: &Path, map: &Path) -> String {
    let mut flags = vec![
        "-Cpanic=abort".to_owned(),
        "-Cllvm-args=-enable-emscripten-cxx-exceptions=0".to_owned(),
        "-Crelocation-model=static".to_owned(),
        format!("--remap-path-prefix={}=/checkout", checkout.display()),
    ];
    for setting in LINK_SETTINGS {
        let setting = if setting == "-Wl,-Map" {
            format!("-Wl,-Map={}", map.display())
        } else {
            setting.to_owned()
        };
        flags.push(format!("-Clink-arg={setting}"));
    }
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

/// Compile the runtime and return the generated script and the linker map.
fn build(cargo: &Path, checkout: &Path, directory: &Path) -> io::Result<(PathBuf, PathBuf)> {
    let map = directory.join("runtime.map");
    let flags = rustflags(checkout, &map);
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

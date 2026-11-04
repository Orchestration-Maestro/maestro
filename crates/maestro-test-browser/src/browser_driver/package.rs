//! Build browser exports from the checkout's exact locked inputs.

use super::inputs;
use maestro_tooling::copy_assets;
use sha2::{Digest as _, Sha256};
use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// An exclusive working tree removed on success, failure or early return.
pub(super) struct Work {
    /// Native path allocated inside Cargo's target directory.
    pub path: PathBuf,
}

impl Work {
    /// Reserve one invocation's output without touching another process's tree.
    pub fn new(target: &Path) -> io::Result<Self> {
        fs::create_dir_all(target)?;
        let path = target.join(format!("browser-driver-{}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Work {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("browser working directory {}: {error}", self.path.display());
        }
    }
}

/// Capture a native tool's stdout while preserving its diagnostics and exit status.
pub(super) fn command(command: &mut Command) -> io::Result<String> {
    println!(
        "COMMAND {} {}",
        command.get_program().to_string_lossy(),
        command
            .get_args()
            .map(|value| value.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let output = command.stderr(Stdio::inherit()).output()?;
    let text = String::from_utf8(output.stdout).map_err(io::Error::other)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{} failed ({}): {text}",
            command.get_program().to_string_lossy(),
            output.status
        )));
    }
    Ok(text)
}

/// Validate absolute supplied Chromium plus the binding CLI against the lock.
pub(super) fn tools(executable: &Path, checkout: &Path) -> io::Result<()> {
    if !executable.is_absolute() || !executable.is_file() {
        return Err(io::Error::other(
            "CHROMIUM_PATH must be an absolute executable file",
        ));
    }
    let cli = command(Command::new("wasm-bindgen").arg("--version"))?;
    setup(
        checkout,
        Path::new(env!("CARGO")),
        "binding-version",
        cli.trim().as_ref(),
    )?;
    let browser = command(Command::new(executable).arg("--version"))?;
    inputs::chromium_version(&browser)?;
    println!(
        "TOOLS chromiumoxide 0.9.1 {} {}",
        browser.trim(),
        cli.trim()
    );
    Ok(())
}

/// Compile the registered integration target and its independently instantiated runtime library.
pub(super) fn build(
    checkout: &Path,
    cargo: &Path,
    target: &Path,
    work: &Path,
) -> io::Result<PathBuf> {
    check_integration(checkout, cargo)?;
    let scratch = work.join("library");
    setup(checkout, cargo, "prepare", scratch.as_os_str())?;
    command(
        Command::new(cargo)
            .current_dir(checkout)
            .args(["fetch", "--locked"]),
    )?;
    command(
        Command::new(cargo)
            .current_dir(&scratch)
            .env("CARGO_TARGET_DIR", target)
            .args(["metadata", "--offline", "--format-version=1"]),
    )?;
    setup(checkout, cargo, "check-lock", scratch.as_os_str())?;
    let built = command(
        Command::new(cargo)
            .current_dir(&scratch)
            .env("CARGO_TARGET_DIR", target)
            .args([
                "rustc",
                "--lib",
                "--target",
                "wasm32-unknown-unknown",
                "--offline",
                "--locked",
                "--message-format=json",
                "--",
                "--cfg",
                "test",
            ]),
    )?;
    let runtime = inputs::artifact(&built, "cdylib", "maestro_browser_test")?;
    let output = work.join("host");
    fs::create_dir(&output)?;
    command(
        Command::new("wasm-bindgen")
            .arg(runtime)
            .args(["--target", "web", "--split-linked-modules", "--out-dir"])
            .arg(&output)
            .args(["--out-name", "sandbox"]),
    )?;
    materialize(&output)?;
    modules(checkout, cargo, target, &output)?;
    Ok(output)
}

/// Reuse the repository preparation owner with its unchanged command operands.
fn setup(checkout: &Path, cargo: &Path, operation: &str, value: &OsStr) -> io::Result<()> {
    maestro_tooling::run(
        &[
            "browser-export-setup".into(),
            operation.into(),
            value.into(),
        ],
        cargo,
        checkout,
    )
    .map(|_| ())
}

/// Compile the registered browser target without executing its native test harness.
fn check_integration(checkout: &Path, cargo: &Path) -> io::Result<()> {
    let compiled = command(Command::new(cargo).current_dir(checkout).args([
        "test",
        "-p",
        "maestro-web",
        "--test",
        "integration",
        "--target",
        "wasm32-unknown-unknown",
        "--locked",
        "--no-run",
        "--message-format=json",
    ]))?;
    println!(
        "ARTIFACT {}",
        inputs::artifact(&compiled, "test", "integration")?
    );
    Ok(())
}

/// Keep both separately locked module instances and their own binding glue.
fn modules(checkout: &Path, cargo: &Path, target: &Path, output: &Path) -> io::Result<()> {
    let fixtures = checkout.join("crates/maestro-web/tests/fixtures/browser_modules");
    copy_assets::copy_tree(&fixtures.join("package"), &output.join("package"))?;
    for name in ["a", "b"] {
        let built = command(
            Command::new(cargo)
                .current_dir(checkout)
                .env("CARGO_TARGET_DIR", target)
                .args(["build", "--manifest-path"])
                .arg(fixtures.join(format!("module_{name}/Cargo.toml")))
                .args([
                    "--lib",
                    "--release",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--locked",
                    "--message-format=json",
                ]),
        )?;
        let wasm = inputs::artifact(&built, "cdylib", &format!("contribution_module_{name}"))?;
        command(
            Command::new("wasm-bindgen")
                .arg(wasm)
                .args(["--target", "web", "--out-dir"])
                .arg(output.join(format!("package/modules/{name}")))
                .args(["--out-name", "module"]),
        )?;
    }
    sandbox_runtime(checkout, cargo, target, output)
}

/// Build the prototype sandbox runtime in release with classic-script glue for
/// parser-blocking startup.
fn sandbox_runtime(checkout: &Path, cargo: &Path, target: &Path, output: &Path) -> io::Result<()> {
    let bundle = output.join("package/modules/sandbox_runtime");
    let manifest = checkout
        .join("crates/maestro-web/tests/fixtures/browser_modules/sandbox_runtime/Cargo.toml");
    let built = command(
        Command::new(cargo)
            .current_dir(checkout)
            .env("CARGO_TARGET_DIR", target)
            .args(["build", "--manifest-path"])
            .arg(manifest)
            .args([
                "--lib",
                "--release",
                "--target",
                "wasm32-unknown-unknown",
                "--locked",
                "--message-format=json",
            ]),
    )?;
    let wasm = inputs::artifact(&built, "cdylib", "sandbox_runtime_probe")?;
    command(
        Command::new("wasm-bindgen")
            .arg(wasm)
            .args(["--target", "no-modules", "--no-modules-global"])
            .arg("__maestroSandboxRuntime")
            .args(["--no-typescript", "--out-dir"])
            .arg(&bundle)
            .args(["--out-name", "runtime"]),
    )?;
    for name in ["runtime.js", "runtime_bg.wasm"] {
        let bytes = fs::read(bundle.join(name))?;
        println!(
            "RECEIPT sandbox-runtime {name} bytes={} sha256={}",
            bytes.len(),
            maestro_tooling::hex(&Sha256::digest(&bytes))
        );
    }
    Ok(())
}

/// Write only the fixed bootstrap documents and helper module.
pub(super) fn materialize(root: &Path) -> io::Result<()> {
    fs::write(root.join("index.html"), "<!doctype html><body></body>")?;
    fs::write(root.join("helper.js"), "export const answer='helper-ok'")?;
    fs::write(
        root.join("sandbox.html"),
        "<script>addEventListener('message',e=>{if(e.data?.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();parent.postMessage({type:'sandbox-installed'},'*');}});parent.postMessage({type:'sandbox-ready'},'*');</script>",
    )?;
    Ok(())
}

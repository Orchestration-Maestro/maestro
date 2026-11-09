//! Build browser exports from the checkout's exact locked inputs.

use super::inputs;
use maestro_tooling::copy_assets;
use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Replace the previous run's export package with an empty working tree in Cargo's target
/// directory, where the run's tests read the new package.
pub(super) fn directory(target: &Path) -> io::Result<PathBuf> {
    let path = target.join("browser-exports");
    if let Err(error) = fs::remove_dir_all(&path)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(error);
    }
    fs::create_dir_all(&path)?;
    Ok(path)
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
    Ok(())
}

/// Write only the fixed bootstrap documents and helper module.
pub(super) fn materialize(root: &Path) -> io::Result<()> {
    fs::write(root.join("index.html"), "<!doctype html><body></body>")?;
    // Canvas exports read canvas points as page coordinates, so the canvas sits at the origin.
    fs::write(
        root.join("canvas.html"),
        "<!doctype html><body style=\"margin:0\"><canvas id=\"canvas\" width=\"800\" height=\"600\"></canvas></body>",
    )?;
    fs::write(root.join("helper.js"), "export const answer='helper-ok'")?;
    fs::write(
        root.join("sandbox.html"),
        "<script>addEventListener('message',e=>{if(e.source===parent&&e.data?.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();parent.postMessage({type:'sandbox-installed'},'*');}});parent.postMessage({type:'sandbox-ready'},'*');</script>",
    )?;
    Ok(())
}

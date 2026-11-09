//! The toolchain a workspace's Cargo children compile with.
//!
//! A toolchain's own `cargo` binary, started directly, carries no `RUSTUP_TOOLCHAIN`, because only
//! the rustup proxy sets it. Its `rustc` calls then go through the proxy, which picks a toolchain
//! for the directory each package compiles in: a dependency's shipped `rust-toolchain` file, or
//! the machine's default, would win over the workspace's pin. Every child started here names the
//! workspace's toolchain instead.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

/// The variable through which rustup selects a toolchain for the processes it starts.
const SELECTOR: &str = "RUSTUP_TOOLCHAIN";

/// A command for `program` that runs in `directory` on the toolchain active there.
///
/// A selection the process already inherited stands, because rustup ranks it above the files in
/// `directory`. Without rustup, or when it names no active toolchain, the command runs as Cargo
/// would run it alone.
pub(crate) fn command(program: impl AsRef<OsStr>, directory: &Path) -> Command {
    let mut command = Command::new(program);
    command.current_dir(directory);
    if std::env::var_os(SELECTOR).is_none()
        && let Some(toolchain) = active(directory)
    {
        command.env(SELECTOR, toolchain);
    }
    command
}

/// The name of the toolchain rustup activates in `directory`.
fn active(directory: &Path) -> Option<String> {
    let output = Command::new("rustup")
        .current_dir(directory)
        .args(["show", "active-toolchain"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .split_whitespace()
        .next()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    use maestro_test_directory::Scratch;
    use maestro_test_process::TestProcess;

    /// Write a crate named `name` whose library returns one.
    fn library(directory: &Path, name: &str, manifest_tail: &str) {
        std::fs::create_dir_all(directory.join("src")).unwrap();
        std::fs::write(
            directory.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n{manifest_tail}"),
        )
        .unwrap();
        std::fs::write(
            directory.join("src/lib.rs"),
            "pub fn value() -> u32 { 1 }\n",
        )
        .unwrap();
    }

    /// A workspace pinned to the toolchain `pin` depends on a crate outside it that ships a
    /// `rust-toolchain` file naming a toolchain that is not installed. Cargo compiles that crate
    /// in its own directory, where rustup reads the file.
    fn workspace_with_a_dependency_that_ships_a_toolchain(
        root: &Path,
        pin: &str,
    ) -> std::path::PathBuf {
        let dependency = root.join("dependency");
        library(&dependency, "dependency", "");
        std::fs::write(
            dependency.join("rust-toolchain"),
            "shipped-by-the-dependency\n",
        )
        .unwrap();
        let workspace = root.join("workspace");
        library(
            &workspace,
            "pinned",
            "[dependencies]\ndependency = { path = \"../dependency\" }\n[workspace]\n",
        );
        std::fs::write(
            workspace.join("rust-toolchain.toml"),
            format!("[toolchain]\nchannel = \"{pin}\"\n"),
        )
        .unwrap();
        workspace
    }

    /// A Cargo started directly, with no inherited selection, still compiles a dependency that
    /// ships its own toolchain file on the workspace's pin.
    #[test]
    fn a_dependency_shipping_a_toolchain_file_compiles_on_the_workspace_pin() {
        let output = TestProcess::child(compile_with_a_dependency_that_ships_a_toolchain)
            .unwrap()
            .env_remove(SELECTOR)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// The controlled child of the test above: it starts without an inherited selection.
    #[test]
    #[ignore = "runs only inside a controlled child"]
    fn compile_with_a_dependency_that_ships_a_toolchain() {
        let pin =
            maestro_test_process::toolchain::channel().expect("the build records its toolchain");
        let scratch = Scratch::new().unwrap();
        let workspace = workspace_with_a_dependency_that_ships_a_toolchain(scratch.path(), pin);

        let output = command(env!("CARGO"), &workspace)
            .args(["build", "--offline"])
            .env("CARGO_TARGET_DIR", scratch.path().join("target"))
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

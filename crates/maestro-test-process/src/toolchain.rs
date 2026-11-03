//! Controlled toolchain declarations for child builds.

use std::{ffi::OsStr, process::Command};

/// Strips runner instrumentation and selects the recorded build toolchain when present.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    if std::env::var_os("__CARGO_LLVM_COV_RUSTC_WRAPPER").is_some() {
        if let Some(wrapper) = std::env::var_os("__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING") {
            command.env("RUSTC_WRAPPER", wrapper);
        } else {
            command.env_remove("RUSTC_WRAPPER");
        }
    }
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .starts_with("__CARGO_LLVM_COV_RUSTC_WRAPPER")
        {
            command.env_remove(key);
        }
    }
    for key in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "LLVM_PROFILE_FILE",
        "CARGO_LLVM_COV",
        "CARGO_LLVM_COV_TARGET_DIR",
        "CARGO_LLVM_COV_BUILD_DIR",
        "CARGO_LLVM_COV_SHOW_ENV",
    ] {
        command.env_remove(key);
    }
    if let Some(channel) = channel() {
        command.env("RUSTUP_TOOLCHAIN", channel);
    }
    command
}

/// The private Cargo home of a fixture rooted at `root`.
#[must_use]
pub fn home(root: &std::path::Path) -> std::path::PathBuf {
    root.join("cargo-home")
}

/// Keep a fixture compiler's Cargo state inside its supplied workspace.
///
/// On Unix, an inherited `RUSTC_WRAPPER`, or failing that `CARGO_BUILD_RUSTC_WRAPPER`, runs through
/// the prepared wrapper alias in `root` that drops `CARGO_HOME` and `CARGO_TARGET_DIR` first. The
/// compiler cache hashes every `CARGO_*` variable, so a private home or output directory the
/// wrapper could see would give each fixture's registry crates a key of their own. Cargo passes
/// the compiler its output directory explicitly, so the variable is not needed there.
///
/// # Errors
/// Returns an error when creating the private home, copying its source configuration or
/// installing the wrapper alias fails. Installing needs the prepared `native_fixture` artifact
/// that nextest's `native-fixture` setup exports.
pub fn fixture_command(
    program: impl AsRef<OsStr>,
    root: &std::path::Path,
    sources: Option<&std::path::Path>,
) -> std::io::Result<Command> {
    let home = home(root);
    std::fs::create_dir_all(&home)?;
    if let Some(sources) = sources {
        std::fs::copy(sources, home.join("config.toml"))?;
    }
    let mut command = command(program);
    command.current_dir(root).env("CARGO_HOME", home);
    #[cfg(unix)]
    {
        let wrapper = replaced_wrapper(&command);
        if let Some(wrapper) = wrapper {
            command
                .env("RUSTC_WRAPPER", cargo_home_hiding_wrapper(root)?)
                .env(ORIGINAL_RUSTC_WRAPPER, wrapper);
        }
    }
    Ok(command)
}

/// The compiler wrapper Cargo would run for `command`: `RUSTC_WRAPPER` when set, even to nothing,
/// otherwise `CARGO_BUILD_RUSTC_WRAPPER`. A value the command sets wins over the process's own.
#[cfg(unix)]
fn replaced_wrapper(command: &Command) -> Option<std::ffi::OsString> {
    let lookup = |name: &str| {
        command
            .get_envs()
            .find(|(key, _)| *key == name)
            .map_or_else(
                || std::env::var_os(name),
                |(_, value)| value.map(OsStr::to_os_string),
            )
    };
    lookup("RUSTC_WRAPPER")
        .or_else(|| lookup("CARGO_BUILD_RUSTC_WRAPPER"))
        .filter(|wrapper| !wrapper.is_empty())
}

/// The variable through which the wrapper receives the wrapper it replaced.
#[cfg(unix)]
const ORIGINAL_RUSTC_WRAPPER: &str = "MAESTRO_FIXTURE_RUSTC_WRAPPER";

/// The wrapper alias in `root` that unsets `CARGO_HOME` and `CARGO_TARGET_DIR` before forwarding.
#[cfg(unix)]
fn cargo_home_hiding_wrapper(root: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let wrapper = root.join("rustc-wrapper");
    if !wrapper.exists() {
        crate::executable::install_recorded(
            &wrapper,
            "tooling-wrapper",
            r#"{"scenario":"hide-home"}"#,
        )?;
    }
    Ok(wrapper)
}

/// The resolved toolchain that compiled this test, independent of runtime overrides.
#[must_use]
pub fn channel() -> Option<&'static str> {
    option_env!("RUSTUP_TOOLCHAIN")
}

/// Write the complete supplied toolchain declaration into a fixture root.
///
/// # Errors
/// Returns failures writing the fixture declaration.
pub fn declare_toolchain(root: &std::path::Path, declaration: &str) -> std::io::Result<()> {
    std::fs::write(root.join("rust-toolchain.toml"), declaration)
}

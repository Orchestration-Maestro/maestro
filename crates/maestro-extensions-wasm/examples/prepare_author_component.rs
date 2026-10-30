//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components, warms their compilation cache
//! and exports every component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

// The preparation module below reaches source discovery through this import.
use build::cargo::components;

#[path = "../../maestro-tooling/src/example_files/preparation.rs"]
mod example_files;

/// Builds the components, warms their compilation cache, then hands their paths to nextest's
/// environment file.
fn main() -> Result<(), String> {
    let config = concat!(env!("CARGO_MANIFEST_DIR"), "/../../mise.toml");
    let rg_pin = mise_output(&["config", "get", "--file", config, "tools.ripgrep"])?;
    let fd_pin = mise_output(&["config", "get", "--file", config, "tools.fd"])?;
    let search = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    let rg = pinned_search_tool("rg", "ripgrep", &rg_pin, &search)?;
    let fd = pinned_search_tool("fd", "fd", &fd_pin, &search)?;
    let path = build::author_component()?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut entries = format!(
        "MAESTRO_TEST_RG={rg}\nMAESTRO_TEST_FD={fd}\n\
         MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n",
        path.display(),
        context.display()
    );
    let mut components = vec![path.to_owned(), context];
    entries.push_str(
        &example_files::prepare(
            &[
                root.join("examples"),
                root.join("../../examples/extensions"),
            ],
            |name, _| {
                let path = build::example_component(name).map_err(std::io::Error::other)?;
                components.push(path.clone());
                Ok(path)
            },
        )
        .map_err(|error| error.to_string())?,
    );
    build::warm(&components)?;
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Resolves a pinned development tool from the supplied search PATH.
///
/// # Errors
///
/// Returns an error if the supplied PATH has no absolute executable, the executable
/// fails or reports another version, or its path or output is not UTF-8.
fn pinned_search_tool(
    name: &str,
    tool: &str,
    version: &str,
    search: &std::ffi::OsStr,
) -> Result<String, String> {
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let path = std::env::split_paths(search)
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name}: no executable on PATH"))?;
    if !path.is_absolute() {
        return Err(format!(
            "{name}: executable path is not absolute: {}",
            path.display()
        ));
    }
    let output = std::process::Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|error| format!("{} --version: {error}", path.display()))?;
    let reported =
        String::from_utf8(output.stdout).map_err(|error| format!("{name} --version: {error}"))?;
    let mut words = reported.split_whitespace();
    if !output.status.success() || words.next() != Some(tool) || words.next() != Some(version) {
        return Err(format!(
            "{name}: expected {tool} {version}, got {reported:?}"
        ));
    }
    path.into_os_string()
        .into_string()
        .map_err(|path| format!("{name}: executable path is not UTF-8: {}", path.display()))
}

/// Reads stored tool pins without evaluating project configuration.
///
/// # Errors
///
/// Returns an error if mise cannot run, exits unsuccessfully or returns invalid UTF-8.
fn mise_output(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("mise")
        // Reading stored pins needs no trust and must not evaluate project hooks.
        .env("MISE_NO_CONFIG", "1")
        .args(args)
        .output()
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "mise {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))
}

#[cfg(test)]
mod tests {
    #![forbid(
        clippy::pedantic,
        clippy::too_many_arguments,
        clippy::excessive_nesting
    )]

    use super::pinned_search_tool;
    use maestro_test_directory::Scratch;
    use std::ffi::OsStr;

    /// Empty and nonexistent search directories never fall back to ambient PATH.
    #[test]
    fn pinned_search_rejects_missing_executable() {
        let root = Scratch::new().unwrap();
        for search in [OsStr::new(""), root.path().as_os_str()] {
            assert_eq!(
                pinned_search_tool("absent-search-tool", "fd", "1.2.3", search),
                Err("absent-search-tool: no executable on PATH".into())
            );
        }
    }

    /// Installs scripts in a separate process so concurrent forks inherit no writable handle.
    #[cfg(unix)]
    fn script(root: &std::path::Path, source: &str) -> std::path::PathBuf {
        let input = root.join("source");
        let path = root.join("fd");
        std::fs::write(&input, format!("#!/bin/sh\n{source}\n")).unwrap();
        assert!(
            std::process::Command::new("/usr/bin/install")
                .args(["-m", "0700"])
                .arg(input)
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        path
    }

    /// The supplied absolute directory wins regardless of ambient HOME or PATH.
    #[cfg(unix)]
    #[test]
    fn pinned_search_accepts_matching_identity_and_version() {
        let root = Scratch::new().unwrap();
        let path = script(
            root.path(),
            "[ \"$1\" = --version ] || exit 1; printf 'fd 1.2.3\\n'",
        );
        assert_eq!(
            pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str()).unwrap(),
            path.to_str().unwrap()
        );
    }

    /// A matching version alone does not establish executable identity or success.
    #[cfg(unix)]
    #[test]
    fn pinned_search_rejects_failed_or_mismatched_probes() {
        let root = Scratch::new().unwrap();
        for source in [
            "printf 'fd 1.2.3'; exit 1",
            "printf 'other 1.2.3'",
            "printf 'fd 0.0.0'",
            "printf ''",
        ] {
            script(root.path(), source);
            assert!(
                pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
                    .unwrap_err()
                    .starts_with("fd: expected fd 1.2.3, got ")
            );
        }
    }

    /// Spawn errors remain distinct from version mismatches.
    #[cfg(unix)]
    #[test]
    fn pinned_search_reports_probe_spawn_failure() {
        let root = Scratch::new().unwrap();
        std::fs::write(root.path().join("fd"), "not executable").unwrap();
        assert!(
            pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
                .unwrap_err()
                .contains("--version:")
        );
    }

    /// Relative candidates are refused rather than resolved against the current directory.
    #[cfg(unix)]
    #[test]
    fn pinned_search_rejects_relative_candidate() {
        let cwd = std::env::current_dir().unwrap();
        let root = Scratch::new_in(&cwd).unwrap();
        script(root.path(), "printf 'fd 1.2.3'");
        let relative = root.path().strip_prefix(&cwd).unwrap();
        assert!(
            pinned_search_tool("fd", "fd", "1.2.3", relative.as_os_str())
                .unwrap_err()
                .starts_with("fd: executable path is not absolute:")
        );
    }

    /// Invalid bytes from a successful probe cannot identify a pinned program.
    #[cfg(unix)]
    #[test]
    fn pinned_search_rejects_non_utf8_output() {
        let root = Scratch::new().unwrap();
        script(root.path(), "printf '\\377'");
        assert!(
            pinned_search_tool("fd", "fd", "1.2.3", root.path().as_os_str())
                .unwrap_err()
                .starts_with("fd --version:")
        );
    }

    /// Nextest's text environment file cannot transport a non-UTF-8 executable path.
    #[cfg(unix)]
    #[test]
    fn pinned_search_rejects_non_utf8_path() {
        use std::os::unix::ffi::OsStrExt;
        let root = Scratch::new().unwrap();
        let directory = root.path().join(OsStr::from_bytes(b"invalid-\xff"));
        std::fs::create_dir(&directory).unwrap();
        script(&directory, "printf 'fd 1.2.3'");
        assert!(
            pinned_search_tool("fd", "fd", "1.2.3", directory.as_os_str())
                .unwrap_err()
                .starts_with("fd: executable path is not UTF-8:")
        );
    }
}

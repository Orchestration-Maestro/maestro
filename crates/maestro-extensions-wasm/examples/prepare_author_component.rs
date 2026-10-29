//! Prepares the author components before nextest's guest-authoring tests.
//!
//! Nextest runs this example from the test run's own build output, so preparing the components
//! compiles no second host. It builds the components, warms the compilation cache of the shared
//! and context components and exports every component's path to the tests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "../src/tests/build.rs"]
mod build;

#[path = "../../maestro-tooling/src/example_files.rs"]
mod example_files;

/// Builds and compiles the components, then hands their paths to nextest's environment file.
fn main() -> Result<(), String> {
    let rg = pinned_search_tool("rg", "ripgrep")?;
    let fd = pinned_search_tool("fd", "fd")?;
    let path = build::author_component()?;
    build::compile(path).map_err(|error| error.to_string())?;
    let context = build::component("context_bridge", "MAESTRO_CONTEXT_COMPONENT")?;
    build::compile(&context).map_err(|error| error.to_string())?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut entries = format!(
        "MAESTRO_TEST_RG={rg}\nMAESTRO_TEST_FD={fd}\n\
         MAESTRO_AUTHOR_COMPONENT={}\nMAESTRO_CONTEXT_COMPONENT={}\n",
        path.display(),
        context.display()
    );
    entries.push_str(
        &example_files::prepare(
            &[
                root.join("examples"),
                root.join("../../examples/extensions"),
            ],
            |name, variable| build::component(name, variable).map_err(std::io::Error::other),
        )
        .map_err(|error| error.to_string())?,
    );
    if let Some(environment) = std::env::var_os("NEXTEST_ENV") {
        std::fs::write(environment, entries).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Resolves a pinned development tool from the supplied search PATH.
///
/// # Errors
///
/// Returns an error if the pin cannot be read, PATH has no absolute executable,
/// the executable fails or reports another version, or its path or output is not UTF-8.
fn pinned_search_tool(name: &str, tool: &str) -> Result<String, String> {
    let config = concat!(env!("CARGO_MANIFEST_DIR"), "/../../mise.toml");
    let version = mise_output(&["config", "get", "--file", config, &format!("tools.{tool}")])?;
    let search = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let path = std::env::split_paths(&search)
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
    if !output.status.success() || words.next() != Some(tool) || words.next() != Some(&version) {
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

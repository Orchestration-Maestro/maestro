//! The compiled-code cache lives below the agent directory the installation names.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::components::{files_below, load_empty_component};
use maestro_test_directory::Scratch;
use maestro_test_process::{AGENT_DIRECTORY, TestProcess, selection};

/// A load compiles its component into the cache below the agent directory, and writes none to
/// the platform's own cache locations under the home directory.
#[test]
fn component_load_caches_below_the_agent_directory() {
    let test = "component_load_caches_below_the_agent_directory";
    if selection().is_child() {
        load_empty_component(&Scratch::new().unwrap());
        return;
    }
    let scratch = Scratch::new().unwrap();
    let (agent, home) = (scratch.path().join("agent"), scratch.path().join("home"));
    std::fs::create_dir(&home).unwrap();
    let output = TestProcess::current(test)
        .unwrap()
        .env_clear()
        .case("load")
        .env(AGENT_DIRECTORY, &agent)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !files_below(&agent).is_empty(),
        "the agent directory holds no compiled code"
    );
    assert_eq!(files_below(&home), Vec::<std::path::PathBuf>::new());
}

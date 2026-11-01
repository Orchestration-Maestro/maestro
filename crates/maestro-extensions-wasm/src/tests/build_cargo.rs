//! The component build hands every selected example to one Cargo invocation.
use std::path::Path;

use super::build::cargo::build_command;

/// Cargo schedules the compiles of one invocation in parallel, so many examples share it
/// instead of building one after another.
#[test]
fn one_cargo_invocation_builds_every_selected_example() {
    let names = ["author_component", "context_bridge", "snake"];
    let command = build_command("cargo", Path::new("."), &names, Path::new("target"));
    let arguments: Vec<String> = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        arguments
            .iter()
            .filter(|argument| *argument == "build")
            .count(),
        1
    );
    let examples: Vec<&str> = arguments
        .windows(2)
        .filter(|pair| pair[0] == "--example")
        .map(|pair| pair[1].as_str())
        .collect();
    assert_eq!(examples, names);
}

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

/// The recipe that stages the development tool the setup runs.
const PREPARE: &str = "just prepare-tests";

/// The packages nextest assigns to the setups that shared CI prepares.
fn served_packages() -> Vec<String> {
    let ci: toml::Table = include_str!("../../../.github/ci.toml").parse().unwrap();
    let setups = ci["test_prepare_setups"].as_array().unwrap();
    let nextest: toml::Table = include_str!("../../../.config/nextest.toml")
        .parse()
        .unwrap();
    let rules = nextest["profile"]["default"]["scripts"].as_array().unwrap();
    let mut consumers: Vec<_> = rules
        .iter()
        .filter(|rule| {
            rule.get("setup")
                .is_some_and(|setup| setups.contains(setup))
        })
        .flat_map(|rule| rule["filter"].as_str().unwrap().split("package(").skip(1))
        .map(|rest| {
            rest.split_once(')')
                .unwrap()
                .0
                .trim_start_matches('=')
                .to_owned()
        })
        .collect();
    consumers.sort_unstable();
    consumers.dedup();
    consumers
}

/// The Markdown files under `directory`, recursively.
fn markdown(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            markdown(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
}

/// The commands of every fenced block in `text`, each logical command on one line.
fn commands(text: &str) -> Vec<Vec<String>> {
    let mut blocks = Vec::new();
    let mut open: Option<Vec<String>> = None;
    let mut continued = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match open.take() {
                Some(block) => blocks.push(block),
                None => open = Some(Vec::new()),
            }
            continued = false;
        } else if let Some(block) = open.as_mut() {
            let line = line.trim_end();
            let (text, next) = line
                .strip_suffix('\\')
                .map_or((line, false), |text| (text, true));
            match (continued, block.last_mut()) {
                (true, Some(last)) => {
                    last.push(' ');
                    last.push_str(text.trim_start());
                }
                _ => block.push(text.to_owned()),
            }
            continued = next;
        }
    }
    blocks
}

/// Whether `command` starts nextest on a package the setup serves.
fn runs_served_nextest(command: &str, served: &[String]) -> bool {
    let words: Vec<_> = command.split_whitespace().collect();
    command.contains("nextest run")
        && words
            .windows(2)
            .any(|pair| pair[0] == "-p" && served.iter().any(|package| package == pair[1]))
}

/// The first command of `block` that starts a served nextest run before any step stages the tool.
fn unstaged_run(block: Vec<String>, served: &[String]) -> Option<String> {
    let mut staged = false;
    for command in block {
        staged |= command.contains(PREPARE) || command.contains("prepare-test-setup prepare");
        if runs_served_nextest(&command, served) && !staged {
            return Some(command);
        }
    }
    None
}

/// Every documented nextest command on a served package follows a step that stages the tool.
#[test]
fn documented_nextest_commands_stage_the_setup_tool_first() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for directory in ["docs", "examples"] {
        markdown(&root.join(directory), &mut files);
    }
    let served = served_packages();
    let mut missing = Vec::new();
    for file in files {
        let blocks = commands(&fs::read_to_string(&file).unwrap());
        missing.extend(
            blocks
                .into_iter()
                .filter_map(|block| unstaged_run(block, &served))
                .map(|command| format!("{}: {command}", file.display())),
        );
    }
    assert!(
        missing.is_empty(),
        "run `{PREPARE}` before:\n{}",
        missing.join("\n")
    );
}

/// The development guide names the command that fixes a missing setup tool.
#[test]
fn development_guide_names_the_command_for_a_missing_setup_tool() {
    let guide =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/development.md"))
            .unwrap();
    assert!(
        guide.split("\n\n").any(
            |paragraph| paragraph.contains("target/test-setup/development")
                && paragraph.contains("missing")
                && paragraph.contains(PREPARE)
        ),
        "no paragraph of docs/development.md says `{PREPARE}` fixes a missing target/test-setup/development"
    );
}

/// A command on a package served by a nextest setup must stage the tool first.
#[test]
fn documented_nextest_commands_stage_the_tool_for_setup_consumers() {
    let served = vec!["maestro-app".to_owned()];
    let block = vec!["cargo nextest run --locked -p maestro-app".to_owned()];

    assert_eq!(
        unstaged_run(block, &served).as_deref(),
        Some("cargo nextest run --locked -p maestro-app")
    );
}

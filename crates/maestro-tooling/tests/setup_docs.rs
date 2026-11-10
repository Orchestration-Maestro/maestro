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

/// Whether `command` starts nextest on a package the setup serves: a package selected by `-p` or
/// `--package`, spaced or joined with `=`, or the whole workspace.
fn runs_served_nextest(command: &str, served: &[String]) -> bool {
    let words: Vec<_> = command.split_whitespace().collect();
    let names = |name: &str| served.iter().any(|package| package == name);
    command.contains("nextest run")
        && (words.contains(&"--workspace")
            || words
                .windows(2)
                .any(|pair| matches!(pair[0], "-p" | "--package") && names(pair[1]))
            || words.iter().any(|word| {
                word.strip_prefix("--package=")
                    .or_else(|| word.strip_prefix("-p="))
                    .is_some_and(names)
            }))
}

/// Every Markdown page that documents a command: the docs, the examples, the contributing guide
/// and each crate's README.
fn documented_pages(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in ["docs", "examples"] {
        markdown(&root.join(directory), &mut files);
    }
    files.push(root.join("CONTRIBUTING.md"));
    let mut crates: Vec<_> = fs::read_dir(root.join("crates"))
        .unwrap()
        .map(|entry| entry.unwrap().path().join("README.md"))
        .filter(|readme| readme.is_file())
        .collect();
    crates.sort();
    files.extend(crates);
    files
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
    let files = documented_pages(&root);
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

/// Every spelling that selects a served package, or the whole workspace, must stage the tool.
#[test]
fn documented_nextest_commands_stage_the_tool_for_every_package_spelling() {
    let served = vec!["maestro-app".to_owned()];
    for command in [
        "cargo nextest run --locked --package maestro-app",
        "cargo nextest run --locked --package=maestro-app",
        "cargo nextest run --locked -p=maestro-app",
        "cargo nextest run --locked --workspace",
    ] {
        assert_eq!(
            unstaged_run(vec![command.to_owned()], &served).as_deref(),
            Some(command)
        );
    }
    assert_eq!(
        unstaged_run(
            vec!["cargo nextest run --locked --package maestro-text".to_owned()],
            &served
        ),
        None
    );
}

/// The checked pages include the contributing guide and every crate README.
#[test]
fn documented_pages_include_the_contributing_guide_and_crate_readmes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pages = documented_pages(&root);

    assert!(pages.contains(&root.join("CONTRIBUTING.md")));
    assert!(pages.contains(&root.join("crates/maestro-agent/README.md")));
}

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

/// The recipe that stages the development tool the setup runs.
const PREPARE: &str = "just prepare-tests";

/// The packages whose tests nextest's author-component setup serves, as shared CI declares them.
fn served_packages() -> Vec<String> {
    let ci: toml::Table = include_str!("../../../.github/ci.toml").parse().unwrap();
    ci["test_prepare_packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
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

/// The documentation files that may name a nextest command, under the checkout `root`.
fn documentation_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in ["docs", "examples"] {
        markdown(&root.join(directory), &mut files);
    }
    files.push(root.join("README.md"));
    files.push(root.join("CONTRIBUTING.md"));
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let readme = entry.unwrap().path().join("README.md");
        if readme.is_file() {
            files.push(readme);
        }
    }
    files
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

/// Whether `command` starts nextest on the whole workspace or on a package the setup serves,
/// selected as `-p name`, `--package name`, `-p=name` or `--package=name`.
fn runs_served_nextest(command: &str, served: &[String]) -> bool {
    let words: Vec<_> = command.split_whitespace().collect();
    let selects = |package: &str| served.iter().any(|served| served == package);
    command.contains("nextest run")
        && (words.contains(&"--workspace")
            || words
                .windows(2)
                .any(|pair| matches!(pair[0], "-p" | "--package") && selects(pair[1]))
            || words
                .iter()
                .filter_map(|word| word.strip_prefix("--package=").or(word.strip_prefix("-p=")))
                .any(selects))
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
    let files = documentation_files(&root);
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

/// A command on a package that shared CI adds to the served list is checked like the others.
#[test]
fn documented_nextest_commands_follow_the_packages_shared_ci_declares() {
    let served = vec!["maestro-app".to_owned()];
    let block = vec!["cargo nextest run --locked -p maestro-app".to_owned()];

    assert_eq!(
        unstaged_run(block, &served).as_deref(),
        Some("cargo nextest run --locked -p maestro-app")
    );
}

/// A run selected with `--package`, or over the whole workspace, serves the same packages.
#[test]
fn documented_nextest_commands_cover_every_package_selection_spelling() {
    let served = vec!["maestro-app".to_owned()];

    for command in [
        "cargo nextest run --package maestro-app",
        "cargo nextest run --package=maestro-app",
        "cargo nextest run -p=maestro-app",
        "cargo nextest run --workspace",
    ] {
        assert_eq!(
            unstaged_run(vec![command.to_owned()], &served).as_deref(),
            Some(command)
        );
    }
    assert_eq!(
        unstaged_run(
            vec!["cargo nextest run --package maestro-text".to_owned()],
            &served
        ),
        None
    );
}

/// The checked files include the contribution guide, the root readme and every crate readme.
#[test]
fn documentation_files_include_the_guide_and_the_readmes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let files = documentation_files(&root);

    for expected in [
        "CONTRIBUTING.md",
        "README.md",
        "crates/maestro-agent/README.md",
        "docs/development.md",
    ] {
        assert!(
            files.iter().any(|file| file.ends_with(expected)),
            "{expected} is not checked"
        );
    }
}

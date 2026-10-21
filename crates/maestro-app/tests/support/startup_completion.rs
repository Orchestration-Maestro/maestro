//! Native corpus replay; only the operation runs under restricted filesystem access.
use super::{Case, cases, child};
use maestro_app::configuration::migrations::{MigrationStyle, run_migrations};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs, io,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    entries: BTreeMap<String, InputEntry>,
    root: Option<String>,
    cwd: Option<String>,
    #[serde(default)]
    relative_root: bool,
    #[serde(default)]
    permissions: BTreeMap<String, u32>,
    #[serde(default)]
    unreadable: Vec<String>,
    #[serde(default)]
    observe_modes: Vec<String>,
}
#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum InputEntry {
    #[serde(rename = "dir")]
    Directory,
    #[serde(rename = "file")]
    File { text: String },
    #[serde(rename = "link")]
    Link { target: String },
    #[serde(rename = "hardlink")]
    Hardlink { target: String },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    calls: Vec<Observation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    result: Option<ResultFields>,
    error: Option<Failure>,
    output: String,
    tree: BTreeMap<String, Entry>,
    #[serde(default)]
    observed_modes: BTreeMap<String, u32>,
}
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ResultFields {
    migrated_auth_providers: Vec<String>,
    deprecation_warnings: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Failure {
    code: String,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Entry {
    #[serde(rename = "dir")]
    Directory,
    #[serde(rename = "file")]
    File { hex: String },
    #[serde(rename = "link")]
    Link {
        target: String,
        readable: bool,
        hex: Option<String>,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    result: Option<ResultFields>,
    error: Option<Failure>,
    output: String,
}

/// Paint a complete logical console line using the recording's supplied palette.
pub fn paint(style: MigrationStyle, text: &str) -> String {
    let (open, close) = match style {
        MigrationStyle::Success => ("\x1b[32m", "\x1b[39m"),
        MigrationStyle::Warning => ("\x1b[33m", "\x1b[39m"),
        MigrationStyle::Dim => ("\x1b[2m", "\x1b[22m"),
        MigrationStyle::Plain => return format!("{text}\n"),
    };
    let text = text
        .replace(close, &format!("{close}{open}"))
        .replace('\n', &format!("{close}\n{open}"));
    format!("{open}{text}{close}\n")
}

fn setup(root: &Path, input: &Input) {
    fs::create_dir(root.join("agent")).unwrap();
    fs::create_dir(root.join("project")).unwrap();
    for (name, entry) in &input.entries {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        match entry {
            InputEntry::Directory => fs::create_dir_all(path).unwrap(),
            InputEntry::File { text } => {
                fs::write(path, text.replace("$ROOT", root.to_str().unwrap())).unwrap();
            }
            InputEntry::Link { target } => {
                symlink(target.replace("$ROOT", root.to_str().unwrap()), path).unwrap();
            }
            InputEntry::Hardlink { .. } => (),
        }
    }
    for (name, entry) in &input.entries {
        if let InputEntry::Hardlink { target } = entry {
            fs::hard_link(root.join(target), root.join(name)).unwrap();
        }
    }
    for (name, mode) in &input.permissions {
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(*mode)).unwrap();
    }
}

fn tree(root: &Path, relative: &str, found: &mut Vec<String>) {
    let path = root.join(relative);
    if !relative.is_empty() {
        found.push(relative.into());
    }
    if fs::symlink_metadata(&path).unwrap().is_dir() {
        for entry in fs::read_dir(&path).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            tree(
                root,
                &if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                },
                found,
            );
        }
    }
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, b| {
        write!(text, "{b:02x}").unwrap();
        text
    })
}
fn check_tree(root: &Path, expected: &BTreeMap<String, Entry>) {
    let mut found = Vec::new();
    tree(root, "", &mut found);
    found.sort();
    assert_eq!(found, expected.keys().cloned().collect::<Vec<_>>());
    for (name, entry) in expected {
        let path = root.join(name);
        let meta = fs::symlink_metadata(&path).unwrap();
        match entry {
            Entry::Directory => assert!(meta.is_dir(), "{name}"),
            Entry::File { hex: expected } => {
                assert!(meta.is_file());
                assert!(
                    hex(&fs::read(path).unwrap()) == *expected,
                    "file bytes at {name}"
                );
            }
            Entry::Link {
                target,
                readable,
                hex: expected,
            } => {
                assert!(meta.is_symlink());
                assert_eq!(
                    fs::read_link(&path).unwrap().to_str().unwrap(),
                    target.replace("$ROOT", root.to_str().unwrap())
                );
                assert_eq!(path.exists(), *readable);
                assert_eq!(fs::read(path).ok().map(|bytes| hex(&bytes)), *expected);
            }
        }
    }
}
fn operation() {
    let root = std::env::var("COMPLETION_ROOT").unwrap();
    let unreadable: Vec<String> =
        serde_json::from_str(&std::env::var("COMPLETION_UNREADABLE").unwrap()).unwrap();
    let saved: Vec<_> = unreadable
        .iter()
        .map(|name| {
            let path = Path::new(&root).join(name);
            let mode = fs::metadata(&path).unwrap().permissions();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
            (path, mode)
        })
        .collect();
    let mut output = String::new();
    let result = run_migrations(
        &std::env::var("COMPLETION_CWD").unwrap(),
        &mut |style, text| {
            output.push_str(&paint(style, text));
            Ok(())
        },
    );
    for (path, mode) in saved {
        fs::set_permissions(path, mode).unwrap();
    }
    let (result, error) = match result {
        Ok(value) => (
            Some(ResultFields {
                migrated_auth_providers: value.migrated_auth_providers,
                deprecation_warnings: value.deprecation_warnings,
            }),
            None,
        ),
        Err(error) => (
            None,
            Some(Failure {
                code: match error.kind() {
                    io::ErrorKind::PermissionDenied => "EACCES",
                    _ => panic!("unexpected native failure category"),
                }
                .into(),
            }),
        ),
    };
    println!(
        "\nPROBE {}",
        serde_json::to_string(&Outcome {
            result,
            error,
            output
        })
        .unwrap()
    );
}
fn replay(name: &str, case: Case<Input, Expected>) {
    let scratch = child::Scratch::new();
    setup(scratch.0.path(), &case.input);
    let authored = case.input.root.as_deref().unwrap_or("agent");
    let agent = if case.input.relative_root {
        format!("./{authored}")
    } else {
        scratch.join(authored)
    };
    let cwd = scratch.join(case.input.cwd.as_deref().unwrap_or("project"));
    assert_eq!(case.expected.calls.len(), 2);
    for expected in case.expected.calls {
        let lines = child::probe(
            maestro_test_process::TestProcess::current(name)
                .unwrap()
                .case("*"),
            scratch.0.path(),
            &[
                ("COMPLETION_OPERATION", "1"),
                ("COMPLETION_ROOT", scratch.0.path().to_str().unwrap()),
                (
                    "COMPLETION_UNREADABLE",
                    &serde_json::to_string(&case.input.unreadable).unwrap(),
                ),
                ("COMPLETION_CWD", &cwd),
                ("MAESTRO_CODING_AGENT_DIR", &agent),
            ],
            &[],
        );
        assert_eq!(lines.len(), 1);
        let actual: Outcome = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(actual.result, expected.result);
        assert_eq!(actual.error, expected.error);
        assert_eq!(
            normalize_output(&actual.output, case.input.permissions.contains_key("agent")),
            expected.output
        );
        check_tree(scratch.0.path(), &expected.tree);
        assert_eq!(
            case.input.observe_modes,
            expected.observed_modes.keys().cloned().collect::<Vec<_>>()
        );
        for (path, mode) in expected.observed_modes {
            assert_eq!(
                fs::metadata(scratch.0.path().join(path))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                mode
            );
        }
    }
    for name in case.input.permissions.keys() {
        let path = scratch.0.path().join(name);
        if path.exists() {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
}

pub fn run(name: &str) {
    if std::env::var_os("COMPLETION_OPERATION").is_some() {
        operation();
        return;
    }
    // A privileged run ignores the cases' modes, so those cases have nothing to check.
    let probe = maestro_test_directory::Scratch::new().unwrap();
    let denials = maestro_test_directory::deny(probe.path(), 0o0)
        .unwrap()
        .is_some();
    for case in cases::<Input, Expected>(name) {
        if denials || (case.input.permissions.is_empty() && case.input.unreadable.is_empty()) {
            replay(name, case);
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConverterInput {
    raw: serde_json::Map<String, serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConverterExpected {
    config: serde_json::Map<String, serde_json::Value>,
    order: Vec<String>,
    migrated: bool,
    pretty: String,
}

pub fn run_order(name: &str) {
    if std::env::var_os("COMPLETION_OPERATION").is_some() {
        operation();
        return;
    }
    for case in cases::<serde_json::Value, serde_json::Value>(name) {
        if case.input.get("raw").is_some() {
            let input: ConverterInput = serde_json::from_value(case.input).unwrap();
            let expected: ConverterExpected = serde_json::from_value(case.expected).unwrap();
            let (config, migrated) =
                maestro_app::presentation_data::keybindings::migrate_keybindings_config(&input.raw);
            assert_eq!(config, expected.config);
            assert_eq!(config.keys().cloned().collect::<Vec<_>>(), expected.order);
            assert_eq!(migrated, expected.migrated);
            assert_eq!(
                format!("{}\n", maestro_json::pretty_object(&config).unwrap()),
                expected.pretty
            );
        } else {
            replay(
                name,
                Case {
                    input: serde_json::from_value(case.input).unwrap(),
                    expected: serde_json::from_value(case.expected).unwrap(),
                },
            );
        }
    }
}

fn normalize_output(output: &str, denied: bool) -> String {
    output
        .lines()
        .map(|line| {
            if let Some((prefix, suffix)) = line.split_once(" commands/ to prompts/: ") {
                let code = if denied { 13 } else { 20 };
                assert!(
                    suffix.contains(&format!("(os error {code})")),
                    "native rename category"
                );
                format!("{prefix} commands/ to prompts/: <native rename failure>\x1b[39m\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

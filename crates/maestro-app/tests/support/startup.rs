//! Strict corpus replay with isolated environment and native failure controls.
use super::child;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::Path,
    process::Command,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Vec<Observation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    operation: String,
    files: BTreeMap<String, String>,
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    symlinks: BTreeMap<String, String>,
    #[serde(default)]
    hardlinks: BTreeMap<String, String>,
    #[serde(default)]
    binary_files: BTreeMap<String, String>,
    #[serde(default)]
    file_modes: BTreeMap<String, u32>,
    root_mode: Option<u32>,
    limit_kib: Option<u64>,
    #[serde(default)]
    relative_root: bool,
    root_kind: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    providers: Option<Vec<String>>,
    error: Option<Failure>,
    tree: Vec<Entry>,
    settings_target_identity_preserved: Option<bool>,
}
#[derive(Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Failure {
    code: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    kind: String,
    text: Option<String>,
    base64: Option<String>,
    bytes: Option<u64>,
    mode: Option<u32>,
    target: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    providers: Option<Vec<String>>,
    error: Option<Failure>,
}

fn setup(root: &Path, input: &Input) {
    fs::create_dir(root).unwrap();
    for (name, text) in &input.files {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o640)).unwrap();
    }
    for name in &input.directories {
        fs::create_dir_all(root.join(name)).unwrap();
    }
    for (name, target) in &input.symlinks {
        fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
        symlink(target, root.join(name)).unwrap();
    }
    for (name, data) in &input.binary_files {
        fs::write(root.join(name), STANDARD.decode(data).unwrap()).unwrap();
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o640)).unwrap();
    }
    for (name, target) in &input.hardlinks {
        fs::hard_link(root.join(target), root.join(name)).unwrap();
    }
    for (name, mode) in &input.file_modes {
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(*mode)).unwrap();
    }
    match input.root_kind.as_deref() {
        Some("absent") => fs::remove_dir(root).unwrap(),
        Some("file") => {
            fs::remove_dir(root).unwrap();
            fs::write(root, "not a directory").unwrap();
            fs::set_permissions(root, fs::Permissions::from_mode(0o640)).unwrap();
        }
        None => (),
        _ => panic!("unknown root kind"),
    }
}

fn paths(root: &Path, relative: &str, found: &mut Vec<String>) {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).unwrap();
    if !relative.is_empty() {
        found.push(relative.to_owned());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            paths(
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

fn check_tree(root: &Path, expected: &[Entry]) {
    let mut actual = Vec::new();
    if root.is_file() {
        actual.push(".".into());
    } else if root.exists() {
        paths(root, "", &mut actual);
    }
    actual.sort();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|item| item.path.clone())
            .collect::<Vec<_>>()
    );
    for item in expected {
        check_entry(root, item);
    }
}

fn check_entry(root: &Path, item: &Entry) {
    let path = if item.path == "." {
        root.to_path_buf()
    } else {
        root.join(&item.path)
    };
    let meta = fs::symlink_metadata(&path).unwrap();
    match item.kind.as_str() {
        "link" => {
            assert!(meta.is_symlink());
            check_link(root, &path, item);
        }
        "directory" => assert!(meta.is_dir()),
        "file" => {
            assert!(meta.is_file());
            let bytes = fs::read(&path).unwrap();
            assert_eq!(Some(meta.len()), item.bytes);
            if let Some(text) = &item.text {
                assert!(bytes == text.as_bytes(), "text mismatch at {}", item.path);
            }
            if let Some(data) = &item.base64 {
                assert!(bytes == STANDARD.decode(data).unwrap(), "binary mismatch");
            }
        }
        _ => panic!("unknown entry kind"),
    }
    if let Some(mode) = item.mode {
        assert_eq!(meta.mode() & 0o777, mode, "mode at {}", item.path);
    }
}
fn check_link(root: &Path, path: &Path, item: &Entry) {
    if let Some(text) = &item.text {
        assert_eq!(fs::read(path).unwrap(), text.as_bytes());
    }
    assert_eq!(
        fs::read_link(path).unwrap().to_str(),
        item.target
            .as_ref()
            .map(|target| target.replace("{root}", root.to_str().unwrap()))
            .as_deref()
    );
}

fn operation() {
    use maestro_app::configuration::migrations::{
        migrate_auth_to_auth_json, migrate_sessions_from_agent_root,
    };
    let result = match std::env::var("STARTUP_OPERATION").unwrap().as_str() {
        "auth" => migrate_auth_to_auth_json().map(Some),
        "sessions" => migrate_sessions_from_agent_root().map(|()| None),
        _ => panic!("unknown operation"),
    };
    let outcome = match result {
        Ok(providers) => Outcome {
            providers,
            error: None,
        },
        Err(error) => Outcome {
            providers: None,
            error: Some(Failure {
                code: match error.raw_os_error() {
                    Some(13) => "EACCES",
                    Some(27) => "EFBIG",
                    _ => panic!("unexpected native error category"),
                }
                .into(),
            }),
        },
    };
    println!("\nPROBE {}", serde_json::to_string(&outcome).unwrap());
}

fn invoke(name: &str, cwd: &Path, envs: &[(&str, &str)], limit: Option<u64>) -> Outcome {
    let lines = if let Some(limit) = limit {
        let output = Command::new("bash")
            .args([
                "-c",
                "trap '' XFSZ; ulimit -f \"$1\"; shift; exec \"$@\"",
                "startup",
                &limit.to_string(),
            ])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .current_dir(cwd)
            .envs(envs.iter().copied())
            .output()
            .unwrap();
        assert!(output.status.success(), "limited child transport failed");
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| line.strip_prefix("PROBE ").map(str::to_owned))
            .collect()
    } else {
        child::probe(name, cwd, envs, &[])
    };
    assert_eq!(lines.len(), 1);
    serde_json::from_str(&lines[0]).unwrap()
}

pub fn run(name: &str) {
    if std::env::var_os("STARTUP_OPERATION").is_some() {
        operation();
        return;
    }
    if std::env::var_os("STARTUP_CORPUS_CHILD").is_none() {
        let output = Command::new("bash")
            .args(["-c", "umask 002; exec \"$@\"", "startup"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env("STARTUP_CORPUS_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "corpus child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let mut corpus: BTreeMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("../fixtures/startup_conversions.json")).unwrap();
    assert_eq!(corpus.len(), 29);
    assert_eq!(corpus.values().map(Vec::len).sum::<usize>(), 125);
    for (index, case) in corpus.remove(name).unwrap().into_iter().enumerate() {
        replay(name, index, &case);
    }
}

fn replay(name: &str, index: usize, case: &Case) {
    let scratch = child::Scratch::new();
    let root = std::path::PathBuf::from(scratch.join("root"));
    setup(&root, &case.input);
    let identity = case
        .expected
        .iter()
        .any(|e| e.settings_target_identity_preserved.is_some())
        .then(|| fs::metadata(root.join("settings.json")).unwrap().ino());
    assert_eq!(case.expected.len(), 2);
    for (turn, expected) in case.expected.iter().enumerate() {
        let mode = if turn == 0 {
            case.input.root_mode.unwrap_or(0o700)
        } else {
            0o700
        };
        if root.is_dir() {
            fs::set_permissions(&root, fs::Permissions::from_mode(mode)).unwrap();
        }
        let authored = if case.input.relative_root {
            "./root".into()
        } else {
            root.to_str().unwrap().to_owned()
        };
        let actual = invoke(
            name,
            &scratch.0,
            &[
                ("STARTUP_OPERATION", &case.input.operation),
                ("MAESTRO_CODING_AGENT_DIR", &authored),
            ],
            if turn == 0 {
                case.input.limit_kib
            } else {
                None
            },
        );
        assert!(
            actual.error == expected.error,
            "error category case {index} turn {turn}"
        );
        assert!(
            actual.providers == expected.providers,
            "provider order case {index} turn {turn}"
        );
        check_tree(&root, &expected.tree);
        if let Some(preserved) = expected.settings_target_identity_preserved {
            assert_eq!(
                identity == Some(fs::metadata(root.join("settings.json")).unwrap().ino()),
                preserved
            );
        }
    }
    if root.is_dir() {
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

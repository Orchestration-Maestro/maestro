//! Strict corpus replay with isolated environment and native failure controls.
use super::child;
use base64::{Engine, engine::general_purpose::STANDARD};
use maestro_test_process::{IsolatedSetup, TestProcess};
use serde::Deserialize;
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
#[derive(Deserialize, PartialEq)]
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
/// What one conversion child reported.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    providers: Option<Vec<String>>,
    error: Option<Failure>,
}

impl Outcome {
    /// The failure category the conversion reported, if it failed.
    pub fn error(self) -> Option<String> {
        self.error.map(|error| error.code)
    }
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
    // The conversion resolves link targets canonically, so a linked temporary root differs.
    let root = maestro_path::fs::canonicalize(root).unwrap();
    assert_eq!(
        fs::read_link(path).unwrap().to_str(),
        item.target
            .as_ref()
            .map(|target| target.replace("{root}", root.to_str().unwrap()))
            .as_deref()
    );
}

/// One conversion launch.
struct Launch<'a> {
    /// The operation the child runs, `auth` or `sessions`.
    operation: &'a str,
    /// The agent directory the child reads, as authored.
    agent_dir: &'a str,
    /// The file-size limit, in kibibytes, the child runs the conversion under.
    limit_kib: Option<u64>,
    /// A file the child writes after lifting the limit.
    exit_write: Option<&'a str>,
}

/// Run one conversion in the prepared child, in `cwd`, and read its outcome.
fn invoke(cwd: &Path, launch: &Launch) -> Outcome {
    let program = std::env::var_os("MAESTRO_STARTUP_CONVERSION")
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    let mut command = Command::new(program);
    command
        .arg(launch.operation)
        .current_dir(cwd)
        .env("MAESTRO_CODING_AGENT_DIR", launch.agent_dir);
    if let Some(limit) = launch.limit_kib {
        let (name, request) = IsolatedSetup::new().file_size_continues().environment();
        command
            .env(name, request)
            .args(["--file-limit-kib", &limit.to_string()]);
    }
    if let Some(path) = launch.exit_write {
        command.args(["--exit-write", path]);
    }
    let output = command.output().expect("the conversion child starts");
    assert!(
        output.status.success(),
        "conversion child failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("PROBE "));
    let line = lines.next().expect("the child reports one outcome");
    assert!(lines.next().is_none());
    serde_json::from_str(line).unwrap()
}

/// Move the sessions below `agent_dir` in the prepared child, in `cwd`, and require success.
pub fn sessions(cwd: &Path, agent_dir: &str) {
    assert!(convert("sessions", cwd, agent_dir).error().is_none());
}

/// Run `operation` once in the prepared child, in `cwd`, with `agent_dir` as the agent directory.
pub fn convert(operation: &str, cwd: &Path, agent_dir: &str) -> Outcome {
    invoke(
        cwd,
        &Launch {
            operation,
            agent_dir,
            limit_kib: None,
            exit_write: None,
        },
    )
}

/// Check that a limited operation leaves room for exit-time profile writes.
pub fn check_file_limit_restoration() {
    let scratch = child::Scratch::new();
    let output = scratch.join("exit-write");
    for fail_write in [false, true] {
        if fail_write {
            fs::write(
                scratch.0.path().join("settings.json"),
                format!("{{\"apiKeys\":{{\"provider\":\"{}\"}}}}", "k".repeat(4096)),
            )
            .unwrap();
        }
        let outcome = invoke(
            scratch.0.path(),
            &Launch {
                operation: "auth",
                agent_dir: scratch.0.path().to_str().unwrap(),
                limit_kib: Some(1),
                exit_write: Some(&output),
            },
        );
        assert_eq!(outcome.error(), fail_write.then(|| "EFBIG".into()));
        assert_eq!(fs::read(&output).unwrap(), [0_u8; 4096]);
        fs::remove_file(&output).unwrap();
    }
}

/// Check that a conversion through a dangling `auth.json` link fails and creates no directory.
pub fn check_dangling_auth_link() {
    let scratch = child::Scratch::new();
    let root = scratch.0.path();
    fs::write(
        root.join("oauth.json"),
        r#"{"provider":{"access":"token"}}"#,
    )
    .unwrap();
    symlink("missing/nested/auth.json", root.join("auth.json")).unwrap();

    let outcome = convert("auth", root, root.to_str().unwrap());

    assert_eq!(outcome.error(), Some("ENOENT".into()));
    assert_eq!(
        fs::read_link(root.join("auth.json")).unwrap(),
        Path::new("missing/nested/auth.json")
    );
    assert!(!root.join("missing").exists());
}

pub fn run(name: &str) {
    if std::env::var_os("STARTUP_CORPUS_CHILD").is_none() {
        let output = TestProcess::current(name)
            .unwrap()
            .setup(IsolatedSetup::new().umask(0o002))
            .nocapture()
            .single_threaded()
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
    let _setup = IsolatedSetup::apply_requested().expect("the child applies its request");
    let mut corpus: BTreeMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("../fixtures/startup_conversions.json")).unwrap();
    assert_eq!(corpus.len(), 29);
    assert_eq!(corpus.values().map(Vec::len).sum::<usize>(), 125);
    // A privileged run ignores the cases' modes, so those cases have nothing to check.
    let denials = crate::native::denials();
    for (index, case) in corpus.remove(name).unwrap().into_iter().enumerate() {
        if denials || (case.input.file_modes.is_empty() && case.input.root_mode.is_none()) {
            replay(index, &case);
        }
    }
}

fn replay(index: usize, case: &Case) {
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
            scratch.0.path(),
            &Launch {
                operation: &case.input.operation,
                agent_dir: &authored,
                limit_kib: if turn == 0 {
                    case.input.limit_kib
                } else {
                    None
                },
                exit_write: None,
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

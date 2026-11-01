//! Self-update selection over real install trees built from recorded cases.
#![cfg(test)]

use maestro_app::configuration::config::{
    self, SelfUpdateContext, SelfUpdateOperations,
    installation::{Installation, PackageLayout},
};
use maestro_process::SpawnSyncError;
use maestro_test_process::TestProcess;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fs, io,
    path::Path,
    process::{ExitStatus, Output},
};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub input: Input,
    pub expected: Value,
    pub trace: Vec<Value>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub package_dir: Option<String>,
    pub package_name: Option<String>,
    pub executable: Option<String>,
    pub npm_command: Option<Vec<String>>,
    pub update_package_name: Option<String>,
    pub standalone: bool,
    pub windows: bool,
    pub op: Operation,
    pub home: Option<String>,
    pub cwd: Option<String>,
    pub cwd_error: Option<bool>,
    pub existing: Option<Vec<String>>,
    pub canonical: BTreeMap<String, String>,
    pub access_errors: Vec<String>,
    pub probe: Probe,
}
#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    #[default]
    Command,
    Unavailable,
    Instruction,
}
#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Probe {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            status: Some(1),
            stdout: String::new(),
            stderr: String::new(),
            error: None,
        }
    }
}
impl Input {
    pub fn dir(&self) -> &str {
        self.package_dir
            .as_deref()
            .unwrap_or("/opt/tool/lib/node_modules/@old/agent")
    }
    pub fn name(&self) -> &str {
        self.package_name.as_deref().unwrap_or("@old/agent")
    }
    pub fn installation(&self) -> Installation {
        let executable = self.executable.clone().unwrap_or_else(|| {
            if self.windows {
                maestro_path::win32::join(&[self.dir(), "dist", "cli.js"])
            } else {
                maestro_path::posix::join(&[self.dir(), "dist", "cli.js"])
            }
        });
        Installation {
            module_dir: if self.windows {
                "C:\\source"
            } else {
                "/source"
            }
            .into(),
            executable,
            layout: if self.standalone {
                PackageLayout::Standalone
            } else {
                PackageLayout::Package
            },
        }
    }
}
pub struct Operations<'a> {
    pub input: &'a Input,
    pub trace: RefCell<Vec<Value>>,
}
impl<'a> Operations<'a> {
    pub fn new(input: &'a Input) -> Self {
        Self {
            input,
            trace: RefCell::default(),
        }
    }
    fn record(&self, value: Value) {
        self.trace.borrow_mut().push(value);
    }
}
impl SelfUpdateOperations for Operations<'_> {
    fn is_windows(&self) -> bool {
        self.input.windows
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError> {
        self.record(json!(["command", command, args]));
        let probe = &self.input.probe;
        if let Some(error) = &probe.error {
            return Err(SpawnSyncError::Spawn(io::Error::other(error.clone())));
        }
        Ok(Output {
            status: status(probe.status),
            stdout: probe.stdout.as_bytes().to_vec(),
            stderr: probe.stderr.as_bytes().to_vec(),
        })
    }
}
#[cfg(unix)]
fn status(code: Option<i32>) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(code.map_or(9, |code| code << 8))
}
#[cfg(windows)]
fn status(code: Option<i32>) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    // Windows processes always have a numeric exit status.
    ExitStatus::from_raw(u32::try_from(code.unwrap_or(1)).unwrap())
}
pub fn command_value(command: config::SelfUpdateCommand) -> Value {
    let mut value =
        json!({"command": command.command, "args": command.args, "display": command.display});
    if let Some(steps) = command.steps {
        value["steps"] = steps
            .into_iter()
            .map(
                |step| json!({"command": step.command, "args": step.args, "display": step.display}),
            )
            .collect();
    }
    value
}
pub fn select(input: &Input, compose_root: bool) -> (io::Result<Value>, Vec<Value>) {
    let installation = input.installation();
    let operations = Operations::new(input);
    let dir = if compose_root {
        config::paths::get_package_dir(
            || unreachable!(),
            Some(input.dir()),
            || unreachable!(),
            |_| unreachable!(),
        )
        .unwrap()
    } else {
        input.dir().into()
    };
    let context = SelfUpdateContext {
        installation: &installation,
        package_dir: &dir,
        operations: &operations,
    };
    let result = match input.op {
        Operation::Command => config::get_self_update_command(
            &context,
            input.name(),
            input.npm_command.as_deref(),
            input.update_package_name.as_deref(),
        )
        .map(|command| command.map_or(Value::Null, command_value)),
        Operation::Unavailable => config::get_self_update_unavailable_instruction(
            &context,
            input.name(),
            input.npm_command.as_deref(),
            input.update_package_name.as_deref(),
        )
        .map(Value::String),
        Operation::Instruction => {
            config::get_update_instruction(&context, input.name()).map(Value::String)
        }
    };
    (result, operations.trace.into_inner())
}
/// Text with every absolute POSIX path in it moved below `root`. A path starts the text or
/// follows whitespace (the byte order mark included), `=` or a quote.
fn below(root: &str, text: &str) -> String {
    let mut moved = String::with_capacity(text.len());
    let mut previous = None;
    for c in text.chars() {
        if c == '/'
            && previous.is_none_or(|p: char| {
                p.is_whitespace() || matches!(p, '=' | '\'' | '"' | '\u{feff}')
            })
        {
            moved.push_str(root);
        }
        moved.push(c);
        previous = Some(c);
    }
    moved
}

/// Every string in `value`, object keys included, with its absolute POSIX paths moved below
/// `root`.
fn moved(root: &str, value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(below(root, &text)),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| moved(root, v)).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, v)| (below(root, &key), moved(root, v)))
                .collect(),
        ),
        other => other,
    }
}

/// Build the recorded install tree below `tree`, enter the recorded working directory and
/// return the case with its POSIX paths moved there. Existence and canonical paths now come
/// from the file system, so the trace no longer records those reads. `None` for a case that
/// needs links or a removed working directory where the platform has neither, for a case
/// recorded with POSIX paths on a Windows host.
fn prepare(mut case: Value, tree: &maestro_test_directory::Scratch) -> Option<Case> {
    if case["input"]["windows"] != true {
        if cfg!(windows) {
            return None;
        }
        let input = case["input"].as_object_mut().unwrap();
        input
            .entry("package_dir")
            .or_insert_with(|| json!("/opt/tool/lib/node_modules/@old/agent"));
        input.entry("cwd").or_insert_with(|| json!("/work"));
        case = moved(tree.path().to_str().unwrap(), case);
    }
    let mut case: Case = serde_json::from_value(case).unwrap();
    case.trace
        .retain(|step| !matches!(step[0].as_str(), Some("exists" | "canonicalize")));
    let input = &case.input;
    if cfg!(not(unix)) && (input.cwd_error == Some(true) || !input.canonical.is_empty()) {
        return None;
    }
    std::env::set_current_dir(tree.path()).unwrap();
    if let Some(cwd) = input.cwd.as_ref().filter(|_| input.cwd_error != Some(true)) {
        std::fs::create_dir_all(cwd).unwrap();
        std::env::set_current_dir(cwd).unwrap();
    }
    let present = input
        .existing
        .clone()
        .unwrap_or_else(|| vec![input.dir().to_owned()]);
    for path in present.iter().chain(input.canonical.values()) {
        std::fs::create_dir_all(path).unwrap();
    }
    #[cfg(unix)]
    for (link, target) in &input.canonical {
        let link = std::path::Path::new(link);
        if link.is_dir() {
            std::fs::remove_dir(link).unwrap();
        }
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    #[cfg(unix)]
    if input.cwd_error == Some(true) {
        tree.enter_removed("removed").unwrap();
        if case.expected == json!({"error": "cwd unavailable"}) {
            case.expected = json!({"error": std::env::current_dir().unwrap_err().to_string()});
        }
    }
    Some(case)
}

/// Refuse this process the write each recorded access error needs. `None` when the host is
/// not refused, as in a privileged run, so the row cannot state its premise here.
#[cfg(unix)]
fn refuse_writes(case: &Case) -> Option<Vec<maestro_test_directory::Restore>> {
    let mut restores = Vec::new();
    for path in &case.input.access_errors {
        match maestro_test_directory::deny(path, 0o500) {
            Ok(Some(restore)) => restores.push(restore),
            Ok(None) => {
                println!("{} needs a process the host refuses write access", case.id);
                return None;
            }
            // An absent path already refuses access to itself.
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => panic!("cannot deny {path}: {error}"),
        }
    }
    Some(restores)
}

/// Replay each recorded case on its own real install tree.
pub fn run(fixture: &str, compose_root: bool) {
    if replayed_elsewhere() {
        return;
    }
    replay_cases(fixture, compose_root);
}

/// Replay the recorded Bun-root cases, then select the same command from checked children
/// whose environments supply a home with spaces, an empty home, an absent home and an
/// unrelated home beside a matching bin root.
pub fn run_bun_roots(fixture: &str) {
    if replayed_elsewhere() {
        return;
    }
    replay_cases(fixture, false);
    // These rows state the Unix home rule, where an empty home stays empty and joins below
    // the working directory.
    #[cfg(unix)]
    for row in home_rows() {
        let tree = maestro_test_directory::Scratch::new().unwrap();
        let case = row.case(tree.path().to_str().unwrap());
        std::fs::create_dir_all(case.input.dir()).unwrap();
        replay_with_home(&case, false, Some(tree.path()), &tree);
    }
}

/// Whether another process replays the cases: a case child replays the one case it was
/// supplied, and the isolated run has already replayed this test's body. Every case moves
/// the working directory, which belongs to the whole process.
fn replayed_elsewhere() -> bool {
    if maestro_test_process::selection().is_child() {
        replay_child();
        return true;
    }
    maestro_test_process::run_alone(std::thread::current().name().unwrap()).unwrap()
}

/// Replay every case of `fixture`.
fn replay_cases(fixture: &str, compose_root: bool) {
    let cases: Vec<Value> = serde_json::from_str(fixture).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let tree = maestro_test_directory::Scratch::new().unwrap();
        let Some(case) = prepare(case, &tree) else {
            continue;
        };
        // The refused writes are given back before the tree that holds them is dropped.
        #[cfg(unix)]
        let Some(_restores) = refuse_writes(&case) else {
            continue;
        };
        if case.input.home.is_some() {
            replay_with_home(&case, compose_root, None, &tree);
        } else {
            replay(&case, compose_root);
        }
    }
}

/// Compare one case's selection against its recorded result and query trace.
fn replay(case: &Case, compose_root: bool) {
    let (result, trace) = select(&case.input, compose_root);
    let observed = match result {
        Ok(value) => json!({"value": value}),
        Err(error) => json!({"error": error.to_string()}),
    };
    assert_eq!(observed, case.expected, "case {}", case.id);
    assert_eq!(trace, case.trace, "trace {}", case.id);
}

/// Replay the case this process received from its parent.
fn replay_child() {
    let payload: SuppliedCase<Case> =
        serde_json::from_reader(io::stdin()).expect("a supplied case arrives on standard input");
    replay(&payload.case, payload.compose_root);
}

/// The case one checked child replays.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SuppliedCase<C> {
    /// Whether selection composes the caller package root.
    compose_root: bool,
    /// The prepared case.
    case: C,
}

/// Replay one case in a checked child whose environment supplies the home directory the
/// recorded selection observed, so this process's environment stays untouched. A case with
/// no home removes the variable instead.
fn replay_with_home(
    case: &Case,
    compose_root: bool,
    dir: Option<&Path>,
    tree: &maestro_test_directory::Scratch,
) {
    let path = tree.path().join("supplied-case.json");
    let mut file = fs::File::create_new(&path).unwrap();
    serde_json::to_writer(&mut file, &SuppliedCase { compose_root, case }).unwrap();
    drop(file);
    let mut process = TestProcess::current(std::thread::current().name().unwrap())
        .unwrap()
        .case(&case.id)
        .stdin(fs::File::open(&path).unwrap());
    process = match case.input.home.as_deref() {
        Some(home) => process.env(home_variable(), home),
        None => process.env_remove(home_variable()),
    };
    if let Some(dir) = dir {
        process = process.current_dir(dir);
    }
    let status = process.status().unwrap();
    assert!(status.success(), "case {} failed in its child", case.id);
}

/// The variable the host reads its home directory from.
fn home_variable() -> &'static str {
    if cfg!(windows) { "USERPROFILE" } else { "HOME" }
}

/// The selections whose home comes from the child environment: a home with spaces, an empty
/// home, an absent home and an unrelated home beside a matching bin root.
#[cfg(unix)]
fn home_rows() -> [HomeRow; 4] {
    const PACKAGE: &str = ".bun/install/global/node_modules/@old/agent";
    const CUSTOM: &str = "custom/install/global/node_modules/@old/agent";
    [
        // A home with spaces supplies the home-based root alone; the query succeeds empty.
        HomeRow {
            id: "home-spaces",
            home: Home::At("home with spaces"),
            package: "home with spaces/.bun/install/global/node_modules/@old/agent",
            stdout: None,
        },
        // An empty home joins the suffix below the working directory, not the account home.
        HomeRow {
            id: "home-empty",
            home: Home::At(""),
            package: PACKAGE,
            stdout: None,
        },
        // An absent home falls back to the account entry; the bin-derived root selects.
        HomeRow {
            id: "home-absent",
            home: Home::Absent,
            package: CUSTOM,
            stdout: Some("custom/bin"),
        },
        // A home holding no Bun root leaves the bin-derived root to select.
        HomeRow {
            id: "home-unrelated",
            home: Home::At("home with spaces"),
            package: CUSTOM,
            stdout: Some("custom/bin"),
        },
    ]
}

/// The home directory one row's child receives.
#[cfg(unix)]
enum Home {
    /// A directory below the tree; an empty spelling is the empty value.
    At(&'static str),
    /// No home variable at all.
    Absent,
}

/// One selection whose home comes from the child environment.
#[cfg(unix)]
struct HomeRow {
    /// Case identifier.
    id: &'static str,
    /// Home the child is given.
    home: Home,
    /// Package directory relative to the tree.
    package: &'static str,
    /// Captured query output relative to the tree, empty when absent.
    stdout: Option<&'static str>,
}

#[cfg(unix)]
impl HomeRow {
    /// The case this row replays below `root`, which must admit the Bun command.
    fn case(&self, root: &str) -> Case {
        Case {
            id: self.id.to_owned(),
            input: Input {
                package_dir: Some(format!("{root}/{}", self.package)),
                home: match self.home {
                    Home::At("") => Some(String::new()),
                    Home::At(home) => Some(format!("{root}/{home}")),
                    Home::Absent => None,
                },
                npm_command: Some(vec!["bun".to_owned()]),
                probe: Probe {
                    status: Some(0),
                    stdout: self
                        .stdout
                        .map_or_else(String::new, |path| format!("{root}/{path}")),
                    stderr: String::new(),
                    error: None,
                },
                ..Input::default()
            },
            expected: json!({"value": {
                "command": "bun",
                "args": ["install", "-g", "--ignore-scripts", "@old/agent"],
                "display": "bun install -g --ignore-scripts @old/agent",
            }}),
            trace: vec![json!(["command", "bun", ["pm", "bin", "-g"]])],
        }
    }
}

/// Assert the owned record only after destroying every selection input; the recorded install
/// trees of these cases are POSIX, so the row runs everywhere but Windows.
#[cfg(not(windows))]
pub fn run_owned(fixture: &str) {
    if maestro_test_process::run_alone(std::thread::current().name().unwrap()).unwrap() {
        return;
    }
    let mut cases: Vec<Value> = serde_json::from_str(fixture).expect("owned fixture parses");
    assert_eq!(cases.len(), 1);
    let tree = maestro_test_directory::Scratch::new().unwrap();
    let case = prepare(cases.remove(0), &tree).expect("the owned case needs no link");
    #[cfg(unix)]
    let Some(_restores) = refuse_writes(&case) else {
        return;
    };
    let Case {
        id,
        input,
        expected,
        trace,
    } = case;
    assert!(matches!(input.op, Operation::Command));
    let command = {
        let installation = input.installation();
        let operations = Operations::new(&input);
        let context = SelfUpdateContext {
            installation: &installation,
            package_dir: input.dir(),
            operations: &operations,
        };
        let command = config::get_self_update_command(
            &context,
            input.name(),
            input.npm_command.as_deref(),
            input.update_package_name.as_deref(),
        )
        .expect("selection succeeds")
        .expect("owned command exists");
        assert_eq!(*operations.trace.borrow(), trace, "trace {id}");
        command
    };
    drop(input);
    assert_eq!(
        json!({"value": command_value(command)}),
        expected,
        "case {id}"
    );
}

//! Self-update selection over real install trees built from recorded cases.
#![cfg(test)]

use maestro_app::configuration::config::{
    self, SelfUpdateContext, SelfUpdateOperations,
    installation::{Installation, PackageLayout},
};
use maestro_process::SpawnSyncError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io,
    process::{ExitStatus, Output},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub input: Input,
    pub expected: Value,
    pub trace: Vec<Value>,
}

#[derive(Default, Deserialize)]
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
    pub home_error: Option<bool>,
    pub cwd: Option<String>,
    pub cwd_error: Option<bool>,
    pub existing: Option<Vec<String>>,
    pub canonical: BTreeMap<String, String>,
    pub access_errors: Vec<String>,
    pub probe: Probe,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    #[default]
    Command,
    Unavailable,
    Instruction,
}
#[derive(Deserialize)]
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
    fn home_dir(&self) -> io::Result<String> {
        self.record(json!(["home"]));
        if self.input.home_error.unwrap_or(false) {
            return Err(io::Error::other("home unavailable"));
        }
        Ok(self
            .input
            .home
            .as_deref()
            .unwrap_or(if self.input.windows {
                "C:\\Users\\sample"
            } else {
                "/home/sample"
            })
            .into())
    }
    fn check_write_access(&self, path: &str) -> io::Result<()> {
        self.record(json!(["access", path]));
        if self.input.access_errors.iter().any(|p| p == path) {
            Err(io::Error::other("access denied"))
        } else {
            Ok(())
        }
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
/// needs links or a removed working directory where the platform has neither, and for a
/// case recorded with POSIX paths on a Windows host.
fn prepare(mut case: Value, tree: &maestro_test_directory::Scratch) -> Option<Case> {
    if case["input"]["windows"] != true {
        if cfg!(windows) {
            return None;
        }
        let input = case["input"].as_object_mut().unwrap();
        input
            .entry("package_dir")
            .or_insert_with(|| json!("/opt/tool/lib/node_modules/@old/agent"));
        input.entry("home").or_insert_with(|| json!("/home/sample"));
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

/// Replay each recorded case on its own real install tree.
pub fn run(fixture: &str, compose_root: bool) {
    // Every case moves the working directory, which belongs to the whole process.
    if maestro_test_process::run_alone(std::thread::current().name().unwrap()).unwrap() {
        return;
    }
    let cases: Vec<Value> = serde_json::from_str(fixture).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let tree = maestro_test_directory::Scratch::new().unwrap();
        let Some(case) = prepare(case, &tree) else {
            continue;
        };
        let (result, trace) = select(&case.input, compose_root);
        let observed = match result {
            Ok(value) => json!({"value": value}),
            Err(error) => json!({"error": error.to_string()}),
        };
        assert_eq!(observed, case.expected, "case {}", case.id);
        assert_eq!(trace, case.trace, "trace {}", case.id);
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
    let Case {
        id,
        input,
        expected,
        trace,
    } = prepare(cases.remove(0), &tree).expect("the owned case needs no link");
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

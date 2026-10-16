//! Controlled observations for self-update selection.
#![cfg(test)]

use maestro_app::configuration::config::{
    self, SelfUpdateContext, SelfUpdateOperations,
    installation::{Installation, PackageLayout},
};
use maestro_process::SpawnSyncError;
use maestro_resources::{ResourceEntry, ResourceFileType, ResourceOperations};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
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
    pub drive_directories: BTreeMap<String, String>,
    pub canonical: BTreeMap<String, String>,
    pub canonical_errors: Vec<String>,
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
    fn dirname(&self, path: &str) -> String {
        if self.windows {
            maestro_path::win32::dirname(path)
        } else {
            maestro_path::posix::dirname(path)
        }
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
impl ResourceOperations for Operations<'_> {
    fn exists(&self, path: &Path) -> bool {
        let path = path.to_str().unwrap();
        self.record(json!(["exists", path]));
        if let Some(existing) = &self.input.existing {
            return existing.iter().any(|p| p == path);
        }
        let parent = self.input.dirname(self.input.dir());
        let root = if self.input.name().starts_with('@') {
            self.input.dirname(&parent)
        } else {
            parent
        };
        path == self.input.dir() || path == root
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let path = path.to_str().unwrap();
        self.record(json!(["canonicalize", path]));
        if self.input.canonical_errors.iter().any(|p| p == path) {
            return Err(io::Error::other("canonicalize failed"));
        }
        Ok(self
            .input
            .canonical
            .get(path)
            .map_or(path, String::as_str)
            .into())
    }
    fn current_directory(&self) -> io::Result<String> {
        if self.input.cwd_error.unwrap_or(false) {
            return Err(io::Error::other("cwd unavailable"));
        }
        Ok(self
            .input
            .cwd
            .as_deref()
            .unwrap_or(if self.input.windows {
                "C:\\work"
            } else {
                "/work"
            })
            .into())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        self.input
            .drive_directories
            .iter()
            .map(|(name, directory)| (name.chars().nth(1).unwrap(), directory.clone()))
            .collect()
    }
    fn read_dir(&self, _: &Path) -> io::Result<Vec<ResourceEntry>> {
        unreachable!()
    }
    fn read_file(&self, _: &Path) -> io::Result<String> {
        unreachable!()
    }
    fn metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        unreachable!()
    }
    fn symlink_metadata(&self, _: &Path) -> io::Result<ResourceFileType> {
        unreachable!()
    }
}
impl SelfUpdateOperations for Operations<'_> {
    fn resource_operations(&self) -> &dyn ResourceOperations {
        self
    }
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
            &installation,
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
pub fn run(fixture: &str, compose_root: bool) {
    let cases: Vec<Case> = serde_json::from_str(fixture).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let (result, trace) = select(&case.input, compose_root);
        let observed = match result {
            Ok(value) => json!({"value": value}),
            Err(error) => json!({"error": error.to_string()}),
        };
        assert_eq!(observed, case.expected, "case {}", case.id);
        assert_eq!(trace, case.trace, "trace {}", case.id);
    }
}

/// Assert the owned record only after destroying every selection input.
pub fn run_owned(fixture: &str) {
    let mut cases: Vec<Case> = serde_json::from_str(fixture).expect("owned fixture parses");
    assert_eq!(cases.len(), 1);
    let Case {
        id,
        input,
        expected,
        trace,
    } = cases.remove(0);
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

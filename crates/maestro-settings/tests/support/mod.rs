//! Shared helpers for the public-interface settings tests.
//!
//! Every test binary includes this module and uses a different subset of it.

#![allow(dead_code)]

use std::future::Future;
use std::process::{Command, Output};
use std::sync::{Arc, Condvar, Mutex};

use maestro_settings::{
    InMemorySettingsStorage, Settings, SettingsManager, SettingsScope, SettingsStorage,
    SettingsStorageError, SettingsUpdate,
};
use serde_json::Value;

/// Drives a settings future on a small current-thread runtime.
pub fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Builds a settings document from JSON text.
pub fn settings(value: Value) -> Settings {
    match value {
        Value::Object(map) => Settings(map),
        other => panic!("settings must be an object, got {other}"),
    }
}

/// Seeds in-memory storage with raw text per scope.
pub fn seeded(global: Option<&str>, project: Option<&str>) -> Arc<InMemorySettingsStorage> {
    let storage = Arc::new(InMemorySettingsStorage::new());
    for (scope, text) in [
        (SettingsScope::Global, global),
        (SettingsScope::Project, project),
    ] {
        if let Some(text) = text {
            put(&*storage, scope, text);
        }
    }
    storage
}

/// Replaces a scope's raw text.
pub fn put(storage: &dyn SettingsStorage, scope: SettingsScope, text: &str) {
    storage
        .with_lock(scope, &mut |_| Ok(Some(text.to_owned())))
        .unwrap();
}

/// Reads a scope's raw text.
pub fn raw(storage: &dyn SettingsStorage, scope: SettingsScope) -> Option<String> {
    let mut text = None;
    storage
        .with_lock(scope, &mut |current| {
            text = current.map(str::to_owned);
            Ok(None)
        })
        .unwrap();
    text
}

/// Reads a scope's raw text as parsed JSON.
pub fn raw_json(storage: &dyn SettingsStorage, scope: SettingsScope) -> Value {
    raw(storage, scope).map_or(Value::Null, |text| serde_json::from_str(&text).unwrap())
}

/// A manager over fresh in-memory storage with its storage handle.
pub fn manager() -> (SettingsManager, Arc<InMemorySettingsStorage>) {
    let storage = seeded(None, None);
    (SettingsManager::from_storage(storage.clone()), storage)
}

/// Serializes a settings document the way a user file would hold it.
fn text_of(document: Value) -> String {
    serde_json::to_string(&settings(document)).unwrap()
}

/// A manager over in-memory storage seeded with a global document.
pub fn manager_with(global: Value) -> (SettingsManager, Arc<InMemorySettingsStorage>) {
    let storage = seeded(Some(&text_of(global)), None);
    (SettingsManager::from_storage(storage.clone()), storage)
}

/// A manager over in-memory storage seeded with both documents.
pub fn manager_with_both(
    global: Value,
    project: Value,
) -> (SettingsManager, Arc<InMemorySettingsStorage>) {
    let storage = seeded(Some(&text_of(global)), Some(&text_of(project)));
    (SettingsManager::from_storage(storage.clone()), storage)
}

/// Storage whose operations tests can fail, hold, script and observe.
pub struct ControlledStorage {
    inner: InMemorySettingsStorage,
    control: Mutex<Control>,
    changed: Condvar,
}

/// Whether operations pass, wait at the entry, or wait with an operation inside.
#[derive(Default, PartialEq)]
enum Gate {
    #[default]
    Open,
    Held,
    Blocked,
}

#[derive(Default)]
struct Control {
    fail_reads: bool,
    gate: Gate,
    outcomes: std::collections::VecDeque<bool>,
    calls: Vec<SettingsScope>,
    writes: Vec<(SettingsScope, String)>,
}

impl ControlledStorage {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: InMemorySettingsStorage::new(),
            control: Mutex::new(Control::default()),
            changed: Condvar::new(),
        })
    }

    fn update(&self, change: impl FnOnce(&mut Control)) {
        change(&mut self.control.lock().unwrap());
        self.changed.notify_all();
    }

    /// Fails every operation before it reads.
    pub fn fail_reads(&self, fail: bool) {
        self.update(|control| control.fail_reads = fail);
    }

    /// Scripts the next write attempts: `true` succeeds, `false` fails; later ones succeed.
    pub fn script_writes(&self, outcomes: &[bool]) {
        self.update(|control| control.outcomes = outcomes.iter().copied().collect());
    }

    /// Makes every operation wait at its entry until [`Self::release`].
    pub fn hold(&self) {
        self.update(|control| control.gate = Gate::Held);
    }

    pub fn release(&self) {
        self.update(|control| control.gate = Gate::Open);
    }

    /// Waits until an operation is waiting at the entry.
    pub fn wait_blocked(&self) {
        let mut control = self.control.lock().unwrap();
        while control.gate != Gate::Blocked {
            control = self.changed.wait(control).unwrap();
        }
    }

    /// Waits until at least `count` writes have succeeded.
    pub fn wait_for_writes(&self, count: usize) {
        let mut control = self.control.lock().unwrap();
        while control.writes.len() < count {
            control = self.changed.wait(control).unwrap();
        }
    }

    /// Replaces a scope's text behind the storage's back, as another process would.
    pub fn inject(&self, scope: SettingsScope, text: &str) {
        put(&self.inner, scope, text);
    }

    /// A scope's current text, bypassing every control.
    pub fn text(&self, scope: SettingsScope) -> Option<String> {
        raw(&self.inner, scope)
    }

    /// Every successful write in order.
    pub fn writes(&self) -> Vec<(SettingsScope, String)> {
        self.control.lock().unwrap().writes.clone()
    }

    /// The scope of every operation that got past the gate, in order.
    pub fn calls(&self) -> Vec<SettingsScope> {
        self.control.lock().unwrap().calls.clone()
    }

    /// Waits at the gate, then reports whether the operation should fail and records it.
    fn enter(&self, scope: SettingsScope) -> bool {
        let mut control = self.control.lock().unwrap();
        while control.gate != Gate::Open {
            control.gate = Gate::Blocked;
            self.changed.notify_all();
            control = self.changed.wait(control).unwrap();
        }
        control.calls.push(scope);
        control.fail_reads
    }

    /// Records a write attempt and reports whether it succeeds.
    fn attempt_write(&self, scope: SettingsScope, text: &str) -> bool {
        let mut control = self.control.lock().unwrap();
        let succeeds = control.outcomes.pop_front().unwrap_or(true);
        if succeeds {
            control.writes.push((scope, text.to_owned()));
            self.changed.notify_all();
        }
        succeeds
    }
}

impl SettingsStorage for ControlledStorage {
    fn with_lock(
        &self,
        scope: SettingsScope,
        update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        if self.enter(scope) {
            return Err("controlled failure".into());
        }
        let current = raw(&self.inner, scope);
        let Some(next) = update(current.as_deref())? else {
            return Ok(());
        };
        if !self.attempt_write(scope, &next) {
            return Err("controlled failure".into());
        }
        put(&self.inner, scope, &next);
        Ok(())
    }
}

/// Asserts two numbers are identical, including the sign of zero.
pub fn assert_number(actual: f64, expected: f64) {
    assert_eq!(
        actual.to_bits(),
        expected.to_bits(),
        "{actual} is not {expected}"
    );
}

/// Environment variable that marks a re-run of the test binary as a child.
const CHILD_MARKER: &str = "MAESTRO_SETTINGS_TEST_CHILD";

/// The case a child process was started for; `None` in the parent test run.
pub fn child_case() -> Option<String> {
    std::env::var(CHILD_MARKER).ok()
}

/// Builds a command that re-runs one test of this binary as a child for `case`.
pub fn child_command(test: &str, case: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("tests::{test}"),
            "--nocapture",
            "--quiet",
            "--test-threads=1",
        ])
        .env(CHILD_MARKER, case);
    command
}

/// The line a child printed with [`report`], parsed as JSON.
pub fn child_report(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find_map(|line| line.strip_prefix("REPORT:"));
    let line = line.unwrap_or_else(|| {
        panic!(
            "no report in child output\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    serde_json::from_str(line).unwrap()
}

/// Prints the child's result for the parent to read.
pub fn report(value: &Value) {
    println!("REPORT:{value}");
}

/// A uniquely named directory below the system temporary directory, removed on drop.
pub struct TempDir(std::path::PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("maestro-settings-{}-{number}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Environment variable naming the file a lock child works on.
const CHILD_PATH: &str = "MAESTRO_SETTINGS_TEST_PATH";

/// Child side of the lock helpers; returns `true` when this process was a lock child.
pub fn run_lock_child() -> bool {
    use std::io::Read;
    let Some(case) = child_case() else {
        return false;
    };
    let path = std::env::var_os(CHILD_PATH).unwrap();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .unwrap();
    match case.as_str() {
        "hold" => {
            file.try_lock().unwrap();
            report(&Value::String("locked".into()));
            let _ = std::io::stdin().read_to_end(&mut Vec::new());
        }
        "probe" => {
            let state = if file.try_lock().is_ok() {
                "free"
            } else {
                "blocked"
            };
            report(&Value::String(state.into()));
        }
        _ => return false,
    }
    true
}

/// A child process holding an exclusive lock until it is released.
pub struct Holder {
    child: std::process::Child,
    _output: std::io::BufReader<std::process::ChildStdout>,
}

impl Holder {
    /// Starts a child that locks `path` and waits until it reports.
    pub fn start(test: &str, path: &std::path::Path) -> Self {
        use std::io::{BufRead, BufReader};
        let mut command = child_command(test, "hold");
        let mut child = command
            .env(CHILD_PATH, path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        while line.trim_end() != "REPORT:\"locked\"" {
            line.clear();
            assert!(
                output.read_line(&mut line).unwrap() > 0,
                "the child never locked {}",
                path.display()
            );
        }
        Self {
            child,
            _output: output,
        }
    }

    /// Lets the child exit and releases its lock.
    pub fn release(mut self) {
        drop(self.child.stdin.take());
        self.child.wait().unwrap();
    }
}

/// Whether another process can lock `path` right now, as `"free"` or `"blocked"`.
pub fn probe_lock(test: &str, path: &std::path::Path) -> String {
    let output = child_command(test, "probe")
        .env(CHILD_PATH, path)
        .output()
        .unwrap();
    child_report(&output).as_str().unwrap().to_owned()
}

/// A storage adapter that scenarios seed and edit from outside the manager.
pub enum Backend {
    Memory(Arc<InMemorySettingsStorage>),
    Native(TempDir, Arc<maestro_settings::FileSettingsStorage>),
    Controlled(Arc<ControlledStorage>),
}

impl Backend {
    /// One backend per adapter.
    pub fn all() -> Vec<Self> {
        let root = TempDir::new();
        let native = Arc::new(maestro_settings::FileSettingsStorage::new(
            &root.path().join("work"),
            &root.path().join("agent"),
            std::ffi::OsStr::new(".maestro"),
        ));
        vec![
            Self::Memory(Arc::new(InMemorySettingsStorage::new())),
            Self::Native(root, native),
            Self::Controlled(ControlledStorage::new()),
        ]
    }

    /// The storage the manager is built over.
    pub fn handle(&self) -> maestro_settings::SettingsStorageHandle {
        match self {
            Self::Memory(storage) => storage.clone(),
            Self::Native(_, storage) => storage.clone(),
            Self::Controlled(storage) => storage.clone(),
        }
    }

    fn file(root: &TempDir, scope: SettingsScope) -> std::path::PathBuf {
        match scope {
            SettingsScope::Global => root.path().join("agent/settings.json"),
            SettingsScope::Project => root.path().join("work/.maestro/settings.json"),
        }
    }

    /// Replaces a scope's text as another process would.
    pub fn write(&self, scope: SettingsScope, text: &str) {
        match self {
            Self::Memory(storage) => put(&**storage, scope, text),
            Self::Native(root, _) => {
                let file = Self::file(root, scope);
                std::fs::create_dir_all(file.parent().unwrap()).unwrap();
                std::fs::write(file, text).unwrap();
            }
            Self::Controlled(storage) => storage.inject(scope, text),
        }
    }

    /// A scope's current text.
    pub fn text(&self, scope: SettingsScope) -> Option<String> {
        match self {
            Self::Memory(storage) => raw(&**storage, scope),
            Self::Native(root, _) => std::fs::read_to_string(Self::file(root, scope)).ok(),
            Self::Controlled(storage) => storage.text(scope),
        }
    }

    /// A scope's current text parsed as JSON; `null` when it was never written.
    pub fn json(&self, scope: SettingsScope) -> Value {
        self.text(scope)
            .map_or(Value::Null, |text| serde_json::from_str(&text).unwrap())
    }
}

/// Like [`child_command`], but starts the child through bash after running `setup`.
#[cfg(unix)]
pub fn shell_child_command(test: &str, case: &str, setup: &str) -> Command {
    let mut command = Command::new("bash");
    command
        .arg("-c")
        .arg(format!("{setup}; exec \"$0\" \"$@\""))
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("tests::{test}"),
            "--nocapture",
            "--quiet",
            "--test-threads=1",
        ])
        .env(CHILD_MARKER, case);
    command
}

//! Controlled package operations and settings ownership.
use maestro_packages::{
    CommandOutput, DefaultPackageManager, PackageFuture, PackageManagerOptions, PackageOperations,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::Value;
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc};

/// Controlled effects observed by the public caller.
#[derive(Default)]
pub struct Effects {
    /// Optional resource adapter for discovery witnesses.
    pub resources: Option<Rc<dyn maestro_resources::skills::ResourceOperations>>,
    /// Use actual existence observations at installed lookup.
    pub native_exists: bool,
    /// Complete command observations.
    pub transcript: RefCell<Vec<Value>>,
    /// Version file observations.
    pub file_reads: RefCell<Vec<String>>,
    /// Offline decision reads.
    pub offline_reads: std::cell::Cell<usize>,
    /// Supplied temporary root.
    pub temp_root: RefCell<Option<String>>,
    /// The runtime that admits this adapter's owned work.
    pub local: RefCell<std::rc::Weak<tokio::task::LocalSet>>,
    /// Manifest text returned at the file seam.
    pub manifest: RefCell<Option<String>>,
    /// Captured responses, distinct from synchronous root lookup.
    pub capture_outputs: RefCell<VecDeque<io::Result<String>>>,
    /// Captured working directory, deadline and environment operands.
    pub operands: RefCell<Vec<CaptureOperands>>,
    /// An optional stateful capture callback.
    pub capture_hook: RefCell<Option<CaptureHook>>,
    /// Offline observations in call order, followed by the retained value.
    pub offline_sequence: RefCell<VecDeque<Option<String>>>,
    /// Retained offline environment value.
    pub offline: RefCell<Option<String>>,
    /// Optional inherited-command behavior.
    pub run_hook: RefCell<Option<RunHook>>,
    /// The admission index at which spawning fails.
    pub fail_spawn_at: std::cell::Cell<Option<usize>>,
    /// Admission count.
    pub spawns: std::cell::Cell<usize>,
    /// Command invocations.
    pub calls: RefCell<Vec<(String, Vec<String>)>>,
    /// A one-shot settings edit during completed root lookup.
    pub on_command: RefCell<Option<Box<dyn FnOnce()>>>,
    /// Whether controlled file creation fails.
    pub fail_write: std::cell::Cell<bool>,
    /// Authored file writes at the filesystem boundary.
    pub writes: RefCell<Vec<(String, String)>>,
    /// Paths checked for existing contents.
    pub paths: RefCell<Vec<String>>,
    /// Responses consumed by commands.
    pub outputs: RefCell<VecDeque<io::Result<CommandOutput>>>,
    /// Whether contents exist.
    pub exists: std::cell::Cell<bool>,
    /// Path prefixes whose contents are absent even when `exists` is set.
    pub absent_under: RefCell<Vec<String>>,
    /// Demand-driven ambient reads.
    pub reads: RefCell<Vec<&'static str>>,
    /// Whether ambient reads fail.
    pub fail_ambient: std::cell::Cell<bool>,
    /// An authored home operand for context-read witnesses.
    pub home_override: RefCell<Option<String>>,
}
/// Observed cwd, deadline and environment operands.
pub type CaptureOperands = (
    Option<String>,
    Option<std::time::Duration>,
    Vec<(String, String)>,
);
/// A stateful callback that returns owned inherited-command work.
pub type RunHook = Rc<dyn Fn(&str, &[String], Option<&str>) -> PackageFuture<'static, Option<i32>>>;
/// A stateful callback that returns owned captured work.
pub type CaptureHook = Rc<dyn Fn(&str, &[String], Option<&str>) -> PackageFuture<'static, String>>;
/// Replaceable adapter sharing its observations with the caller.
#[derive(Clone)]
pub struct Controlled(pub Rc<Effects>);
impl PackageOperations for Controlled {
    fn resource_operations(&self) -> &dyn maestro_resources::skills::ResourceOperations {
        self.0
            .resources
            .as_deref()
            .unwrap_or(&maestro_resources::NativeResourceOperations)
    }
    fn read_file(&self, path: &str) -> io::Result<String> {
        self.0.file_reads.borrow_mut().push(path.into());
        if self.0.native_exists {
            return std::fs::read_to_string(path);
        }
        self.0
            .manifest
            .borrow()
            .clone()
            .ok_or_else(|| io::Error::other("missing manifest"))
    }
    fn temp_dir(&self) -> String {
        self.0.reads.borrow_mut().push("temp");
        self.0
            .temp_root
            .borrow()
            .clone()
            .unwrap_or_else(|| "/temporary".into())
    }
    fn offline_value(&self) -> Option<String> {
        self.0.offline_reads.set(self.0.offline_reads.get() + 1);
        self.0
            .offline_sequence
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| self.0.offline.borrow().clone())
    }
    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> io::Result<()> {
        let local = self
            .0
            .local
            .borrow()
            .upgrade()
            .ok_or_else(|| io::Error::other("stopped runtime"))?;
        let index = self.0.spawns.get();
        if self.0.fail_spawn_at.get() == Some(index) {
            return Err(io::Error::other("admission failed"));
        }
        self.0.spawns.set(index + 1);
        drop(local.spawn_local(operation));
        Ok(())
    }
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: maestro_packages::CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        self.0
            .calls
            .borrow_mut()
            .push((command.into(), args.to_vec()));
        self.0.operands.borrow_mut().push((
            options.cwd.map(str::to_owned),
            options.timeout,
            options
                .env
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        ));
        let mut call = serde_json::json!({"mode":"capture","command":command,"args":args});
        if let Some(cwd) = options.cwd {
            call["cwd"] = cwd.into();
        }
        if let Some(timeout) = options.timeout {
            call["timeoutMs"] = serde_json::json!(timeout.as_millis());
        }
        if !options.env.is_empty() {
            call["env"] = serde_json::json!(
                options
                    .env
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        self.0.transcript.borrow_mut().push(call);
        let hook = self.0.capture_hook.borrow().clone();
        if let Some(hook) = hook {
            return hook(command, args, options.cwd);
        }
        let result = self
            .0
            .capture_outputs
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Ok("\"2\"".into()));
        Box::pin(async move { result })
    }
    fn exists(&self, path: &str) -> bool {
        self.0.paths.borrow_mut().push(path.into());
        if self.0.native_exists {
            return self
                .resource_operations()
                .exists(std::path::Path::new(path));
        }
        self.0.exists.get()
            && !self
                .0
                .absent_under
                .borrow()
                .iter()
                .any(|prefix| path.starts_with(prefix.as_str()))
    }
    fn home_dir(&self) -> io::Result<String> {
        self.0.reads.borrow_mut().push("home");
        if let Some(home) = self.0.home_override.borrow().as_ref().cloned() {
            return Ok(home);
        }
        if self.0.fail_ambient.get() {
            Err(io::Error::other("home unavailable"))
        } else {
            Ok("/home/fake".into())
        }
    }
    fn current_dir(&self) -> io::Result<String> {
        self.0.reads.borrow_mut().push("cwd");
        if self.0.fail_ambient.get() {
            Err(io::Error::other("cwd unavailable"))
        } else {
            Ok("/ambient".into())
        }
    }
    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }
    fn create_dir_all(&self, path: &str) -> io::Result<()> {
        if self.0.native_exists {
            return std::fs::create_dir_all(path);
        }
        Ok(())
    }
    fn write_file(&self, path: &str, text: &str, exclusive: bool) -> io::Result<()> {
        if self.0.fail_write.get() {
            return Err(io::Error::other("file write failed"));
        }
        self.0.writes.borrow_mut().push((path.into(), text.into()));
        if self.0.native_exists {
            return maestro_packages::NativePackageOperations::new(
                |_| false,
                Rc::new(|| false),
                &Rc::new(tokio::task::LocalSet::new()),
            )
            .write_file(path, text, exclusive);
        }
        Ok(())
    }
    fn directory_is_empty(&self, _path: &str) -> io::Result<bool> {
        Ok(true)
    }
    fn remove_path(&self, _path: &str) -> io::Result<()> {
        Ok(())
    }
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        self.0
            .calls
            .borrow_mut()
            .push((command.into(), args.to_vec()));
        let mut call = serde_json::json!({"mode":"run","command":command,"args":args});
        if let Some(cwd) = cwd {
            call["cwd"] = cwd.into();
        }
        self.0.transcript.borrow_mut().push(call);
        let hook = self.0.run_hook.borrow().clone();
        if let Some(hook) = hook {
            return hook(command, args, cwd);
        }
        Box::pin(async { Ok(Some(0)) })
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        self.0
            .calls
            .borrow_mut()
            .push((command.into(), args.to_vec()));
        self.0
            .transcript
            .borrow_mut()
            .push(serde_json::json!({"mode":"sync","command":command,"args":args}));
        let callback = self.0.on_command.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
        self.0
            .outputs
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Ok(output("/root", "", Some(0))))
    }
}
/// Captured response for a controlled child.
pub fn output(stdout: &str, stderr: &str, status: Option<i32>) -> CommandOutput {
    CommandOutput {
        stdout: stdout.into(),
        stderr: stderr.into(),
        status,
    }
}
/// The manager and its surviving aliases.
pub fn manager(
    global: &Value,
) -> (
    DefaultPackageManager<Controlled>,
    Rc<RefCell<SettingsManager>>,
    Rc<Effects>,
) {
    assert!(global.is_object());
    let settings = Rc::new(RefCell::new(SettingsManager::in_memory(Settings(
        global.as_object().cloned().unwrap_or_default(),
    ))));
    let effects = Rc::new(Effects::default());
    *effects.manifest.borrow_mut() = Some("{\"version\":\"1\"}".into());
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/work/project".into(),
            agent_dir: "/home/reader/agent".into(),
            settings_manager: settings.clone(),
        },
        Controlled(effects.clone()),
    );
    (manager, settings, effects)
}

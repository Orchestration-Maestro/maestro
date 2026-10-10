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
    /// Admission count.
    pub spawns: std::cell::Cell<usize>,
    /// Command invocations.
    pub calls: RefCell<Vec<(String, Vec<String>)>>,
    /// A one-shot settings edit during completed root lookup.
    pub on_command: RefCell<Option<Box<dyn FnOnce()>>>,
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
/// A stateful callback that returns owned captured work.
pub type CaptureHook = Rc<dyn Fn(&str, &[String], Option<&str>) -> PackageFuture<'static, String>>;
/// Replaceable adapter sharing its observations with the caller.
#[derive(Clone)]
pub struct Controlled(pub Rc<Effects>);
impl PackageOperations for Controlled {
    fn read_file(&self, _path: &str) -> io::Result<String> {
        self.0
            .manifest
            .borrow()
            .clone()
            .ok_or_else(|| io::Error::other("missing manifest"))
    }
    fn offline_value(&self) -> Option<String> {
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
        self.0.spawns.set(self.0.spawns.get() + 1);
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
    fn create_dir_all(&self, _path: &str) -> io::Result<()> {
        Ok(())
    }
    fn write_file(&self, _path: &str, _text: &str) -> io::Result<()> {
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
        _command: &'a str,
        _args: &'a [String],
        _cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        Box::pin(async { Ok(Some(0)) })
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        self.0
            .calls
            .borrow_mut()
            .push((command.into(), args.to_vec()));
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

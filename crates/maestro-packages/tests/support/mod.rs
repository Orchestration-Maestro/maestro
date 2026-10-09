//! Controlled package operations and settings ownership.
use maestro_packages::{
    CommandOutput, DefaultPackageManager, PackageManagerOptions, PackageOperations,
};
use maestro_settings::{Settings, SettingsManager};
use serde_json::Value;
use std::{cell::RefCell, collections::VecDeque, io, rc::Rc};

/// Controlled effects observed by the public caller.
#[derive(Default)]
pub struct Effects {
    /// Commands after child completion.
    pub calls: RefCell<Vec<(String, Vec<String>)>>,
    /// A one-shot settings edit during completed root lookup.
    pub on_command: RefCell<Option<Box<dyn FnOnce()>>>,
    /// Paths checked for existing contents.
    pub paths: RefCell<Vec<String>>,
    /// Responses consumed by commands.
    pub outputs: RefCell<VecDeque<io::Result<CommandOutput>>>,
    /// Whether contents exist.
    pub exists: std::cell::Cell<bool>,
    /// Demand-driven ambient reads.
    pub reads: RefCell<Vec<&'static str>>,
    /// Whether ambient reads fail.
    pub fail_ambient: std::cell::Cell<bool>,
    /// An authored home operand for context-read witnesses.
    pub home_override: RefCell<Option<String>>,
}
/// Replaceable adapter sharing its observations with the caller.
#[derive(Clone)]
pub struct Controlled(pub Rc<Effects>);
impl PackageOperations for Controlled {
    fn exists(&self, path: &str) -> bool {
        self.0.paths.borrow_mut().push(path.into());
        self.0.exists.get()
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

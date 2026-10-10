//! A disposable native directory with scripted child processes and injected failures.
#![cfg(test)]
use maestro_packages::{
    CommandOutput, DefaultPackageManager, NativePackageOperations, PackageFuture,
    PackageManagerOptions, PackageOperations, ProgressCallback, ProgressEvent,
};
use maestro_settings::{Settings, SettingsManager, SettingsStorageHandle};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    path::PathBuf,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// One child launch requested of the scripted adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    /// The program.
    pub command: String,
    /// The arguments.
    pub args: Vec<String>,
    /// The requested working directory.
    pub cwd: Option<String>,
}
/// A filesystem effect that can be made to fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `create_dir_all`.
    Create,
    /// `write_file`.
    Write,
    /// `directory_is_empty`.
    Read,
    /// `remove_path`.
    Remove,
}
/// A one-shot action run when a child is admitted.
pub type Hook = Box<dyn FnOnce(&Call)>;
/// Observations and instructions shared between a test and its adapter.
#[derive(Default)]
pub struct Script {
    /// Child launches in order.
    pub calls: RefCell<Vec<Call>>,
    /// Exit results consumed per launch; success when exhausted.
    pub results: RefCell<VecDeque<io::Result<Option<i32>>>>,
    /// Actions consumed per launch, before the child can finish.
    pub hooks: RefCell<VecDeque<Hook>>,
    /// Keeps admitted children pending while set.
    pub held: Cell<bool>,
    /// Filesystem effects that fail when the path ends with the suffix.
    pub failures: RefCell<Vec<(Op, String)>>,
    /// Runs once after a successful removal.
    pub after_remove: RefCell<Option<AfterRemove>>,
    /// Makes ambient directory reads fail.
    pub ambient_fails: Cell<bool>,
    /// Ambient directory reads in order.
    pub reads: RefCell<Vec<&'static str>>,
    /// Captured-command launches in order.
    pub captures: RefCell<Vec<Call>>,
    /// Runs once at the first captured command.
    pub on_capture: RefCell<Option<Box<dyn FnOnce()>>>,
}
/// A one-shot action run after a path is removed.
pub type AfterRemove = Box<dyn FnOnce(&str)>;
/// Native files with scripted children.
pub struct Sandbox {
    /// Shared instructions and observations.
    script: Rc<Script>,
    /// Real filesystem effects.
    native: NativePackageOperations,
    /// The home reported to callers.
    home: String,
}
impl Script {
    /// Pends while children are held.
    fn poll_gate(&self, cx: &Context<'_>) -> Poll<()> {
        if !self.held.get() {
            return Poll::Ready(());
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
impl Sandbox {
    /// Fails with the injected error when this effect is scripted to fail.
    fn check(&self, op: Op, path: &str) -> io::Result<()> {
        let failing = self
            .script
            .failures
            .borrow()
            .iter()
            .any(|(failing, suffix)| *failing == op && path.ends_with(suffix.as_str()));
        if failing {
            Err(io::Error::other(format!("injected {op:?} failure")))
        } else {
            Ok(())
        }
    }
    /// Reports an ambient read or its injected failure.
    fn ambient(&self, name: &'static str, value: &str) -> io::Result<String> {
        self.script.reads.borrow_mut().push(name);
        if self.script.ambient_fails.get() {
            Err(io::Error::other(format!("{name} unavailable")))
        } else {
            Ok(value.to_owned())
        }
    }
}
impl PackageOperations for Sandbox {
    fn exists(&self, path: &str) -> bool {
        self.native.exists(path)
    }
    fn home_dir(&self) -> io::Result<String> {
        self.ambient("home", &self.home)
    }
    fn current_dir(&self) -> io::Result<String> {
        let ambient = std::env::current_dir()?;
        self.ambient("cwd", &ambient.to_string_lossy())
    }
    fn drive_directory(&self, _drive: char) -> Option<String> {
        None
    }
    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        self.script.captures.borrow_mut().push(Call {
            command: command.into(),
            args: args.to_vec(),
            cwd: None,
        });
        let hook = self.script.on_capture.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(CommandOutput {
            status: Some(0),
            stdout: "/captured/root\n".into(),
            stderr: String::new(),
        })
    }
    fn create_dir_all(&self, path: &str) -> io::Result<()> {
        self.check(Op::Create, path)?;
        self.native.create_dir_all(path)
    }
    fn write_file(&self, path: &str, text: &str) -> io::Result<()> {
        self.check(Op::Write, path)?;
        self.native.write_file(path, text)
    }
    fn directory_is_empty(&self, path: &str) -> io::Result<bool> {
        self.check(Op::Read, path)?;
        self.native.directory_is_empty(path)
    }
    fn remove_path(&self, path: &str) -> io::Result<()> {
        self.check(Op::Remove, path)?;
        self.native.remove_path(path)?;
        let after = self.script.after_remove.borrow_mut().take();
        if let Some(after) = after {
            after(path);
        }
        Ok(())
    }
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        Box::pin(async move {
            let call = Call {
                command: command.into(),
                args: args.to_vec(),
                cwd: cwd.map(str::to_owned),
            };
            self.script.calls.borrow_mut().push(call.clone());
            let hook = self.script.hooks.borrow_mut().pop_front();
            if let Some(hook) = hook {
                hook(&call);
            }
            std::future::poll_fn(|cx| self.script.poll_gate(cx)).await;
            self.script
                .results
                .borrow_mut()
                .pop_front()
                .unwrap_or(Ok(Some(0)))
        })
    }
}

/// One exclusively-created disposable directory, removed on drop.
pub struct Scratch(pub PathBuf);
impl Scratch {
    /// Creates a fresh directory with an unused name.
    pub fn new() -> io::Result<Self> {
        let name = format!(
            "maestro-acquire-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}

/// The shared progress log.
pub type Events = Rc<RefCell<Vec<ProgressEvent>>>;
/// A callback that appends every event it receives.
pub fn recorder(events: &Events) -> ProgressCallback {
    let events = events.clone();
    Rc::new(move |event| {
        events.borrow_mut().push(event.clone());
        Ok(())
    })
}

/// A manager over a disposable tree with scripted children and a recording callback.
pub struct Fixture {
    /// The disposable root.
    pub scratch: Scratch,
    /// The manager under test.
    pub manager: Rc<DefaultPackageManager<Sandbox>>,
    /// The live settings owner.
    pub settings: Rc<RefCell<SettingsManager>>,
    /// Instructions and observations.
    pub script: Rc<Script>,
    /// Events received by the default recording callback.
    pub events: Events,
}
impl Fixture {
    /// Seeds in-memory global settings.
    pub fn new(global: &Value) -> Self {
        let settings =
            SettingsManager::in_memory(Settings(global.as_object().cloned().unwrap_or_default()));
        Self::over(settings, None, false)
    }
    /// Uses the supplied settings, an optional input base and an optionally relative agent directory.
    pub fn over(settings: SettingsManager, cwd: Option<&str>, relative_agent: bool) -> Self {
        let scratch = Scratch::new().unwrap();
        let root = scratch.0.to_string_lossy().into_owned();
        let settings = Rc::new(RefCell::new(settings));
        let script = Rc::new(Script::default());
        let operations = Sandbox {
            script: script.clone(),
            native: NativePackageOperations::new(|_| false, Rc::new(|| false)),
            home: format!("{root}/home"),
        };
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: cwd.map_or_else(|| format!("{root}/project"), str::to_owned),
                agent_dir: if relative_agent {
                    let ambient = std::env::current_dir()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    maestro_path::relative(
                        &ambient,
                        &format!("{root}/agent"),
                        &maestro_path::Cwd {
                            current: "",
                            drive_directories: &[],
                        },
                    )
                } else {
                    format!("{root}/agent")
                },
                settings_manager: settings.clone(),
            },
            operations,
        ));
        let events = Events::default();
        maestro_packages::PackageManager::set_progress_callback(&*manager, Some(recorder(&events)));
        Self {
            scratch,
            manager,
            settings,
            script,
            events,
        }
    }
    /// Uses settings loaded from the supplied storage.
    pub fn with_storage(storage: SettingsStorageHandle) -> Self {
        Self::over(SettingsManager::from_storage(storage), None, false)
    }
    /// Uses an agent directory written relative to the process working directory.
    pub fn relative_agent() -> Self {
        Self::over(SettingsManager::in_memory(Settings::default()), None, true)
    }
    /// An absolute path below the disposable root.
    pub fn path(&self, relative: &str) -> String {
        format!("{}/{relative}", self.scratch.0.to_string_lossy())
    }
    /// The project input base.
    pub fn project(&self, relative: &str) -> String {
        self.path(&format!("project/{relative}"))
    }
    /// The user agent directory.
    pub fn agent(&self, relative: &str) -> String {
        self.path(&format!("agent/{relative}"))
    }
    /// Waits for every settings write queued so far.
    pub fn flush(&self) {
        let flushed = self.settings.borrow().flush();
        block_on(Box::pin(async move {
            flushed.await;
            Ok(())
        }))
        .unwrap();
    }
    /// Launches seen so far.
    pub fn calls(&self) -> Vec<Call> {
        self.script.calls.borrow().clone()
    }
    /// The recorded `(phase, message)` pairs.
    pub fn phases(&self) -> Vec<(maestro_packages::ProgressEventType, Option<String>)> {
        self.events
            .borrow()
            .iter()
            .map(|event| (event.r#type, event.message.clone()))
            .collect()
    }
}

/// Polls the future to completion; a held child must be released by another poll source.
pub fn block_on<T>(mut future: PackageFuture<'_, T>) -> io::Result<T> {
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}
/// Polls once, returning the output when the future is already ready.
pub fn poll_once<T>(future: &mut PackageFuture<'_, T>) -> Option<io::Result<T>> {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => Some(value),
        Poll::Pending => None,
    }
}

/// Creates a file with its parents.
pub fn write(path: &str, text: &str) {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
/// Reads a file's text.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap()
}

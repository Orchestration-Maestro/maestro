//! A disposable native directory with scripted child processes.
#![cfg(test)]
use crate::native_support;
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
    path::Path,
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
    /// Captured-command launches in order.
    pub captures: RefCell<Vec<Call>>,
    /// Runs once at the first captured command.
    pub on_capture: RefCell<Option<Box<dyn FnOnce()>>>,
}
/// Native files with scripted children.
pub struct Sandbox {
    /// Shared instructions and observations.
    script: Rc<Script>,
    /// Real spawning and captured commands.
    native: NativePackageOperations,
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
impl PackageOperations for Sandbox {
    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> std::io::Result<()> {
        self.native.spawn(operation)
    }
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        options: maestro_packages::CommandCaptureOptions<'a>,
    ) -> maestro_packages::PackageFuture<'a, String> {
        self.native.run_command_capture(command, args, options)
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

pub use super::scratch::Scratch;

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
    /// Caller-driven work owner, retained through every operation.
    pub local: Rc<tokio::task::LocalSet>,
    /// Events received by the default recording callback.
    pub events: Events,
}
impl Fixture {
    /// Seeds in-memory global settings.
    #[must_use]
    pub fn new(global: &Value) -> Self {
        let settings =
            SettingsManager::in_memory(Settings(global.as_object().cloned().unwrap_or_default()));
        Self::over(settings, None, false)
    }
    /// Uses the supplied settings, an optional input base and an optionally relative agent directory.
    /// # Panics
    /// Panics when the disposable directory is unavailable, or when `relative_agent` is true
    /// and the ambient working directory is unavailable.
    pub fn over(settings: SettingsManager, cwd: Option<&str>, relative_agent: bool) -> Self {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path().to_path_buf();
        let settings = Rc::new(RefCell::new(settings));
        let script = Rc::new(Script::default());
        let local = Rc::new(tokio::task::LocalSet::new());
        let operations = Sandbox {
            script: script.clone(),
            native: NativePackageOperations::new(Rc::new(|| false), &local),
        };
        let manager = Rc::new(DefaultPackageManager::new(
            PackageManagerOptions {
                cwd: cwd.map_or_else(
                    || {
                        native_support::under(&root, "project")
                            .to_string_lossy()
                            .into_owned()
                    },
                    str::to_owned,
                ),
                agent_dir: if relative_agent {
                    let ambient = std::env::current_dir()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    maestro_path::relative(
                        &ambient,
                        &native_support::under(&root, "agent").to_string_lossy(),
                        &maestro_path::Cwd {
                            current: "",
                            drive_directories: &[],
                        },
                    )
                } else {
                    native_support::under(&root, "agent")
                        .to_string_lossy()
                        .into_owned()
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
            local,
            events,
        }
    }
    /// Uses settings loaded from the supplied storage.
    pub fn with_storage(storage: SettingsStorageHandle) -> Self {
        Self::over(SettingsManager::from_storage(storage), None, false)
    }
    /// Uses an agent directory written relative to the process working directory.
    #[must_use]
    pub fn relative_agent() -> Self {
        Self::over(SettingsManager::in_memory(Settings::default()), None, true)
    }
    /// An absolute path below the disposable root.
    #[must_use]
    pub fn path(&self, relative: &str) -> String {
        native_support::under(self.scratch.path(), relative)
            .to_string_lossy()
            .into_owned()
    }
    /// The project input base.
    #[must_use]
    pub fn project(&self, relative: &str) -> String {
        self.path(&format!("project/{relative}"))
    }
    /// The user agent directory.
    #[must_use]
    pub fn agent(&self, relative: &str) -> String {
        self.path(&format!("agent/{relative}"))
    }
    /// Waits for every settings write queued so far.
    /// # Panics
    /// Panics if driving the flush fails.
    pub fn flush(&self) {
        let flushed = self.settings.borrow().flush();
        block_on(Box::pin(async move {
            flushed.await;
            Ok(())
        }))
        .unwrap();
    }
    /// Launches seen so far.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        self.script.calls.borrow().clone()
    }
    /// The recorded `(phase, message)` pairs.
    #[must_use]
    pub fn phases(&self) -> Vec<(maestro_packages::ProgressEventType, Option<String>)> {
        self.events
            .borrow()
            .iter()
            .map(|event| (event.r#type, event.message.clone()))
            .collect()
    }
}

/// Polls the future to completion; a held child must be released by another poll source.
/// # Errors
/// Returns the future's error.
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

/// Reads a file's text.
/// # Panics
/// Panics if the file cannot be read as UTF-8.
#[must_use]
pub fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

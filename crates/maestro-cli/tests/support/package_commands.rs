//! Controlled package command harness: outputs, settings, operations and updates.
#![cfg(test)]
#![cfg(unix)]

use maestro_app::configuration::config::installation::{Installation, PackageLayout};
use maestro_app::configuration::config::{
    SelfUpdateCommand, SelfUpdateContext, SelfUpdateOperations,
    get_self_update_command_from_settings, get_self_update_unavailable_instruction_from_settings,
};
use maestro_app::configuration::version_check::VersionCheckOptions;
use maestro_app::{
    CommandCaptureOptions, CommandOutput, PackageFuture, PackageManager, PackageManagerOptions,
    PackageOperations, Settings, SettingsManager, SettingsScope, SettingsStorage,
    SettingsStorageError, SettingsUpdate, SpawnSyncError,
};
use maestro_cli::input::config_selector::ConfigPresentation;
use maestro_cli::packages::{PackageCommandContext, handle_config_command, handle_package_command};
use maestro_test_directory::Scratch;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::process::{ExitStatus, Output};
use std::rc::Rc;
use std::sync::Arc;

/// The configuration directory of the settings files.
pub const CONFIG_DIR: &str = ".maestro";

/// A shared closure run before each write to one buffer.
type Hook = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

/// A buffer that keeps every written byte and may run a hook before each write.
#[derive(Clone, Default)]
pub struct Capture {
    bytes: Rc<RefCell<Vec<u8>>>,
    hook: Hook,
}

impl Capture {
    /// The shared writer handle of this buffer.
    pub fn handle(&self) -> Rc<RefCell<dyn Write>> {
        Rc::new(RefCell::new(Append {
            bytes: Rc::clone(&self.bytes),
            hook: Rc::clone(&self.hook),
        }))
    }

    /// Run `hook` before every write to this buffer.
    pub fn set_hook(&self, hook: Rc<dyn Fn()>) {
        *self.hook.borrow_mut() = Some(hook);
    }

    /// Everything written so far.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.borrow()).into_owned()
    }

    /// Forget everything written so far.
    pub fn clear(&self) {
        self.bytes.borrow_mut().clear();
    }
}

/// Writer clone appending into one shared buffer.
struct Append {
    bytes: Rc<RefCell<Vec<u8>>>,
    hook: Hook,
}

impl Write for Append {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let hook = self.hook.borrow().clone();
        if let Some(hook) = hook {
            hook();
        }
        self.bytes.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Recorded package operations with a caller-driven admission runtime.
#[derive(Default)]
pub struct Effects {
    /// Captured and inherited commands in call order.
    pub calls: RefCell<Vec<String>>,
    /// The runtime admitting owned work.
    pub local: RefCell<std::rc::Weak<tokio::task::LocalSet>>,
    /// Answers for captured commands.
    pub captures: RefCell<VecDeque<io::Result<String>>>,
    /// The exit code of an inherited command; absent means success.
    pub inherited: Cell<Option<i32>>,
    /// A failure every captured command reports instead.
    pub refusal: RefCell<Option<String>>,
}

impl Effects {
    /// Record `mode` and the literal operands of one command.
    fn record(&self, mode: &str, command: &str, args: &[String]) {
        self.calls
            .borrow_mut()
            .push(format!("{mode} {command} {}", args.join(" ")));
    }
}

/// Package operations sharing one [`Effects`] record.
#[derive(Clone)]
pub struct Operations(Rc<Effects>);

impl PackageOperations for Operations {
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        _options: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        self.0.record("capture", command, args);
        let refusal = self.0.refusal.borrow().clone();
        let answer = self.0.captures.borrow_mut().pop_front();
        Box::pin(async move {
            match refusal {
                Some(reason) => Err(io::Error::other(reason)),
                None => answer.unwrap_or_else(|| Ok(String::new())),
            }
        })
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
        drop(local.spawn_local(operation));
        Ok(())
    }

    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        self.0.record("sync", command, args);
        Ok(CommandOutput {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        _cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        self.0.record("run", command, args);
        let answer = self.0.inherited.get();
        Box::pin(async move { Ok(answer.or(Some(0))) })
    }
}

/// A second caller-supplied adapter, distinguished in the same transcript.
#[derive(Clone)]
pub struct Second(Rc<Effects>);

impl PackageOperations for Second {
    fn run_command_capture<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        _options: CommandCaptureOptions<'a>,
    ) -> PackageFuture<'a, String> {
        self.0.record("second-capture", command, args);
        Box::pin(async { Ok(String::new()) })
    }

    fn spawn(
        &self,
        operation: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static>>,
    ) -> io::Result<()> {
        Operations(Rc::clone(&self.0)).spawn(operation)
    }

    fn run_command_sync(&self, command: &str, args: &[String]) -> io::Result<CommandOutput> {
        self.0.record("second-sync", command, args);
        Ok(CommandOutput {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        _cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        self.0.record("second-run", command, args);
        Box::pin(async { Ok(Some(0)) })
    }
}

/// Controlled read-only self-update observations and update steps.
#[derive(Default)]
pub struct Updates {
    /// Synchronous query answers, in call order.
    pub queries: RefCell<VecDeque<String>>,
    /// The native-image status of each run step.
    pub steps: RefCell<VecDeque<ExitStatus>>,
    /// Commands handed to the runner in call order.
    pub runs: RefCell<Vec<String>>,
    /// A failure every synchronous query reports instead of an answer.
    pub sync_failure: RefCell<Option<String>>,
}

/// A terminating status with `code`.
fn coded(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(code << 8)
}

impl SelfUpdateOperations for Updates {
    fn is_windows(&self) -> bool {
        false
    }

    fn run_command_sync(&self, _command: &str, _args: &[String]) -> Result<Output, SpawnSyncError> {
        if let Some(reason) = self.sync_failure.borrow().as_deref() {
            return Err(SpawnSyncError::Spawn(io::Error::other(reason.to_owned())));
        }
        let stdout = self.queries.borrow_mut().pop_front().unwrap_or_default();
        Ok(Output {
            status: coded(0),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        })
    }

    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = io::Result<ExitStatus>> + 'a>> {
        self.runs
            .borrow_mut()
            .push(format!("{command} {}", args.join(" ")));
        let status = self
            .steps
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| coded(0));
        Box::pin(async move { Ok(status) })
    }
}

/// A storage adapter whose loads fail with the supplied failures per scope.
pub struct Failing {
    /// The global failure.
    pub global: Failure,
    /// The project failure.
    pub project: Failure,
}

impl SettingsStorage for Failing {
    fn with_lock(
        &self,
        scope: SettingsScope,
        _update: &mut dyn FnMut(Option<&str>) -> SettingsUpdate,
    ) -> Result<(), SettingsStorageError> {
        let failure = match scope {
            SettingsScope::Global => self.global.clone(),
            SettingsScope::Project => self.project.clone(),
        };
        Err(Box::new(failure))
    }
}

/// A recorded failure with an optional supplied trace.
#[derive(Clone)]
pub struct Failure {
    /// The message the failure reports.
    pub message: &'static str,
    /// The optional trace reached through `source`.
    pub trace: Option<Trace>,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::fmt::Debug for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Failure")
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.trace
            .as_ref()
            .map(|trace| trace as &(dyn std::error::Error + 'static))
    }
}

/// The trace text of a [`Failure`].
#[derive(Clone)]
pub struct Trace(pub &'static str);

impl std::fmt::Display for Trace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::fmt::Debug for Trace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Trace")
    }
}

impl std::error::Error for Trace {}

/// Everything one package command observes.
/// An edit of the live settings.
type Edit = Arc<dyn Fn(&mut SettingsManager)>;

/// A response body carrying one chunk.
fn release_body(chunk: Result<Vec<u8>, maestro_app::FetchError>) -> maestro_app::HttpBody {
    Box::pin(futures_util::stream::once(async move { chunk }))
}

#[derive(Default)]
pub struct State {
    /// The package operations of the manager.
    pub effects: Rc<Effects>,
    /// The self-update observations and steps.
    pub updates: Rc<Updates>,
    /// The startup offline choice passed into newly constructed managers.
    pub offline: Cell<Option<bool>>,
    /// Settings owners handed to the manager, in call order.
    pub supplied: RefCell<Vec<Rc<RefCell<SettingsManager>>>>,
    /// Settings owners observed by the loader, in call order.
    pub loaded: RefCell<Vec<Rc<RefCell<SettingsManager>>>>,
    /// A failure the settings loader reports instead.
    pub load_failure: RefCell<Option<String>>,
    /// A failure the working-directory observation reports instead.
    pub cwd_failure: RefCell<Option<String>>,
    /// A failure the agent-directory observation reports instead.
    pub agent_failure: RefCell<Option<String>>,
    /// A failure the package-manager construction reports instead.
    pub manager_failure: RefCell<Option<String>>,
    /// Whether the factory supplies the second adapter.
    pub second: Cell<bool>,
    /// Whether the package directory of the installation is absent.
    pub absent_package: Cell<bool>,
    /// A storage adapter the loader uses instead of the settings files.
    pub storage: RefCell<Option<Arc<dyn SettingsStorage>>>,
    /// Settings owners observed by the manager factory, in call order.
    pub factory: RefCell<Vec<*const RefCell<SettingsManager>>>,
    /// An edit the manager factory makes to the live settings.
    pub edit_in_factory: RefCell<Option<Edit>>,
    /// Observations and effects of the command, in call order.
    pub steps: RefCell<Vec<&'static str>>,
}

/// Everything one package command is given.
pub struct Harness {
    /// The disposable root of every path.
    pub scratch: Scratch,
    /// The shared observations of the command.
    pub state: Rc<State>,
    /// The release request options.
    pub version_options: VersionCheckOptions,
    /// The release requests actually sent through the supplied transport.
    pub release_requests: Arc<std::sync::atomic::AtomicUsize>,
    /// The service origin of the command.
    pub origin: String,
    /// Standard output of the command.
    pub stdout: Capture,
    /// Standard error of the command.
    pub stderr: Capture,
    /// The status the command set.
    pub exit_code: Cell<Option<i32>>,
    /// The working directory the command observes.
    pub cwd: String,
    /// The agent directory the command observes.
    pub agent_dir: String,
}

impl Harness {
    /// A harness over a disposable root with the supplied global and project settings.
    pub fn new(global: Option<Value>, project: Option<Value>) -> Self {
        let scratch = Scratch::new().expect("scratch root");
        let cwd = scratch.path().join("root").to_string_lossy().into_owned();
        let agent_dir = scratch.path().join("agent").to_string_lossy().into_owned();
        std::fs::create_dir_all(&cwd).expect("working directory");
        std::fs::create_dir_all(&agent_dir).expect("agent directory");
        let prefix = scratch.path().join("prefix");
        let package = prefix.join("lib/node_modules/maestro");
        std::fs::create_dir_all(&package).expect("package directory");
        let harness = Self {
            scratch,
            state: Rc::new(State::default()),
            version_options: VersionCheckOptions::default(),
            release_requests: Arc::default(),
            origin: String::new(),
            stdout: Capture::default(),
            stderr: Capture::default(),
            exit_code: Cell::new(None),
            cwd,
            agent_dir,
        };
        harness.state.updates.queries.borrow_mut().push_back(
            prefix
                .join("lib/node_modules")
                .to_string_lossy()
                .into_owned(),
        );
        harness.write_settings(global, project);
        let steps = Rc::clone(&harness.state);
        harness
            .stderr
            .set_hook(Rc::new(move || steps.steps.borrow_mut().push("warning")));
        harness
    }

    /// Write the settings files of this harness.
    pub fn write_settings(&self, global: Option<Value>, project: Option<Value>) {
        if let Some(global) = global {
            write_json(&format!("{}/settings.json", self.agent_dir), &global);
        }
        if let Some(project) = project {
            write_json(
                &format!("{}/{CONFIG_DIR}/settings.json", self.cwd),
                &project,
            );
        }
    }

    /// Use a storage adapter instead of the settings files of this harness.
    pub fn use_storage(&self, storage: Arc<dyn SettingsStorage>) {
        *self.state.storage.borrow_mut() = Some(storage);
    }

    /// Answer the release lookup with one controlled body and status.
    pub fn release(&mut self, status: u16, body: &str) {
        use maestro_app::{Fetch, HttpResponse};
        let body = body.to_owned();
        let requests = Arc::clone(&self.release_requests);
        let fetch: Fetch = Arc::new(move |_request| {
            requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let chunk = if status == 200 {
                Ok(body.clone().into_bytes())
            } else {
                Err(maestro_app::FetchError::Aborted)
            };
            Box::pin(async move {
                Ok(HttpResponse {
                    status,
                    status_text: String::new(),
                    headers: std::collections::BTreeMap::new(),
                    body: release_body(chunk),
                })
            })
        });
        self.version_options.fetch = Some(fetch);
    }

    /// Halt the harness on a settings failure before any query is answered.
    pub fn fail_loading(&self, reason: &str) {
        *self.state.load_failure.borrow_mut() = Some(reason.to_owned());
    }

    /// Halt the harness on a working-directory failure.
    pub fn fail_cwd(&self, reason: &str) {
        *self.state.cwd_failure.borrow_mut() = Some(reason.to_owned());
    }

    /// Halt the harness on an agent-directory failure.
    pub fn fail_agent_dir(&self, reason: &str) {
        *self.state.agent_failure.borrow_mut() = Some(reason.to_owned());
    }

    /// Halt the harness on a package-manager construction failure.
    pub fn fail_manager(&self, reason: &str) {
        *self.state.manager_failure.borrow_mut() = Some(reason.to_owned());
    }

    /// The live settings owner the last command was given.
    pub fn supplied(&self) -> Rc<RefCell<SettingsManager>> {
        self.state
            .supplied
            .borrow()
            .last()
            .cloned()
            .expect("a settings owner was supplied")
    }

    /// The commands recorded by the package operations.
    pub fn calls(&self) -> Vec<String> {
        self.state.effects.calls.borrow().clone()
    }

    /// The commands handed to the self-update runner.
    pub fn runs(&self) -> Vec<String> {
        self.state.updates.runs.borrow().clone()
    }

    /// Run `args`, reporting whether the handler claimed them.
    ///
    /// # Panics
    /// Panics when the command does not complete.
    pub fn run(&self, args: &[&str]) -> bool {
        self.try_run(args).expect("the command completes")
    }

    /// Run `args`, keeping the handler's own error.
    ///
    /// # Panics
    /// Panics when the harness cannot start its runtime.
    pub fn try_run(&self, args: &[&str]) -> io::Result<bool> {
        self.run_command(args, None)
    }

    /// Run `args` as a configuration command over `presentation`.
    ///
    /// # Panics
    /// Panics when the command does not complete.
    pub fn run_config(&self, args: &[&str], presentation: ConfigPresentation) -> bool {
        self.try_run_config(args, presentation)
            .expect("the command completes")
    }

    /// Run `args` as a configuration command, keeping the handler's own error.
    ///
    /// # Panics
    /// Panics when the harness cannot start its runtime.
    pub fn try_run_config(
        &self,
        args: &[&str],
        presentation: ConfigPresentation,
    ) -> io::Result<bool> {
        self.run_command(args, Some(presentation))
    }

    /// Run one command of this harness on its own current-thread runtime.
    ///
    /// # Panics
    /// Panics when the harness cannot start its runtime.
    fn run_command(
        &self,
        args: &[&str],
        presentation: Option<ConfigPresentation>,
    ) -> io::Result<bool> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let local = Rc::new(tokio::task::LocalSet::new());
        *self.state.effects.local.borrow_mut() = Rc::downgrade(&local);
        let tokens: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        runtime.block_on(local.run_until(async { self.handle(&tokens, presentation).await }))
    }

    /// Build the context and handle `argv`.
    async fn handle(
        &self,
        argv: &[String],
        presentation: Option<ConfigPresentation>,
    ) -> io::Result<bool> {
        self.with_context(async |context| match presentation {
            Some(presentation) => handle_config_command(argv, &context, presentation).await,
            None => handle_package_command(argv, &context).await,
        })
        .await
    }

    /// Build this harness's command context and run `body` with it.
    pub async fn with_context<T>(
        &self,
        body: impl AsyncFnOnce(PackageCommandContext<'_>) -> T,
    ) -> T {
        let cwd = self.cwd.clone();
        let agent_dir = self.agent_dir.clone();
        let observe_cwd = {
            let state = Rc::clone(&self.state);
            move || {
                state.steps.borrow_mut().push("cwd");
                match state.cwd_failure.borrow().as_deref() {
                    Some(reason) => Err(io::Error::other(reason.to_owned())),
                    None => Ok(cwd.clone()),
                }
            }
        };
        let observe_agent = move || {
            self.state.steps.borrow_mut().push("agent");
            match self.state.agent_failure.borrow().as_deref() {
                Some(reason) => Err(io::Error::other(reason.to_owned())),
                None => Ok(agent_dir.clone()),
            }
        };
        let load = |cwd: &str, agent_dir: &str| self.load(cwd, agent_dir);
        let create =
            |options: PackageManagerOptions| match self.state.manager_failure.borrow().as_deref() {
                Some(reason) => Err(io::Error::other(reason.to_owned())),
                None => Ok(self.build_manager(options)),
            };
        let select = |settings: &Settings, package: &str| self.select(settings, package);
        let instruction = |settings: &Settings, package: &str| self.instruction(settings, package);
        let context = PackageCommandContext {
            cwd: &observe_cwd,
            agent_dir: &observe_agent,
            load_settings: &load,
            create_package_manager: &create,
            select_self_update_command: &select,
            self_update_unavailable_instruction: &instruction,
            self_update_operations: &*self.state.updates,
            version_options: &self.version_options,
            service_origin: &self.origin,
            stdout: self.stdout.handle(),
            stderr: self.stderr.handle(),
            exit_code: &self.exit_code,
            entrypoint: Some("/opt/maestro/bin/maestro"),
            styles_enabled: false,
        };
        body(context).await
    }

    /// Build the package manager over the settings owner of one command.
    fn build_manager(&self, options: PackageManagerOptions) -> Rc<dyn PackageManager> {
        let state = Rc::clone(&self.state);
        let effects = Rc::clone(&state.effects);
        state
            .factory
            .borrow_mut()
            .push(Rc::as_ptr(&options.settings_manager));
        state.steps.borrow_mut().push("manager");
        let edit = state.edit_in_factory.borrow().clone();
        if let Some(edit) = edit {
            edit(&mut options.settings_manager.borrow_mut());
        }
        if state.second.get() {
            let manager =
                maestro_app::DefaultPackageManager::new(options, Second(Rc::clone(&effects)))
                    .with_offline(state.offline.get());
            return Rc::new(manager) as Rc<dyn PackageManager>;
        }
        let manager =
            maestro_app::DefaultPackageManager::new(options, Operations(Rc::clone(&effects)))
                .with_offline(state.offline.get());
        Rc::new(manager) as Rc<dyn PackageManager>
    }

    /// Load the shared settings owner over `cwd` and `agent_dir`.
    fn load(&self, cwd: &str, agent_dir: &str) -> io::Result<Rc<RefCell<SettingsManager>>> {
        let state = Rc::clone(&self.state);
        if let Some(reason) = state.load_failure.borrow().as_deref() {
            return Err(io::Error::other(reason.to_owned()));
        }
        let manager = Rc::new(RefCell::new(match state.storage.borrow().clone() {
            Some(storage) => SettingsManager::from_storage(storage),
            None => SettingsManager::create(cwd, agent_dir, CONFIG_DIR),
        }));
        state.steps.borrow_mut().push("settings");
        state.loaded.borrow_mut().push(Rc::clone(&manager));
        state.supplied.borrow_mut().push(Rc::clone(&manager));
        Ok(manager)
    }

    /// The installation evidence of this harness: a global npm package.
    fn installation(&self) -> Installation {
        let package = if self.state.absent_package.get() {
            "prefix/lib/node_modules/absent"
        } else {
            "prefix/lib/node_modules/maestro"
        };
        Installation {
            module_dir: self
                .scratch
                .path()
                .join(package)
                .to_string_lossy()
                .into_owned(),
            executable: "/opt/maestro/bin/maestro".into(),
            layout: PackageLayout::Package,
        }
    }

    /// Select an update command from a retained settings snapshot.
    fn select(&self, settings: &Settings, package: &str) -> io::Result<Option<SelfUpdateCommand>> {
        let installation = self.installation();
        let directory = installation.module_dir.clone();
        let context = SelfUpdateContext {
            installation: &installation,
            package_dir: &directory,
            operations: &*self.state.updates,
        };
        get_self_update_command_from_settings(&context, settings, package)
    }

    /// Explain why this installation cannot update itself.
    fn instruction(&self, settings: &Settings, package: &str) -> io::Result<String> {
        let installation = self.installation();
        let directory = installation.module_dir.clone();
        let context = SelfUpdateContext {
            installation: &installation,
            package_dir: &directory,
            operations: &*self.state.updates,
        };
        get_self_update_unavailable_instruction_from_settings(&context, settings, package)
    }
}

/// Write one pretty-printed settings document.
fn write_json(path: &str, value: &Value) {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent).expect("settings directory");
    }
    let mut text = serde_json::to_string_pretty(value).expect("settings text");
    text.push('\n');
    std::fs::write(path, text).expect("settings file");
}

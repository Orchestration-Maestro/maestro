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
    /// Complete command observations.
    pub transcript: RefCell<Vec<Value>>,
    /// The runtime that admits this adapter's owned work.
    pub local: RefCell<std::rc::Weak<tokio::task::LocalSet>>,
    /// Captured responses, distinct from synchronous root lookup.
    pub capture_outputs: RefCell<VecDeque<io::Result<String>>>,
    /// Captured working directory, deadline and environment operands.
    pub operands: RefCell<Vec<CaptureOperands>>,
    /// An optional stateful capture callback.
    pub capture_hook: RefCell<Option<CaptureHook>>,
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
    /// Responses consumed by commands.
    pub outputs: RefCell<VecDeque<io::Result<CommandOutput>>>,
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
/// Completes one owned operation on a fresh current-thread runtime that admitted its workers,
/// then closes that runtime.
///
/// Only the unix manager tests consume it, so other hosts would see it unused.
#[cfg(unix)]
pub fn drive<T>(effects: &Rc<Effects>, operation: impl std::future::Future<Output = T>) -> T {
    let local = Rc::new(tokio::task::LocalSet::new());
    *effects.local.borrow_mut() = Rc::downgrade(&local);
    let runtime = crate::runtime::runtime();
    let result = runtime.block_on(local.run_until(operation));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
    result
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

/// Spells each real staging directory `.maestro-stage-<pid>-<n>` as the recorded
/// `.controlled-stage`.
pub fn recorded_stages(text: &str) -> String {
    const REAL: &str = ".maestro-stage-";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(REAL) {
        out.push_str(&rest[..index]);
        out.push_str(".controlled-stage");
        rest =
            rest[index + REAL.len()..].trim_start_matches(|c: char| c.is_ascii_digit() || c == '-');
    }
    out.push_str(rest);
    out
}

/// The spelling of the absolute fixture path `path` below `root`; relative paths stay as
/// they are.
pub fn below(root: &std::path::Path, path: &str) -> String {
    if path.starts_with('/') {
        // The authored separators become native ones, as the package manager joins them.
        let path = path.replace('/', std::path::MAIN_SEPARATOR_STR);
        format!("{}{path}", root.to_str().unwrap())
    } else {
        path.to_owned()
    }
}
/// A configured manager, its settings, its controlled commands and the tree it works in.
#[cfg(unix)]
pub type Configured = (
    Rc<DefaultPackageManager<Controlled>>,
    Rc<RefCell<SettingsManager>>,
    Rc<Effects>,
    super::scratch::Scratch,
);
/// A configured manager over a fresh tree where every listed npm and Git source is
/// installed, npm contents at version 1.
#[cfg(unix)]
pub fn installed(user: &Value, project: &Value) -> Configured {
    installed_with(user, project, None)
}
/// [`installed`] over the caller's startup offline selection.
#[cfg(unix)]
pub fn installed_with(user: &Value, project: &Value, offline: Option<bool>) -> Configured {
    let configured = uninstalled_with(user, project, offline);
    let tree = configured.3.path();
    let user = user.get("packages").and_then(Value::as_array);
    let scoped = user.into_iter().flatten().map(|entry| (true, entry));
    for (user, entry) in scoped.chain(project.as_array().unwrap().iter().map(|e| (false, e))) {
        let Some(source) = entry.as_str().or_else(|| entry["source"].as_str()) else {
            continue;
        };
        if let Some(spec) = source.strip_prefix("npm:") {
            let name = spec
                .rfind('@')
                .filter(|&at| at > 0)
                .map_or(spec, |at| &spec[..at]);
            super::native_support::write(
                format!("{}/{name}/package.json", npm_root(tree, user)),
                "{\"version\":\"1\"}",
            );
        } else if let Some(git) = maestro_packages::parse_git_url(source) {
            let base = if user {
                "/home/reader/agent"
            } else {
                "/work/project/.maestro"
            };
            let path = format!("{}/git/{}/{}", below(tree, base), git.host, git.path);
            std::fs::create_dir_all(path).unwrap();
        }
    }
    configured
}
/// A configured manager over a fresh empty tree.
#[cfg(unix)]
pub fn uninstalled(user: &Value, project: &Value) -> Configured {
    uninstalled_with(user, project, None)
}
/// [`uninstalled`] over the caller's startup offline selection.
#[cfg(unix)]
pub fn uninstalled_with(user: &Value, project: &Value, offline: Option<bool>) -> Configured {
    let tree = super::scratch::Scratch::new().unwrap();
    let (manager, settings, effects) = manager_below(tree.path(), user, project, offline);
    (manager, settings, effects, tree)
}
/// A configured manager over `root`, laid out as the fixture paths spell it: the project at
/// `/work/project`, the agent directory at `/home/reader/agent`, and the global npm root
/// lookup answering `/root`. `offline` is the caller's startup selection.
#[cfg(unix)]
pub fn manager_below(
    root: &std::path::Path,
    user: &Value,
    project: &Value,
    offline: Option<bool>,
) -> (
    Rc<DefaultPackageManager<Controlled>>,
    Rc<RefCell<SettingsManager>>,
    Rc<Effects>,
) {
    let (_, settings, effects) = manager(user);
    settings.borrow_mut().set_project_packages(
        project
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(maestro_settings::PackageSource::Unknown)
            .collect(),
    );
    let global = output(&format!("{}\n", below(root, "/root")), "", Some(0));
    effects.outputs.borrow_mut().push_back(Ok(global));
    let manager = DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: below(root, "/work/project"),
            agent_dir: below(root, "/home/reader/agent"),
            settings_manager: settings.clone(),
        },
        Controlled(effects.clone()),
    )
    .with_offline(offline);
    (Rc::new(manager), settings, effects)
}
/// The npm root holding a scope's installed packages in a configured tree.
#[cfg(unix)]
pub fn npm_root(tree: &std::path::Path, user: bool) -> String {
    let root = if user {
        "/root"
    } else {
        "/work/project/.maestro/npm/node_modules"
    };
    below(tree, root)
}
/// Writes the manifest of the project's installed npm package `name`; `None` removes it.
#[cfg(unix)]
pub fn project_manifest(tree: &super::scratch::Scratch, name: &str, text: Option<&str>) {
    let path = format!("{}/{name}/package.json", npm_root(tree.path(), false));
    match text {
        Some(text) => std::fs::write(path, text).unwrap(),
        None => std::fs::remove_file(path).unwrap(),
    }
}

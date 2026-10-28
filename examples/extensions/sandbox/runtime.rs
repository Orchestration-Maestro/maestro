//! Initialization and cleanup of the example's platform resources.
use super::{
    config::{FilesystemConfig, SandboxConfig},
    platform::command_output,
    platform_commands::TEMPORARY,
    proxies::Proxies,
    seccomp,
};
use maestro_extensions_wasm::{
    Background, ExtensionAPI,
    child_process::{ChildProcess, SpawnOptions, Stdio},
    tools::path_utils::{expand_path, guest_path, make_temp_dir, resolve},
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};

/// Immutable operands for wrapping one command; process execution stays in the native factory.
pub(crate) struct PlatformContext {
    /// Supplied native operations and path helpers.
    pub(crate) api: ExtensionAPI,
    /// Directory against which the configuration's relative paths were authored.
    pub(crate) cwd: String,
    /// Unmodified authored filesystem policy.
    pub(crate) filesystem: FilesystemConfig,
    /// Explicit networking permissions.
    pub(crate) network: super::network::NetworkConfig,
    /// Explicitly retains the host process namespace.
    pub(crate) weaker: bool,
    /// Identity of the next owned macOS profile file.
    pub(crate) profile_id: Cell<u64>,
    /// Private control directory, never writable by the isolated command.
    pub(crate) control_directory: String,
    /// The native bash executable resolved during initialization.
    pub(crate) shell: String,
    /// Host proxy ports, retained for the command's diagnostic environment.
    pub(crate) ports: [u16; 2],
}

/// One initialized sandbox, with explicit cleanup independent of in-flight borrowed handles.
pub(crate) struct Runtime {
    /// Native operating system selected before initialization.
    platform: String,
    /// Authored settings used for the status count.
    pub(crate) config: SandboxConfig,
    /// Linux wrapping operands and native I/O capability.
    context: PlatformContext,
    /// The authored native temporary directory, retained after cleanup.
    temporary: String,
    /// Whether new wrapping is still admitted.
    active: Cell<bool>,
    /// Guest-owned loopback sockets and their background tasks.
    proxies: RefCell<Option<Proxies>>,
    /// Native Unix-socket bridges, each in its own process group.
    bridges: RefCell<Vec<Rc<dyn ChildProcess>>>,
}
impl Runtime {
    /// Prepares the platform commands before publishing an enabled sandbox.
    pub(crate) async fn start(
        api: ExtensionAPI,
        background: Background,
        config: SandboxConfig,
        cwd: String,
        platform: String,
    ) -> Result<Self, String> {
        for program in if platform == "Linux" {
            vec!["bwrap", "socat", "rg"]
        } else {
            vec!["rg"]
        } {
            let argument = if program == "socat" {
                "-V"
            } else {
                "--version"
            };
            command_output(&api, program, &[argument.into()]).await?;
        }
        let shell =
            command_output(&api, "bash", &["-c".into(), "printf '%s' \"$BASH\"".into()]).await?;
        if platform == "Darwin" {
            drop(
                command_output(
                    &api,
                    &shell,
                    &["-c".into(), "command -v sandbox-exec".into()],
                )
                .await?,
            );
        }
        let authored = std::env::var("MAESTRO_TMPDIR")
            .ok()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "/tmp/maestro".into());
        let temporary = resolve(&api, &[&cwd, &expand_path(&api, &authored)?])?;
        drop(ensure_temporary(&api, &shell, &temporary).await?);
        let control_directory = make_temp_dir(&api, "maestro-sandbox")?;
        let mut runtime = Self {
            context: PlatformContext {
                api,
                cwd,
                filesystem: config.filesystem.clone(),
                network: config.network.clone(),
                weaker: config.enable_weaker_nested_sandbox,
                profile_id: Cell::new(0),
                control_directory,
                shell,
                ports: [0, 0],
            },
            platform,
            config,
            temporary,
            active: Cell::new(true),
            proxies: RefCell::new(None),
            bridges: RefCell::default(),
        };
        runtime.initialize(background).await?;
        Ok(runtime)
    }

    /// Owns resources as soon as they exist, so a partial initialization is cleaned up too.
    async fn initialize(&mut self, background: Background) -> Result<(), String> {
        if self.context.control_directory.contains(',') {
            return Err("Sandbox control directory cannot contain a comma".into());
        }
        std::fs::write(
            guest_path(
                &self.context.api,
                &format!("{}/empty", self.context.control_directory),
            )?,
            [],
        )
        .map_err(|error| error.to_string())?;
        if self.platform == "Linux" && !self.config.network.allow_all_unix_sockets {
            let architecture = command_output(&self.context.api, "uname", &["-m".into()]).await?;
            let path = format!("{}/unix-block.bpf", self.context.control_directory);
            std::fs::write(
                guest_path(&self.context.api, &path)?,
                seccomp::filter(architecture.trim())?,
            )
            .map_err(|error| error.to_string())?;
        }
        let proxies = Proxies::start(background, self.config.network.clone()).await?;
        self.context.ports = proxies.ports;
        self.proxies.replace(Some(proxies));
        if self.platform != "Linux" {
            return Ok(());
        }
        for (name, port) in [
            ("http", self.context.ports[0]),
            ("socks", self.context.ports[1]),
        ] {
            let args = vec![
                format!(
                    "UNIX-LISTEN:{}/{name}.sock,fork,reuseaddr,mode=0600",
                    self.context.control_directory
                ),
                format!("TCP4:127.0.0.1:{port},keepalive,keepidle=10,keepintvl=5,keepcnt=3"),
            ];
            let options = SpawnOptions {
                detached: true,
                stdio: [Stdio::Ignore; 3],
                ..SpawnOptions::default()
            };
            let child = self
                .context
                .api
                .native_io()
                .spawn("socat".into(), args, options)
                .await?;
            self.bridges.borrow_mut().push(child);
        }
        self.wait_for_bridges().await
    }

    /// Retains the bounded readiness interval while waiting on the existing native clock.
    async fn wait_for_bridges(&self) -> Result<(), String> {
        for delay in [0, 100, 200, 300] {
            if self.bridges_ready()? {
                return Ok(());
            }
            self.context
                .api
                .native_io()
                .sleep(Duration::from_millis(delay), None)
                .await?;
        }
        if self.bridges_ready()? {
            Ok(())
        } else {
            Err("Sandbox bridge sockets were not created before initialization completed".into())
        }
    }

    /// Observes only this runtime's private socket paths.
    fn bridges_ready(&self) -> Result<bool, String> {
        for name in ["http", "socks"] {
            let path = format!("{}/{name}.sock", self.context.control_directory);
            if !Path::new(&guest_path(&self.context.api, &path)?).exists() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Validates the persistent temporary directory and expands current filesystem exclusions.
    pub(crate) async fn wrap(&self, command: &str) -> Result<String, String> {
        if !self.active.get() {
            return Err("Sandbox has shut down".into());
        }
        let temporary =
            ensure_temporary(&self.context.api, &self.context.shell, &self.temporary).await?;
        let wrapped = if self.platform == "Linux" {
            self.context
                .wrap_linux(command, &temporary, &self.temporary)
                .await?
        } else {
            super::macos::wrap(&self.context, command, &temporary, &self.temporary)?
        };
        if !self.active.get() {
            return Err("Sandbox shut down while preparing the command".into());
        }
        Ok(wrapped)
    }

    /// Stops admission and releases every owned process and socket; user temp artifacts remain.
    pub(crate) async fn close(&self) {
        for child in self.stop() {
            if let Err(error) = child.wait().await {
                eprintln!("Cannot reap sandbox bridge: {error}");
            }
        }
    }

    /// Kills bridge groups and removes only the private control directory.
    fn stop(&self) -> Vec<Rc<dyn ChildProcess>> {
        if !self.active.replace(false) {
            return Vec::new();
        }
        let proxies = self.proxies.borrow_mut().take();
        drop(proxies);
        let children = std::mem::take(&mut *self.bridges.borrow_mut());
        let io = self.context.api.native_io();
        for child in &children {
            let signal = Some("SIGKILL".into());
            let result = match child.pid().and_then(|pid| i32::try_from(pid).ok()) {
                Some(pid) => io.kill(-pid, signal),
                None => child.kill(signal).map(|_| ()),
            };
            if let Err(error) = result {
                eprintln!("Cannot stop sandbox bridge: {error}");
            }
        }
        if let Err(error) = self.remove_control() {
            eprintln!("Cannot remove sandbox control directory: {error}");
        }
        children
    }
    /// Unlinks known control entries without asking WASI to classify Unix socket entries.
    fn remove_control(&self) -> Result<(), String> {
        let directory = guest_path(&self.context.api, &self.context.control_directory)?;
        for name in ["http.sock", "socks.sock", "unix-block.bpf", "empty"] {
            match std::fs::remove_file(Path::new(&directory).join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        for entry in std::fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            std::fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
        }
        std::fs::remove_dir(directory).map_err(|error| error.to_string())
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        drop(self.stop());
    }
}

/// Creates an owned mode-0700 directory and returns its observed physical native path.
async fn ensure_temporary(api: &ExtensionAPI, shell: &str, path: &str) -> Result<String, String> {
    command_output(
        api,
        shell,
        &["-c".into(), TEMPORARY.into(), "--".into(), path.into()],
    )
    .await
}

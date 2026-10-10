//! Resolve authored values through environment and command operations.
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, OnceLock},
};

/// Shared initialization of one command's success or absence.
type CommandResult = Arc<OnceLock<Option<String>>>;

/// Process-wide command results, including failed resolutions.
static COMMAND_CACHE: LazyLock<Mutex<HashMap<String, CommandResult>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Clear cached command results.
pub fn clear_config_value_cache() {
    COMMAND_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}
/// Effects used when resolving a configured value.
pub trait ConfigValueOperations {
    /// Read one exact environment name.
    fn environment(&self, name: &str) -> Option<String>;
    /// Return untrimmed stdout from a successful command.
    fn execute(&self, command: &str) -> Option<Vec<u8>>;
}
/// Resolve a value, caching command results by the complete configured string.
/// Concurrent misses share one initialization, including an absent result.
pub fn resolve_config_value(
    config: &str,
    operations: &dyn ConfigValueOperations,
) -> Option<String> {
    if !config.starts_with('!') {
        return resolve_config_value_uncached(config, operations);
    }
    let result = Arc::clone(
        COMMAND_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(config.to_owned())
            .or_default(),
    );
    result
        .get_or_init(|| resolve_config_value_uncached(config, operations))
        .clone()
}
/// Resolve a configured value without consulting the command cache.
pub fn resolve_config_value_uncached(
    config: &str,
    operations: &dyn ConfigValueOperations,
) -> Option<String> {
    if let Some(command) = config.strip_prefix('!') {
        let output = operations.execute(command)?;
        let value = String::from_utf8_lossy(&output)
            .trim_matches(config_whitespace)
            .to_owned();
        return (!value.is_empty()).then_some(value);
    }
    Some(
        operations
            .environment(config)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| config.to_owned()),
    )
}
/// Whitespace admitted at command-output boundaries.
fn config_whitespace(character: char) -> bool {
    matches!(character, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
/// Resolve freshly, reporting the authored command description on failure.
///
/// # Errors
/// Returns an error when command resolution produces no value.
pub fn resolve_config_value_or_throw(
    config: &str,
    description: &str,
    operations: &dyn ConfigValueOperations,
) -> std::io::Result<String> {
    resolve_config_value_uncached(config, operations).ok_or_else(|| {
        std::io::Error::other(format!(
            "Failed to resolve {description} from shell command: {}",
            &config[1..]
        ))
    })
}
/// Resolve cached header values, omitting empty or unresolved results.
pub fn resolve_headers(
    headers: Option<&indexmap::IndexMap<String, String>>,
    operations: &dyn ConfigValueOperations,
) -> Option<indexmap::IndexMap<String, String>> {
    let resolved: indexmap::IndexMap<_, _> = headers?
        .iter()
        .filter_map(|(key, value)| {
            resolve_config_value(value, operations)
                .filter(|value| !value.is_empty())
                .map(|value| (key.clone(), value))
        })
        .collect();
    (!resolved.is_empty()).then_some(resolved)
}
/// Resolve headers freshly, retaining empty strings and stopping at the first failure.
///
/// # Errors
/// Returns the first command-resolution error, with the header name in its description.
pub fn resolve_headers_or_throw(
    headers: Option<&indexmap::IndexMap<String, String>>,
    description: &str,
    operations: &dyn ConfigValueOperations,
) -> std::io::Result<Option<indexmap::IndexMap<String, String>>> {
    let Some(headers) = headers else {
        return Ok(None);
    };
    let resolved: indexmap::IndexMap<_, _> = headers
        .iter()
        .map(|(key, value)| {
            resolve_config_value_or_throw(
                value,
                &format!("{description} header \"{key}\""),
                operations,
            )
            .map(|value| (key.clone(), value))
        })
        .collect::<std::io::Result<_>>()?;
    Ok((!resolved.is_empty()).then_some(resolved))
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::ProcessConfigValueOperations;

/// Native process execution and shell selection.
#[cfg(not(target_arch = "wasm32"))]
pub(in crate::credentials) mod native {
    use super::ConfigValueOperations;
    use std::{io, process::Stdio};
    use tokio::{io::AsyncReadExt, process::Command};

    /// Lazily supplied configured executable and ordered arguments.
    type ShellSelector = dyn Fn() -> io::Result<(String, Vec<String>)> + Send + Sync;
    /// Native environment and synchronous helper execution.
    pub struct ProcessConfigValueOperations {
        /// Used only for Windows configured execution.
        shell_config: Box<ShellSelector>,
    }
    impl ProcessConfigValueOperations {
        /// Retain a lazy configured-shell selector without evaluating it.
        pub fn new(
            shell_config: impl Fn() -> io::Result<(String, Vec<String>)> + Send + Sync + 'static,
        ) -> Self {
            Self {
                shell_config: Box::new(shell_config),
            }
        }
    }
    impl ConfigValueOperations for ProcessConfigValueOperations {
        fn environment(&self, name: &str) -> Option<String> {
            std::env::var(name).ok()
        }
        fn execute(&self, command: &str) -> Option<Vec<u8>> {
            routed(cfg!(windows), command, &*self.shell_config, &run, || {
                let comspec = cfg!(windows)
                    .then(|| std::env::var("COMSPEC").ok())
                    .flatten();
                default_shell(command, Platform::current(), comspec.as_deref())
            })
        }
    }
    /// Executable with arguments; verbatim arguments are used only by Windows cmd.
    #[derive(Debug, PartialEq, Eq)]
    pub(in crate::credentials) struct Launch {
        /// Executable selected by the caller or default-shell policy.
        pub(in crate::credentials) executable: String,
        /// Ordered arguments, including the command.
        pub(in crate::credentials) args: Vec<String>,
        /// Whether Windows arguments bypass normal quoting.
        pub(in crate::credentials) verbatim: bool,
    }
    /// A completed attempt is never retried, even when it produces no value.
    pub(in crate::credentials) enum Attempt {
        /// Successful stdout or a completed failure.
        Completed(Option<Vec<u8>>),
        /// Selection, infrastructure or missing-executable failure permits fallback.
        Unavailable,
    }
    /// Route configured Windows attempts before default execution.
    pub(in crate::credentials) fn routed(
        windows: bool,
        command: &str,
        selector: &(impl Fn() -> io::Result<(String, Vec<String>)> + ?Sized),
        run: &impl Fn(Launch) -> Attempt,
        default: impl FnOnce() -> Launch,
    ) -> Option<Vec<u8>> {
        if windows && let Ok((executable, mut args)) = selector() {
            args.push(command.to_owned());
            if let Attempt::Completed(value) = run(Launch {
                executable,
                args,
                verbatim: false,
            }) {
                return value;
            }
        }
        match run(default()) {
            Attempt::Completed(value) => value,
            Attempt::Unavailable => None,
        }
    }
    /// Runtime platform's default-shell family.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(in crate::credentials) enum Platform {
        /// Windows configured/default routing.
        Windows,
        /// Android's system shell.
        Android,
        /// Other native Unix shells.
        Unix,
    }
    impl Platform {
        /// Select the compile target's shell family.
        fn current() -> Self {
            if cfg!(windows) {
                Self::Windows
            } else if cfg!(target_os = "android") {
                Self::Android
            } else {
                Self::Unix
            }
        }
    }
    /// Construct the runtime's default shell command without parsing command text.
    pub(in crate::credentials) fn default_shell(
        command: &str,
        platform: Platform,
        comspec: Option<&str>,
    ) -> Launch {
        if platform != Platform::Windows {
            return Launch {
                executable: if platform == Platform::Android {
                    "/system/bin/sh"
                } else {
                    "/bin/sh"
                }
                .into(),
                args: vec!["-c".into(), command.into()],
                verbatim: false,
            };
        }
        let executable = comspec
            .filter(|value| !value.is_empty())
            .unwrap_or("cmd.exe");
        let name = executable.rsplit('\\').next().unwrap_or(executable);
        let cmd = name.eq_ignore_ascii_case("cmd") || name.eq_ignore_ascii_case("cmd.exe");
        let args = if cmd {
            vec![
                "/d".into(),
                "/s".into(),
                "/c".into(),
                format!("\"{command}\""),
            ]
        } else {
            vec!["-c".into(), command.into()]
        };
        Launch {
            executable: executable.into(),
            args,
            verbatim: cmd,
        }
    }
    /// Run a current-thread runtime away from any caller runtime.
    fn run(launch: Launch) -> Attempt {
        std::thread::scope(|scope| {
            let worker =
                std::thread::Builder::new().spawn_scoped(scope, move || run_runtime(launch));
            worker
                .ok()
                .and_then(|worker| worker.join().ok())
                .unwrap_or(Attempt::Unavailable)
        })
    }
    /// Build the local runtime on the helper thread.
    fn run_runtime(launch: Launch) -> Attempt {
        match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime.block_on(collect(launch)),
            Err(_) => Attempt::Unavailable,
        }
    }
    /// Missing executables permit fallback; other launch errors complete the attempt.
    pub(in crate::credentials) fn spawn_failure(error: &io::Error) -> Attempt {
        if error.kind() == io::ErrorKind::NotFound {
            Attempt::Unavailable
        } else {
            Attempt::Completed(None)
        }
    }
    /// Capture stdout and exit concurrently, retaining the child for timeout cleanup.
    async fn collect(launch: Launch) -> Attempt {
        let mut command = Command::new(&launch.executable);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
            if launch.verbatim {
                for arg in &launch.args {
                    command.as_std_mut().raw_arg(arg);
                }
            } else {
                command.args(&launch.args);
            }
        }
        #[cfg(not(windows))]
        command.args(&launch.args);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return spawn_failure(&error),
        };
        let Some(mut stdout) = child.stdout.take() else {
            return Attempt::Completed(None);
        };
        let mut bytes = Vec::new();
        let completion = tokio::time::timeout(std::time::Duration::from_millis(10_000), async {
            tokio::try_join!(stdout.read_to_end(&mut bytes), child.wait())
        })
        .await;
        match completion {
            Ok(Ok((_, status))) if status.success() => Attempt::Completed(Some(bytes)),
            Ok(_) => Attempt::Completed(None),
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                Attempt::Completed(None)
            }
        }
    }
}

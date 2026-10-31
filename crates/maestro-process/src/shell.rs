//! Native shell command construction and literal operand quoting.

/// Executable with arguments; verbatim arguments are used only by Windows cmd.
#[derive(Debug, PartialEq, Eq)]
pub struct Launch {
    /// Executable selected by the caller or default-shell policy.
    pub executable: String,
    /// Ordered arguments, including the command.
    pub args: Vec<String>,
    /// Whether Windows arguments bypass normal quoting.
    pub verbatim: bool,
}

impl Launch {
    /// Collects the child directory and environment before executable resolution.
    #[must_use]
    pub fn with_context<I>(
        self,
        environment: I,
        cwd: Option<&std::path::Path>,
    ) -> LaunchContext<'_, I> {
        LaunchContext {
            executable: self.executable,
            args: self
                .args
                .into_iter()
                .map(std::ffi::OsString::from)
                .collect(),
            #[cfg(windows)]
            verbatim: self.verbatim,
            environment,
            cwd,
            clear_environment: false,
            background: false,
        }
    }
}

/// A consuming native launch whose lookup inputs cannot change after resolution.
///
/// Stream choices are supplied to the operation that spawns the child.
/// Command construction stays private:
/// ```compile_fail,E0624
/// use maestro_process::Launch;
/// let launch = Launch { executable: "example".into(), args: vec![], verbatim: false };
/// launch.with_context([("PATH", "")], None).execute(Ok);
/// ```
#[derive(Debug)]
pub struct LaunchContext<'a, I> {
    /// Unresolved executable.
    executable: String,
    /// Literal native operands, including any appended path.
    args: Vec<std::ffi::OsString>,
    /// Whether the final Windows shell operand bypasses quoting.
    #[cfg(windows)]
    verbatim: bool,
    /// Original environment pairs, consumed without an intermediate string copy.
    environment: I,
    /// Child working directory; absent inherits the parent directory.
    cwd: Option<&'a std::path::Path>,
    /// Whether supplied pairs replace rather than overlay the environment.
    clear_environment: bool,
    /// Whether to suppress a Windows console window.
    background: bool,
}

impl<I, K, V> LaunchContext<'_, I>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    /// Appends one literal native argument without converting path bytes to text.
    #[must_use]
    pub fn arg(mut self, argument: impl AsRef<std::ffi::OsStr>) -> Self {
        self.args.push(argument.as_ref().into());
        self
    }

    /// Replaces inherited variables with the supplied environment.
    #[must_use]
    pub fn clear_environment(mut self) -> Self {
        self.clear_environment = true;
        self
    }

    /// Suppresses the Windows console window; other platforms need no setting.
    #[must_use]
    pub fn background(mut self) -> Self {
        self.background = true;
        self
    }

    /// Runs a synchronous child with ignored stdin and captured stdout and stderr.
    /// # Errors
    /// Returns executable resolution, spawn or capture errors unchanged.
    pub fn output(self) -> std::io::Result<std::process::Output> {
        self.execute(|mut command| command.output())
    }

    /// Runs through the shared synchronous bridge with caller-selected capture policy.
    /// # Errors
    /// Returns native failures or bridge failures in the existing error variants.
    pub fn spawn_sync(
        self,
        options: crate::SpawnSyncOptions<'_>,
    ) -> Result<std::process::Output, crate::SpawnSyncError> {
        self.execute(|command| Ok(crate::spawn_sync(&mut command.into(), options)))
            .map_err(crate::SpawnSyncError::Spawn)?
    }

    /// Spawns a Tokio child with caller-selected standard streams.
    /// # Errors
    /// Returns executable resolution or native spawn failures unchanged.
    pub fn spawn(
        self,
        [stdin, stdout, stderr]: [std::process::Stdio; 3],
    ) -> std::io::Result<tokio::process::Child> {
        self.execute(|command| {
            tokio::process::Command::from(command)
                .stdin(stdin)
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
        })
    }

    /// Spawns through the shared captured-child owner with ignored stdin.
    /// # Errors
    /// Returns executable resolution or native spawn failures unchanged.
    pub fn spawn_captured(self) -> std::io::Result<crate::child_process::CapturedChild> {
        self.execute(|command| crate::child_process::CapturedChild::spawn(&mut command.into()))
    }

    /// Resolves once from the complete context and invokes an operation-private spawn.
    fn execute<T>(
        self,
        operation: impl FnOnce(std::process::Command) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        let environment: Vec<_> = self.environment.into_iter().collect();
        #[cfg(windows)]
        let executable = native_windows_executable(
            &self.executable,
            &environment,
            self.cwd,
            !self.clear_environment,
        )?;
        #[cfg(not(windows))]
        let executable = &self.executable;
        let mut command = crate::command(executable).into_std();
        if self.clear_environment {
            command.env_clear();
        }
        command.envs(environment);
        if let Some(cwd) = self.cwd {
            command.current_dir(cwd);
        }
        if self.background {
            configure_background_command(&mut command);
        }
        #[cfg(windows)]
        if self.verbatim {
            use std::os::windows::process::CommandExt;
            if let Some((text, args)) = self.args.split_last() {
                command.args(args).raw_arg(text);
            }
            return operation(command);
        }
        command.args(self.args);
        operation(command)
    }
}

/// Builds a Windows shell launch without parsing the caller's command text.
#[must_use]
pub fn windows_shell_launch(command: &str, comspec: Option<&str>) -> Launch {
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

/// Resolves a Windows command against supplied directory and extension observations.
#[cfg(any(windows, test))]
fn resolve_windows_executable(
    command: &str,
    directories: impl IntoIterator<Item = impl AsRef<std::path::Path>>,
    extensions: &str,
    cwd: &std::path::Path,
    exists: impl Fn(&std::path::Path) -> bool,
) -> Option<std::path::PathBuf> {
    let name = command.rsplit(['\\', '/']).next()?;
    let candidates = if command.contains(['\\', '/', ':']) {
        vec![anchor_windows_path(std::path::Path::new(command), cwd)]
    } else {
        directories
            .into_iter()
            .map(|dir| anchor_windows_path(dir.as_ref(), cwd).join(command))
            .collect()
    };
    candidates.into_iter().find_map(|candidate| {
        if name.contains('.') {
            return exists(&candidate).then_some(candidate);
        }
        extensions
            .split(';')
            .filter(|ext| !ext.is_empty())
            .find_map(|ext| {
                let mut path = candidate.as_os_str().to_os_string();
                path.push(ext);
                let path = std::path::PathBuf::from(path);
                exists(&path).then_some(path)
            })
    })
}

/// Anchors native Windows paths, including a relative path on the child's drive.
#[cfg(any(windows, test))]
fn anchor_windows_path(path: &std::path::Path, cwd: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, Prefix};
    let same_drive = match (path.components().next(), cwd.components().next()) {
        (Some(Component::Prefix(path)), Some(Component::Prefix(base))) => {
            matches!((path.kind(), base.kind()), (Prefix::Disk(a) | Prefix::VerbatimDisk(a), Prefix::Disk(b) | Prefix::VerbatimDisk(b)) if a.eq_ignore_ascii_case(&b))
        }
        _ => false,
    };
    if same_drive && !path.has_root() {
        cwd.join(path.components().skip(1).collect::<std::path::PathBuf>())
    } else {
        cwd.join(path)
    }
}

/// Resolves against the effective child directory and environment before spawning.
#[cfg(windows)]
fn native_windows_executable<K, V>(
    command: &str,
    environment: &[(K, V)],
    cwd: Option<&std::path::Path>,
    inherit: bool,
) -> std::io::Result<std::path::PathBuf>
where
    K: AsRef<std::ffi::OsStr>,
    V: AsRef<std::ffi::OsStr>,
{
    let value = |key: &str| {
        environment
            .iter()
            .rev()
            .find(|(name, _)| name.as_ref().eq_ignore_ascii_case(key))
            .map(|(_, value)| std::borrow::Cow::Borrowed(value.as_ref()))
            .or_else(|| {
                inherit
                    .then(|| std::env::var_os(key))
                    .flatten()
                    .map(std::borrow::Cow::Owned)
            })
    };
    let cwd = cwd.map_or_else(std::env::current_dir, std::path::absolute)?;
    let path = value("PATH").unwrap_or_default();
    let directories = value("NoDefaultCurrentDirectoryInExePath")
        .is_none()
        .then_some(cwd.as_path())
        .into_iter()
        .map(std::borrow::Cow::Borrowed)
        .chain(
            std::env::split_paths(&path)
                .filter(|directory| !directory.as_os_str().is_empty())
                .map(std::borrow::Cow::Owned),
        );
    let extensions = value("PATHEXT");
    let extensions = extensions
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or(".EXE;.COM;.BAT;.CMD");
    let executable = resolve_windows_executable(
        command,
        directories,
        extensions,
        &cwd,
        std::path::Path::is_file,
    )
    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
    std::path::absolute(executable)
}

pub use maestro_text::shell_quote;

/// Serializes one literal PowerShell operand.
#[must_use]
pub fn quote_powershell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-/.:+=".contains(&byte))
    {
        value.to_owned()
    } else {
        quote_powershell_literal(value)
    }
}

/// Serializes a single-quoted PowerShell string literal, including safe-set words.
#[must_use]
pub fn quote_powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Normalizes a process basename in shell recognition order.
fn normalized_process_name(name: &str) -> String {
    name.rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .trim_start_matches('-')
        .trim_end_matches(".exe")
        .to_ascii_lowercase()
}

/// Quotes an argument vector for the selected shell; empty vectors have no command.
#[must_use]
pub fn interactive_shell_command(argv: &[String], shell_name: &str) -> Option<String> {
    let quote = if matches!(
        normalized_process_name(shell_name).as_str(),
        "pwsh" | "powershell"
    ) {
        quote_powershell_arg
    } else {
        shell_quote
    };
    (!argv.is_empty()).then(|| {
        argv.iter()
            .map(|arg| quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Recognizes shell basenames without treating replacement programs as shells.
#[must_use]
pub fn is_pane_shell_process_name(name: &str) -> bool {
    matches!(
        normalized_process_name(name).as_str(),
        "sh" | "bash"
            | "dash"
            | "zsh"
            | "fish"
            | "ksh"
            | "mksh"
            | "csh"
            | "tcsh"
            | "elvish"
            | "xonsh"
            | "nu"
            | "pwsh"
            | "powershell"
            | "cmd"
    )
}

/// Builds a command without opening a Windows console.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut command = crate::command(program).into_std();
    configure_background_command(&mut command);
    command
}

/// Applies the native background-launch flag without changing operands.
pub(crate) fn configure_background_command(command: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Builds a custom command using a login POSIX shell.
#[cfg(unix)]
#[must_use]
pub fn detached_custom_command_process(text: &str) -> std::process::Command {
    let mut process = command("/bin/sh");
    process.args(["-lc", text]);
    process
}

/// Builds editor shell text that removes the scrollback file and retains editor status.
///
/// # Errors
/// This Linux builder does not currently return an error.
#[cfg(target_os = "linux")]
pub fn scrollback_editor_argv(path: &std::path::Path) -> std::io::Result<Vec<String>> {
    let path = shell_quote(&path.display().to_string());
    Ok(vec![
        "/bin/sh".into(),
        "-c".into(),
        format!(
            r#"scrollback_file={path}; eval "${{EDITOR:-vi}} \"\$scrollback_file\""; status=$?; rm -f "$scrollback_file"; exit $status"#
        ),
    ])
}

/// Builds a custom pane command without interpreting its command text.
#[cfg(target_os = "linux")]
#[must_use]
pub fn pane_custom_command_pty_builder(text: &str) -> crate::pty::PtyCommand {
    crate::pty::PtyCommand {
        program: "/bin/sh".into(),
        argv0: None,
        args: vec!["-c".into(), text.into()],
        cwd: None,
        environment: crate::pty::PtyEnvironment::Inherit(Vec::new()),
    }
}

/// Returns the sole pane shell, rejecting another foreground group or replacement program.
#[cfg(target_os = "linux")]
#[must_use]
pub fn available_pane_shell(
    child_pid: u32,
    mode: crate::pty::foreground::ProcessDetectionMode,
) -> Option<String> {
    available_pane_shell_from_job(
        child_pid,
        crate::pty::foreground::foreground_job(child_pid, mode)?,
    )
}

/// Checks shell eligibility without losing the original process name.
#[cfg(target_os = "linux")]
pub(crate) fn available_pane_shell_from_job(
    child: u32,
    job: crate::pty::foreground::ForegroundJob,
) -> Option<String> {
    if job.process_group_id != child || job.processes.iter().any(|process| process.pid != child) {
        return None;
    }
    job.processes
        .into_iter()
        .find(|process| process.pid == child)
        .map(|process| process.name)
        .filter(|name| is_pane_shell_process_name(name))
}

#[cfg(test)]
mod tests;

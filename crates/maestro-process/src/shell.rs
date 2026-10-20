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
    /// Builds a native command, preserving literal argv unless shell text is verbatim.
    ///
    /// # Errors
    /// Returns `NotFound` when a Windows command cannot be resolved.
    pub fn command(self) -> std::io::Result<std::process::Command> {
        #[cfg(windows)]
        let executable = native_windows_executable(&self.executable)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
        #[cfg(not(windows))]
        let executable = &self.executable;
        let mut command = std::process::Command::new(executable);
        #[cfg(windows)]
        if self.verbatim {
            use std::os::windows::process::CommandExt;
            if let Some((text, args)) = self.args.split_last() {
                command.args(args).raw_arg(text);
            }
            return Ok(command);
        }
        command.args(self.args);
        Ok(command)
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
    directories: &[std::path::PathBuf],
    extensions: &str,
    exists: impl Fn(&std::path::Path) -> bool,
) -> Option<std::path::PathBuf> {
    let name = command.rsplit(['\\', '/']).next()?;
    let candidates = if command.contains(['\\', '/', ':']) {
        vec![std::path::PathBuf::from(command)]
    } else {
        directories.iter().map(|dir| dir.join(command)).collect()
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

/// Observes the current directory, search path and executable extensions once per launch.
#[cfg(windows)]
fn native_windows_executable(command: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let directories: Vec<_> = std::env::current_dir()
        .into_iter()
        .chain(std::env::split_paths(&path))
        .collect();
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.COM;.BAT;.CMD".into());
    resolve_windows_executable(command, &directories, &extensions, std::path::Path::is_file)
}

/// Serializes one literal POSIX shell operand.
#[must_use]
pub fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"@%_+=:,./-".contains(&byte))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

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
    let mut command = std::process::Command::new(program);
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

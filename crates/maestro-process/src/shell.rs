//! Windows command-interpreter launch normalization.

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
    /// Returns `NotFound` when a Windows extensionless command cannot be resolved.
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
    if name.contains('.') {
        return Some(command.into());
    }
    let candidates = if command.contains(['\\', '/', ':']) {
        vec![std::path::PathBuf::from(command)]
    } else {
        directories.iter().map(|dir| dir.join(command)).collect()
    };
    candidates.into_iter().find_map(|candidate| {
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

#[cfg(test)]
mod tests;

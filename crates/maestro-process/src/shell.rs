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

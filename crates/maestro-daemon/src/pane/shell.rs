//! Pane shell selection and native launch policy.

use crate::settings::model::ShellModeConfig;
use maestro_process::pty::{PtyCommand, PtyEnvironment};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::Path,
};

/// Platform selecting shell defaults and login policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellLaunchTarget {
    /// Windows command-line shells.
    Windows,
    /// macOS defaults to a login shell.
    Macos,
    /// Other Unix targets default to a direct shell.
    OtherUnix,
}

/// Select the configured shell, inherited shell, or platform default.
#[must_use]
pub fn pane_shell_from(
    configured: &str,
    inherited: Option<&str>,
    target: ShellLaunchTarget,
) -> String {
    let configured = configured.trim();
    if !configured.is_empty() {
        return configured.to_owned();
    }
    if target == ShellLaunchTarget::Windows {
        return "powershell.exe".into();
    }
    inherited
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
        .unwrap_or("/bin/sh")
        .into()
}

/// Supplied shell name and interactive login policy.
#[derive(Clone, Copy)]
pub struct PaneShellConfig<'a> {
    /// Explicit shell, or blank to use the environment/default.
    pub default_shell: &'a str,
    /// Whether to start a login shell.
    pub mode: ShellModeConfig,
}

impl<'a> PaneShellConfig<'a> {
    /// Retain the caller's shell configuration.
    #[must_use]
    pub fn new(default_shell: &'a str, mode: ShellModeConfig) -> Self {
        Self {
            default_shell,
            mode,
        }
    }
}

/// Whether the platform and configuration require a login shell.
#[must_use]
pub fn shell_mode_uses_login_shell(mode: ShellModeConfig, target: ShellLaunchTarget) -> bool {
    match mode {
        ShellModeConfig::Auto => target == ShellLaunchTarget::Macos,
        ShellModeConfig::Login => true,
        ShellModeConfig::NonLogin => false,
    }
}

/// Check executable permission without accepting directories.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Resolve login shells before launch, without silently selecting another shell.
///
/// # Errors
/// Returns `NotFound` for a missing or non-executable shell.
pub fn resolve_shell_for_login_mode(shell: &str, path: Option<&OsStr>) -> io::Result<String> {
    if shell.contains(std::path::MAIN_SEPARATOR) {
        return is_executable_file(Path::new(shell))
            .then(|| shell.to_owned())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("login shell {shell:?} is not executable"),
                )
            });
    }
    path.and_then(|path| {
        std::env::split_paths(path)
            .map(|dir| dir.join(shell))
            .find(|candidate| is_executable_file(candidate))
            .and_then(|candidate| candidate.into_os_string().into_string().ok())
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("login shell {shell:?} was not found on PATH"),
        )
    })
}

/// Wrap the profile prompt before reporting filesystem cwd, preserving its status input.
/// Single quotes and character codes survive Windows PowerShell argument quoting.
pub const WINDOWS_POWERSHELL_SHELL_INTEGRATION_COMMAND: &str = r"if ($null -eq $global:__MaestroOriginalPrompt) { $global:__MaestroOriginalPrompt = $function:prompt; function global:prompt { $out = @(& $global:__MaestroOriginalPrompt) -join ' '; $loc = $ExecutionContext.SessionState.Path.CurrentLocation; if ($loc.Provider.Name -eq 'FileSystem') { try { [Environment]::CurrentDirectory = $loc.ProviderPath } catch {}; $esc = [string][char]27; $out += $esc + ']9;9;' + $loc.ProviderPath + $esc + '\' }; $out } }";

/// Whether the direct Windows shell needs prompt-based cwd reporting.
#[must_use]
pub fn uses_windows_powershell_pane_shell_for_target(
    config: PaneShellConfig<'_>,
    target: ShellLaunchTarget,
    inherited_shell: Option<&str>,
) -> bool {
    let shell = pane_shell_from(config.default_shell, inherited_shell, target);
    let name = shell
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&shell)
        .to_ascii_lowercase();
    target == ShellLaunchTarget::Windows
        && !shell_mode_uses_login_shell(config.mode, target)
        && matches!(
            name.as_str(),
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
        )
}

/// Build native launch operands from supplied environment observations.
///
/// # Errors
/// Returns `NotFound` if a configured login shell cannot be resolved.
pub fn pane_shell_command_builder_for_target(
    config: PaneShellConfig<'_>,
    target: ShellLaunchTarget,
    environment: impl Fn(&str) -> Option<OsString>,
) -> io::Result<PtyCommand> {
    let inherited = environment("SHELL");
    let shell = pane_shell_from(
        config.default_shell,
        inherited.as_deref().and_then(OsStr::to_str),
        target,
    );
    let mut command = PtyCommand {
        program: shell.clone().into(),
        argv0: None,
        args: Vec::new(),
        cwd: None,
        environment: PtyEnvironment::Inherit(Vec::new()),
    };
    if shell_mode_uses_login_shell(config.mode, target) {
        let shell = resolve_shell_for_login_mode(&shell, environment("PATH").as_deref())?;
        command.environment =
            PtyEnvironment::Inherit(vec![("SHELL".into(), Some(shell.clone().into()))]);
        if target == ShellLaunchTarget::Windows {
            command.program = environment("ComSpec").unwrap_or_else(|| "cmd.exe".into());
        } else {
            command.argv0 = Some(format!("-{}", shell.rsplit('/').next().unwrap_or(&shell)).into());
            command.program = shell.into();
        }
    } else if uses_windows_powershell_pane_shell_for_target(
        config,
        target,
        inherited.as_deref().and_then(OsStr::to_str),
    ) {
        command.args = [
            "-NoExit",
            "-Command",
            WINDOWS_POWERSHELL_SHELL_INTEGRATION_COMMAND,
        ]
        .map(OsString::from)
        .into();
    }
    Ok(command)
}

#[cfg(test)]
mod tests;

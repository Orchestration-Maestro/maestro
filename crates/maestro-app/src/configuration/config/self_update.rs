//! Owned update commands selected from installation evidence.

use super::installation::{InstallMethod, Installation, detect_install_method};
use maestro_path::{posix, win32};
use maestro_process::SpawnSyncError;
use maestro_text::is_whitespace_scalar;
use std::{
    future::Future,
    io,
    path::Path,
    pin::Pin,
    process::{ExitStatus, Output, Stdio},
};

mod execute;
mod native;
pub use execute::{
    SelfUpdatePlan, get_self_update_command_from_settings, get_self_update_plan,
    get_self_update_unavailable_instruction_from_settings, run_self_update,
};
pub use native::NativeSelfUpdateOperations;

/// One executable with literal, ordered arguments and a human-readable display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfUpdateCommandStep {
    /// Executable name or path.
    pub command: String,
    /// Literal arguments, not shell text.
    pub args: Vec<String>,
    /// Command display, quoting whitespace-containing operands.
    pub display: String,
}
/// An installation command with optional ordered replacement steps.
#[derive(Debug, PartialEq, Eq)]
pub struct SelfUpdateCommand {
    /// Installation executable name or path.
    pub command: String,
    /// Literal installation arguments.
    pub args: Vec<String>,
    /// Display of the complete update.
    pub display: String,
    /// Removal followed by installation when replacing a package name.
    pub steps: Option<Vec<SelfUpdateCommandStep>>,
}
/// Borrowed observations used only during synchronous selection.
pub struct SelfUpdateContext<'a> {
    /// Previously observed installation identity.
    pub installation: &'a Installation,
    /// Package directory selected by the caller.
    pub package_dir: &'a str,
    /// Replaceable path flavor and read-only query capture.
    pub operations: &'a dyn SelfUpdateOperations,
}
/// Replaceable path flavor and read-only query capture.
///
/// Home and write-access observations are the native ones.
pub trait SelfUpdateOperations {
    /// Whether authored paths use Windows semantics.
    fn is_windows(&self) -> bool;
    /// Capture a read-only package-manager query.
    ///
    /// # Errors
    /// Returns the process owner's original error.
    fn run_command_sync(&self, command: &str, args: &[String]) -> Result<Output, SpawnSyncError>;
    /// Run one selected step with inherited standard streams until it exits.
    ///
    /// The default runs the native launcher without a deadline; an adapter that
    /// already owns child processes replaces it.
    ///
    /// # Errors
    /// Returns executable resolution or native spawn failures unchanged.
    fn run_command<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
    ) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + 'a>> {
        Box::pin(async move {
            let mut child = execute::native_launch(command, args)
                .with_context(std::iter::empty::<(&str, &str)>(), None)
                .spawn([Stdio::inherit(), Stdio::inherit(), Stdio::inherit()])?;
            child.wait().await
        })
    }
}

/// Select Windows or POSIX directory splitting without native path conversion.
fn dirname(path: &str, windows: bool) -> String {
    if windows {
        win32::dirname(path)
    } else {
        posix::dirname(path)
    }
}
/// Select Windows or POSIX basename semantics.
fn basename(path: &str, windows: bool) -> String {
    if windows {
        win32::basename(path, None)
    } else {
        posix::basename(path, None)
    }
}
/// Join authored path operands in the supplied host flavor.
fn join(parts: &[&str], windows: bool) -> String {
    if windows {
        win32::join(parts)
    } else {
        posix::join(parts)
    }
}
/// Infer only a matching package under a Unix-style global npm layout.
fn inferred_install(context: &SelfUpdateContext<'_>, name: &str) -> Option<(String, String)> {
    let dir = context.package_dir;
    let windows = context.operations.is_windows() || dir.contains('\\');
    let parent = dirname(dir, windows);
    let mut parts = name.split('/');
    let scope = parts.next()?;
    let root = match parts.next() {
        Some(name)
            if scope.starts_with('@')
                && basename(&parent, windows) == scope
                && basename(dir, windows) == name =>
        {
            dirname(&parent, windows)
        }
        None if basename(dir, windows) == name => parent,
        _ => return None,
    };
    let parent = dirname(&root, windows);
    (basename(&root, windows) == "node_modules" && basename(&parent, windows) == "lib")
        .then(|| (root, dirname(&parent, windows)))
}
/// Join authored operands with whitespace-containing ones quoted.
fn command_line(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .map(|arg| {
            if arg.chars().any(is_whitespace_scalar) {
                format!("\"{arg}\"")
            } else {
                arg.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
/// Construct the display without modifying executable or argument bytes.
fn step(command: String, args: Vec<String>) -> SelfUpdateCommandStep {
    let display = command_line(&command, &args);
    SelfUpdateCommandStep {
        command,
        args,
        display,
    }
}
/// Option every supported package manager accepts to skip package lifecycle scripts.
const IGNORE_SCRIPTS: &str = "--ignore-scripts";
/// Select executable and verbs for one known installation method, with lifecycle scripts off.
fn command_for_method(
    context: &SelfUpdateContext<'_>,
    name: &str,
    npm: Option<&[String]>,
    update: &str,
) -> Option<SelfUpdateCommand> {
    let method = detect_install_method(context.installation);
    let (command, mut args, verb) = match method {
        InstallMethod::Npm => {
            let (command, mut args) = npm_program(npm);
            if npm.is_none_or(<[String]>::is_empty)
                && let Some((_, prefix)) = inferred_install(context, name)
            {
                args.extend(["--prefix".into(), prefix]);
            }
            (command.to_owned(), args, "install")
        }
        InstallMethod::Pnpm => ("pnpm".into(), Vec::new(), "install"),
        InstallMethod::Yarn => ("yarn".into(), vec!["global".into()], "add"),
        InstallMethod::Bun => ("bun".into(), Vec::new(), "install"),
        InstallMethod::BunBinary | InstallMethod::Unknown => return None,
    };
    let uninstall = if name == update {
        None
    } else {
        let remove = match method {
            InstallMethod::Pnpm | InstallMethod::Yarn => "remove",
            _ => "uninstall",
        };
        let mut removal = args.clone();
        removal.push(remove.into());
        if method != InstallMethod::Yarn {
            removal.push("-g".into());
        }
        removal.extend([IGNORE_SCRIPTS.into(), name.into()]);
        Some(step(command.clone(), removal))
    };
    args.push(verb.into());
    if method != InstallMethod::Yarn {
        args.push("-g".into());
    }
    args.extend([IGNORE_SCRIPTS.into(), update.into()]);
    let install = step(command, args);
    Some(match uninstall {
        None => SelfUpdateCommand {
            command: install.command,
            args: install.args,
            display: install.display,
            steps: None,
        },
        Some(remove) => SelfUpdateCommand {
            command: install.command.clone(),
            args: install.args.clone(),
            display: format!("{} && {}", remove.display, install.display),
            steps: Some(vec![remove, install]),
        },
    })
}
/// Read trimmed query stdout, suppressing only optional query failures.
fn read_output(
    context: &SelfUpdateContext<'_>,
    command: &str,
    args: &[String],
    required: bool,
) -> io::Result<Option<String>> {
    let result = context.operations.run_command_sync(command, args);
    if let Ok(output) = &result
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout)
            .trim_matches(is_whitespace_scalar)
            .to_owned();
        return Ok((!text.is_empty()).then_some(text));
    }
    if !required {
        return Ok(None);
    }
    let (reason, source) = match result {
        Err(error) => {
            let reason = std::error::Error::source(&error)
                .map_or_else(|| error.to_string(), ToString::to_string);
            (reason, Some(error))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr)
                .trim_matches(is_whitespace_scalar)
                .to_owned();
            let reason = if stderr.is_empty() {
                format!(
                    "exit code {}",
                    output
                        .status
                        .code()
                        .map_or_else(|| "unknown".into(), |c| c.to_string())
                )
            } else {
                stderr
            };
            (reason, None)
        }
    };
    Err(io::Error::other(QueryFailure {
        message: format!(
            "Failed to run {}: {reason}",
            std::iter::once(command)
                .chain(args.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        source,
    }))
}
/// Query global candidates in manager-defined order before comparing roots.
fn global_roots(
    context: &SelfUpdateContext<'_>,
    name: &str,
    npm: Option<&[String]>,
) -> io::Result<Vec<String>> {
    let windows = context.operations.is_windows();
    let method = detect_install_method(context.installation);
    let configured = npm.is_some_and(|n| !n.is_empty());
    let (command, mut args) = match method {
        InstallMethod::Npm => npm_program(npm),
        InstallMethod::Pnpm => ("pnpm", Vec::new()),
        InstallMethod::Yarn => ("yarn", Vec::new()),
        InstallMethod::Bun => ("bun", Vec::new()),
        InstallMethod::Unknown | InstallMethod::BunBinary => return Ok(Vec::new()),
    };
    let bun = method == InstallMethod::Bun
        || (method == InstallMethod::Npm && configured && command == "bun");
    args.extend(
        (if bun {
            vec!["pm", "bin", "-g"]
        } else if method == InstallMethod::Yarn {
            vec!["global", "dir"]
        } else {
            vec!["root", "-g"]
        })
        .into_iter()
        .map(str::to_owned),
    );
    let output = read_output(
        context,
        command,
        &args,
        method == InstallMethod::Npm && configured,
    )?;
    if bun {
        return bun_roots(context, output);
    }
    let mut roots = Vec::new();
    if let Some(root) = output {
        let second = match method {
            InstallMethod::Pnpm => Some(dirname(&root, windows)),
            InstallMethod::Yarn => Some(join(&[&root, "node_modules"], windows)),
            _ => None,
        };
        roots.push(root);
        roots.extend(second);
    }
    if method == InstallMethod::Npm && !configured {
        roots.extend(inferred_install(context, name).map(|(root, _)| root));
    }
    Ok(roots)
}
/// Resolve authored paths lazily using the supplied path flavor.
fn resolve(context: &SelfUpdateContext<'_>, path: &str) -> io::Result<String> {
    maestro_path::resolve_with_flavor(
        &[&[path]],
        context.operations.is_windows(),
        maestro_resources::drive_directories,
        maestro_resources::current_directory,
    )
    .map(|paths| paths.concat())
}
/// Compare existing canonical paths, not authored spelling or mere prefixes.
fn normalized(context: &SelfUpdateContext<'_>, path: &str) -> io::Result<Option<String>> {
    let resolved = resolve(context, path)?;
    if !Path::new(&resolved).exists() {
        return Ok(None);
    }
    Ok(maestro_resources::real_path(Path::new(&resolved))
        .ok()
        .map(|path| {
            let path = path.to_string_lossy().into_owned();
            if context.operations.is_windows() {
                path.to_lowercase()
            } else {
                path
            }
        }))
}
/// Require a strict canonical descendant of at least one evidenced global root.
fn managed(
    context: &SelfUpdateContext<'_>,
    name: &str,
    npm: Option<&[String]>,
) -> io::Result<bool> {
    let Some(package) = normalized(context, context.package_dir)? else {
        return Ok(false);
    };
    let separator = if context.operations.is_windows() {
        '\\'
    } else {
        '/'
    };
    for root in global_roots(context, name, npm)? {
        if let Some(mut root) = normalized(context, &root)? {
            if !root.ends_with(separator) {
                root.push(separator);
            }
            if package.starts_with(&root) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
/// Check the authored package path before its immediate parent.
fn writable(context: &SelfUpdateContext<'_>) -> bool {
    check_write_access(context.package_dir).is_ok()
        && check_write_access(&dirname(
            context.package_dir,
            context.operations.is_windows(),
        ))
        .is_ok()
}
/// Select an owned automatic update only for a writable global installation.
///
/// # Errors
/// Returns required query, home or working-directory failures.
pub fn get_self_update_command(
    context: &SelfUpdateContext<'_>,
    package_name: &str,
    npm_command: Option<&[String]>,
    update_package_name: Option<&str>,
) -> io::Result<Option<SelfUpdateCommand>> {
    let command = command_for_method(
        context,
        package_name,
        npm_command,
        update_package_name.unwrap_or(package_name),
    );
    if command.is_none() || !managed(context, package_name, npm_command)? || !writable(context) {
        return Ok(None);
    }
    Ok(command)
}
/// Explain manual update options using the same installation evidence.
///
/// # Errors
/// Returns required query, home or working-directory failures.
pub fn get_self_update_unavailable_instruction(
    context: &SelfUpdateContext<'_>,
    package_name: &str,
    npm_command: Option<&[String]>,
    update_package_name: Option<&str>,
) -> io::Result<String> {
    let method = detect_install_method(context.installation);
    if method == InstallMethod::BunBinary {
        return Ok(format!(
            "Download from: {}/releases/latest",
            env!("CARGO_PKG_REPOSITORY")
        ));
    }
    let update = update_package_name.unwrap_or(package_name);
    if let Some(command) = command_for_method(context, package_name, npm_command, update) {
        let method = match method {
            InstallMethod::Npm => "npm",
            InstallMethod::Pnpm => "pnpm",
            InstallMethod::Yarn => "yarn",
            InstallMethod::Bun => "bun",
            _ => "unknown",
        };
        if managed(context, package_name, npm_command)? && !writable(context) {
            return Ok(format!(
                "This installation is managed by a global {method} install, but the install path is not writable. Update it yourself with: {}",
                command.display
            ));
        }
        return Ok(format!(
            "This installation is not managed by a global {method} install. Update it with the package manager, wrapper, or source checkout that provides it."
        ));
    }
    Ok(format!(
        "Update {update} using the package manager, wrapper, or source checkout that provides this installation."
    ))
}
/// Describe a manual command without probing ownership or write access.
///
/// # Errors
/// Returns failures from unavailable-instruction selection when no command exists.
pub fn get_update_instruction(
    context: &SelfUpdateContext<'_>,
    package_name: &str,
) -> io::Result<String> {
    if let Some(command) = command_for_method(context, package_name, None, package_name) {
        return Ok(format!("Run: {}", command.display));
    }
    get_self_update_unavailable_instruction(context, package_name, None, None)
}

/// Combine home-based and query-based Bun roots, preserving observation order.
fn bun_roots(context: &SelfUpdateContext<'_>, output: Option<String>) -> io::Result<Vec<String>> {
    let windows = context.operations.is_windows();
    let mut roots = vec![join(
        &[
            &maestro_tools::expand_path("~")?,
            ".bun",
            "install",
            "global",
            "node_modules",
        ],
        windows,
    )];
    if let Some(bin) = output {
        roots.push(join(
            &[&dirname(&bin, windows), "install", "global", "node_modules"],
            windows,
        ));
    }
    Ok(roots)
}

/// Authored query context retaining the process owner's original error chain.
#[derive(Debug)]
struct QueryFailure {
    /// Complete user-facing query diagnostic.
    message: String,
    /// Original process error, when capture did not complete.
    source: Option<SpawnSyncError>,
}
impl std::fmt::Display for QueryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for QueryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &dyn std::error::Error)
    }
}

/// Split configured npm argv, treating an empty vector as unconfigured.
fn npm_program(npm: Option<&[String]>) -> (&str, Vec<String>) {
    npm.and_then(|args| args.split_first()).map_or_else(
        || ("npm", Vec::new()),
        |(command, args)| (command.as_str(), args.to_vec()),
    )
}

/// Use the OS access operation rather than creating or truncating a probe file.
#[cfg(unix)]
fn check_write_access(path: &str) -> io::Result<()> {
    Ok(rustix::fs::access(path, rustix::fs::Access::WRITE_OK)?)
}

/// Match native Windows directory and regular-file attribute access results.
///
/// Windows observes existence and read-only attributes, not directory ACLs, so an
/// attempted update can still fail with a permissions error.
#[cfg(not(unix))]
fn check_write_access(path: &str) -> io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    if metadata.is_dir() || !metadata.permissions().readonly() {
        Ok(())
    } else {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

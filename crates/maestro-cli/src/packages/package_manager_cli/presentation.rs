//! Visible lines of the package commands.

use super::PackageCommandContext;
use super::help::usage;
use super::parse::{PackageCommand, PackageCommandOptions};
use maestro_app::{
    APP_NAME, ConfiguredPackage, InstalledSourceScope, PackageManager, ProgressEvent,
    ProgressEventType, Settings, SettingsManager,
};
use maestro_theme::Theme;
use std::{cell::RefCell, io, io::Write, rc::Rc};

/// Write `text` and one line feed to `writer`.
pub(crate) fn line(writer: &Rc<RefCell<dyn Write>>, text: &str) -> io::Result<()> {
    writeln!(writer.borrow_mut(), "{text}")
}

/// Write a green status line to standard output.
pub(super) fn succeeded(text: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    line(&context.stdout, &Theme::green(text, context.styles_enabled))
}

/// Write a red diagnostic line to standard error.
pub(super) fn failed(text: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    line(&context.stderr, &Theme::red(text, context.styles_enabled))
}

/// Write the application's failure line for an operation error.
pub(super) fn failure(error: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    failed(&format!("Error: {error}"), context)
}

/// The diagnostic that stops `options` before any observation, as its primary and
/// secondary lines.
pub(super) fn diagnostic(options: &PackageCommandOptions) -> Option<(String, String)> {
    let command = options.command;
    let secondary = || format!("Usage: {}", usage(command));
    if let Some(option) = &options.invalid_option {
        return Some((
            format!("Unknown option {option} for \"{}\".", command.name()),
            format!("Use \"{APP_NAME} --help\" or \"{}\".", usage(command)),
        ));
    }
    if let Some(option) = &options.missing_option_value {
        return Some((format!("Missing value for {option}."), secondary()));
    }
    if let Some(argument) = &options.invalid_argument {
        return Some((format!("Unexpected argument {argument}."), secondary()));
    }
    if let Some(conflict) = &options.conflicting_options {
        return Some((conflict.clone(), secondary()));
    }
    let missing = matches!(command, PackageCommand::Install | PackageCommand::Remove)
        && options.required_source().is_none();
    missing.then(|| (format!("Missing {} source.", command.name()), secondary()))
}

/// Write one diagnostic as its red primary line and dim secondary line.
pub(super) fn report(
    diagnostic: &(String, String),
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let styles = context.styles_enabled;
    line(&context.stderr, &Theme::red(&diagnostic.0, styles))?;
    line(&context.stderr, &Theme::dim(&diagnostic.1, styles))
}

/// Drain the recorded settings failures of `settings` as warning lines.
///
/// The settings borrow is released before anything is written, so a caller that
/// writes to the settings from its output handle still succeeds.
pub(super) fn warn(
    settings: &Rc<RefCell<SettingsManager>>,
    place: &str,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let errors = settings.borrow_mut().drain_errors();
    for error in errors {
        let scope = error.scope.label();
        let stack = error
            .error
            .source()
            .map(ToString::to_string)
            .filter(|stack| !stack.is_empty());
        let text = format!("Warning ({place}, {scope} settings): {}", error.error);
        line(
            &context.stderr,
            &Theme::yellow(&text, context.styles_enabled),
        )?;
        if let Some(stack) = stack {
            line(&context.stderr, &Theme::dim(&stack, context.styles_enabled))?;
        }
    }
    Ok(())
}

/// Install a progress writer that prints only the start message of each event.
pub(super) fn set_progress(manager: &Rc<dyn PackageManager>, context: &PackageCommandContext<'_>) {
    let stdout = Rc::clone(&context.stdout);
    let styles = context.styles_enabled;
    manager.set_progress_callback(Some(Rc::new(move |event: &ProgressEvent| {
        if event.r#type != ProgressEventType::Start {
            return Ok(());
        }
        let message = event.message.as_deref().unwrap_or("undefined");
        line(&stdout, &Theme::dim(message, styles))
    })));
}

/// Write one configured package row and its optional installed path.
fn package_row(package: &ConfiguredPackage, context: &PackageCommandContext<'_>) -> io::Result<()> {
    let display = if package.filtered {
        format!("  {} (filtered)", package.source)
    } else {
        format!("  {}", package.source)
    };
    line(&context.stdout, &display)?;
    match package.installed_path.as_deref() {
        Some(path) if !path.is_empty() => line(
            &context.stdout,
            &Theme::dim(&format!("    {path}"), context.styles_enabled),
        ),
        Some(_) | None => Ok(()),
    }
}

/// Write one scope heading followed by its rows.
fn scope_rows(
    heading: &str,
    packages: &[&ConfiguredPackage],
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    line(
        &context.stdout,
        &Theme::bold(heading, context.styles_enabled),
    )?;
    for package in packages {
        package_row(package, context)?;
    }
    Ok(())
}

/// Write the configured packages in user-then-project order.
pub(super) fn list(
    manager: &Rc<dyn PackageManager>,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let packages = manager.list_configured_packages()?;
    if packages.is_empty() {
        return line(
            &context.stdout,
            &Theme::dim("No packages installed.", context.styles_enabled),
        );
    }
    let of_scope = |scope| {
        packages
            .iter()
            .filter(move |package| package.scope == scope)
            .collect::<Vec<_>>()
    };
    let user = of_scope(InstalledSourceScope::User);
    let project = of_scope(InstalledSourceScope::Project);
    if !user.is_empty() {
        scope_rows("User packages:", &user, context)?;
    }
    if !project.is_empty() {
        if !user.is_empty() {
            line(&context.stdout, "")?;
        }
        scope_rows("Project packages:", &project, context)?;
    }
    Ok(())
}

/// Write the completed application update.
pub(super) fn application_updated(context: &PackageCommandContext<'_>) -> io::Result<()> {
    succeeded(&format!("Updated {APP_NAME}"), context)
}

/// Write the version that needs no update.
pub(super) fn up_to_date(version: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    succeeded(
        &format!("{APP_NAME} is already up to date (v{version})"),
        context,
    )
}

/// Write the command that is about to run.
pub(super) fn updating(display: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    line(
        &context.stdout,
        &Theme::dim(
            &format!("Updating {APP_NAME} with {display}..."),
            context.styles_enabled,
        ),
    )
}

/// Write the manual command for a failing update.
pub(super) fn manual_command(display: &str, context: &PackageCommandContext<'_>) -> io::Result<()> {
    line(
        &context.stderr,
        &Theme::dim(
            &format!("If this keeps failing, run this command yourself: {display}"),
            context.styles_enabled,
        ),
    )
}

/// Write the explanation of an installation that cannot update itself.
pub(super) fn unavailable(
    package: &str,
    settings: &Settings,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    line(
        &context.stderr,
        &format!("error: {APP_NAME} cannot self-update this installation."),
    )?;
    let instruction = (context.self_update_unavailable_instruction)(settings, package)?;
    line(&context.stderr, &instruction)?;
    match context.entrypoint {
        Some(entrypoint) => {
            line(&context.stderr, "")?;
            line(
                &context.stderr,
                &format!("Location of {APP_NAME} executable: {entrypoint}"),
            )
        }
        None => Ok(()),
    }
}

//! Package commands translated into application operations.

mod help;
mod parse;
pub(crate) mod presentation;

pub use parse::PackageCommand;

use super::package_manager_cli::parse::{PackageCommandOptions, UpdateTarget};
use crate::input::config_selector::{ConfigPresentation, ConfigSelectorOptions, select_config};
use maestro_app::configuration::config::{
    SelfUpdateCommand, SelfUpdateOperations, get_self_update_plan, run_self_update,
};
use maestro_app::configuration::version_check::VersionCheckOptions;
use maestro_app::{
    InstalledSourceScope, PackageManager, PackageManagerOptions, Settings, SettingsManager,
};
use std::cell::{Cell, RefCell};
use std::{io, rc::Rc};

/// The loader of the shared settings owner, keyed by two authored directories.
pub type SettingsLoader<'a> = &'a dyn Fn(&str, &str) -> io::Result<Rc<RefCell<SettingsManager>>>;

/// Replaceable observations, operations and outputs of one package command.
pub struct PackageCommandContext<'a> {
    /// The working directory, observed only where an operation needs it.
    pub cwd: &'a dyn Fn() -> io::Result<String>,
    /// The user configuration directory, observed only where an operation needs it.
    pub agent_dir: &'a dyn Fn() -> io::Result<String>,
    /// Loads the shared settings owner over those two directories.
    pub load_settings: SettingsLoader<'a>,
    /// Builds the package manager over the same settings owner.
    pub create_package_manager:
        &'a dyn Fn(PackageManagerOptions) -> io::Result<Rc<dyn PackageManager>>,
    /// Selects an owned update command from a settings snapshot.
    pub select_self_update_command:
        &'a dyn Fn(&Settings, &str) -> io::Result<Option<SelfUpdateCommand>>,
    /// Explains why this installation cannot update itself.
    pub self_update_unavailable_instruction: &'a dyn Fn(&Settings, &str) -> io::Result<String>,
    /// Runs the selected update steps.
    pub self_update_operations: &'a dyn SelfUpdateOperations,
    /// The release request and its transport.
    pub version_options: &'a VersionCheckOptions,
    /// The service origin, empty when none is configured.
    pub service_origin: &'a str,
    /// Standard output of the command.
    pub stdout: Rc<RefCell<dyn io::Write>>,
    /// Standard error of the command.
    pub stderr: Rc<RefCell<dyn io::Write>>,
    /// Status the command sets instead of terminating the process.
    pub exit_code: &'a Cell<Option<i32>>,
    /// The running executable, when the caller knows it.
    pub entrypoint: Option<&'a str>,
    /// Whether terminal styles are enabled.
    pub styles_enabled: bool,
}

/// The prepared operations of one handled command.
struct Prepared {
    /// The supplied package manager.
    manager: Rc<dyn PackageManager>,
    /// The retained raw global settings, taken before the manager was built.
    settings: Settings,
    /// The same settings owner whose queued writes must finish before command exit.
    settings_manager: Rc<RefCell<SettingsManager>>,
}

/// Load settings, report their failures and build the manager in that order.
fn prepare(context: &PackageCommandContext<'_>) -> io::Result<Prepared> {
    let cwd = (context.cwd)()?;
    let agent_dir = (context.agent_dir)()?;
    let settings = (context.load_settings)(&cwd, &agent_dir)?;
    presentation::warn(&settings, "package command", context)?;
    let snapshot = settings.borrow().get_global_settings();
    let manager = (context.create_package_manager)(PackageManagerOptions {
        cwd,
        agent_dir,
        settings_manager: Rc::clone(&settings),
    })?;
    Ok(Prepared {
        manager,
        settings: snapshot,
        settings_manager: settings,
    })
}

/// The settings scope a command writes to.
fn scope(local: bool) -> Option<InstalledSourceScope> {
    local.then_some(InstalledSourceScope::Project)
}

/// Install the positional source.
async fn install(
    options: &PackageCommandOptions,
    prepared: &Prepared,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let Some(source) = options.required_source() else {
        return Ok(());
    };
    prepared
        .manager
        .install_and_persist(source, scope(options.local))
        .await?;
    presentation::succeeded(&format!("Installed {source}"), context)
}

/// Remove the positional source, reporting a registration that did not exist.
async fn remove(
    options: &PackageCommandOptions,
    prepared: &Prepared,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let Some(source) = options.required_source() else {
        return Ok(());
    };
    if prepared
        .manager
        .remove_and_persist(source, scope(options.local))
        .await?
    {
        return presentation::succeeded(&format!("Removed {source}"), context);
    }
    presentation::failed(&format!("No matching package found for {source}"), context)?;
    context.exit_code.set(Some(1));
    Ok(())
}

/// Update the application through the supplied plan and runner.
async fn self_update(
    force: bool,
    prepared: &Prepared,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let plan = get_self_update_plan(force, context.service_origin, context.version_options).await;
    if !plan.should_run {
        return presentation::up_to_date(maestro_app::configuration::config::VERSION, context);
    }
    let Some(command) =
        (context.select_self_update_command)(&prepared.settings, &plan.package_name)?
    else {
        presentation::unavailable(&plan.package_name, &prepared.settings, context)?;
        context.exit_code.set(Some(1));
        return Ok(());
    };
    presentation::updating(&command.display, context)?;
    if let Err(error) = run_self_update(&command, context.self_update_operations).await {
        presentation::failure(&error.to_string(), context)?;
        presentation::manual_command(&command.display, context)?;
        context.exit_code.set(Some(1));
        return Ok(());
    }
    presentation::application_updated(context)
}

/// Update the selected packages, then the application.
async fn update(
    options: &PackageCommandOptions,
    prepared: &Prepared,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let fallback = UpdateTarget::All;
    let target = options.update_target.as_ref().unwrap_or(&fallback);
    let source = match target {
        UpdateTarget::Extensions(source) => source.as_deref(),
        UpdateTarget::All | UpdateTarget::SelfOnly => None,
    };
    if target.includes_extensions() {
        Rc::clone(&prepared.manager).update(source).await?;
        let text = source.map_or_else(
            || "Updated packages".to_owned(),
            |source| format!("Updated {source}"),
        );
        presentation::succeeded(&text, context)?;
    }
    if target.includes_self() {
        self_update(options.force, prepared, context).await?;
    }
    Ok(())
}

/// Run the recognized command over the prepared operations.
async fn operate(
    options: &PackageCommandOptions,
    prepared: &Prepared,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    match options.command {
        PackageCommand::Install => install(options, prepared, context).await,
        PackageCommand::Remove => remove(options, prepared, context).await,
        PackageCommand::List => presentation::list(&prepared.manager, context),
        PackageCommand::Update => update(options, prepared, context).await,
    }
}

/// Handle the configuration command, reporting whether the arguments belonged to it.
///
/// Only a leading `config` is handled and later tokens are ignored. The selector runs over
/// the resolved configured paths and, once it settles, the command asks the presentation's
/// exit effect for status 0.
///
/// # Errors
/// Returns working-directory, settings-load, manager-construction, resolution and
/// presentation failures, which the caller reports as it does for its own operations.
pub async fn handle_config_command(
    args: &[String],
    context: &PackageCommandContext<'_>,
    presentation: ConfigPresentation,
) -> io::Result<bool> {
    if args.first().map(String::as_str) != Some("config") {
        return Ok(false);
    }
    let cwd = (context.cwd)()?;
    let agent_dir = (context.agent_dir)()?;
    let settings = (context.load_settings)(&cwd, &agent_dir)?;
    presentation::warn(&settings, "config command", context)?;
    let manager = (context.create_package_manager)(PackageManagerOptions {
        cwd: cwd.clone(),
        agent_dir: agent_dir.clone(),
        settings_manager: Rc::clone(&settings),
    })?;
    let resolved_paths = manager.resolve(None).await?;
    let exit = Rc::clone(&presentation.host.exit);
    select_config(
        ConfigSelectorOptions {
            resolved_paths,
            settings_manager: settings,
            cwd,
            agent_dir,
        },
        presentation,
    )
    .await?;
    exit(0);
    Ok(true)
}

/// Handle one package command, reporting whether the arguments belonged to one.
///
/// # Errors
/// Returns working-directory, settings-load, manager-construction and output failures,
/// which the caller reports as it does for its own required operations.
pub async fn handle_package_command(
    args: &[String],
    context: &PackageCommandContext<'_>,
) -> io::Result<bool> {
    let Some(options) = parse::parse_command(args) else {
        return Ok(false);
    };
    if options.help {
        help::print_help(options.command, context)?;
        return Ok(true);
    }
    if let Some(diagnostic) = presentation::diagnostic(&options) {
        presentation::report(&diagnostic, context)?;
        context.exit_code.set(Some(1));
        return Ok(true);
    }
    let prepared = prepare(context)?;
    presentation::set_progress(&prepared.manager, context);
    let outcome = operate(&options, &prepared, context).await;
    let flush = prepared.settings_manager.borrow().flush();
    flush.await;
    match outcome {
        Ok(()) => Ok(true),
        Err(error) => {
            presentation::failure(&error.to_string(), context)?;
            context.exit_code.set(Some(1));
            Ok(true)
        }
    }
}

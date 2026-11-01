//! Selected self-updates: release plan, settings adaptation and ordered execution.

use super::{SelfUpdateCommand, SelfUpdateCommandStep, SelfUpdateContext, SelfUpdateOperations};
use crate::Settings;
use crate::configuration::{
    config::{PACKAGE_NAME, VERSION},
    version_check::{VersionCheckOptions, get_latest_maestro_release, is_newer_package_version},
};
use serde::Deserialize;
use std::{io, process::ExitStatus};

/// The distribution identity and whether installation should run.
#[derive(Debug, PartialEq, Eq)]
pub struct SelfUpdatePlan {
    /// The selected distribution identity.
    pub package_name: String,
    /// Whether installation should run.
    pub should_run: bool,
}

/// Select the current or advertised distribution identity without presentation.
pub async fn get_self_update_plan(
    force: bool,
    origin: &str,
    options: &VersionCheckOptions,
) -> SelfUpdatePlan {
    if force {
        return installable(PACKAGE_NAME.into());
    }
    let Ok(Some(release)) = get_latest_maestro_release(VERSION, origin, options).await else {
        return installable(PACKAGE_NAME.into());
    };
    let package_name = release.package_name.unwrap_or_else(|| PACKAGE_NAME.into());
    if package_name != PACKAGE_NAME {
        return installable(package_name);
    }
    let Ok(collator) = maestro_text::collation::Collator::new(&|key| std::env::var(key).ok())
    else {
        return installable(package_name);
    };
    SelfUpdatePlan {
        should_run: is_newer_package_version(&release.version, VERSION, &collator),
        package_name,
    }
}

/// An installation of `package_name` that has not been skipped.
fn installable(package_name: String) -> SelfUpdatePlan {
    SelfUpdatePlan {
        package_name,
        should_run: true,
    }
}

/// Decode only the field consumed by the selected installation method.
fn npm_command(
    context: &SelfUpdateContext<'_>,
    settings: &Settings,
) -> io::Result<Option<Vec<String>>> {
    use super::super::installation::{InstallMethod, detect_install_method};
    if detect_install_method(context.installation) != InstallMethod::Npm {
        return Ok(None);
    }
    settings
        .0
        .get("npmCommand")
        .filter(|value| !value.is_null())
        .map(|value| Vec::<String>::deserialize(value).map_err(io::Error::other))
        .transpose()
}

/// Select an update using the retained raw global preference snapshot.
///
/// # Errors
/// Returns the typed error of a consumed package-manager preference.
pub fn get_self_update_command_from_settings(
    context: &SelfUpdateContext<'_>,
    settings: &Settings,
    update_package_name: &str,
) -> io::Result<Option<SelfUpdateCommand>> {
    let npm = npm_command(context, settings)?;
    super::get_self_update_command(
        context,
        PACKAGE_NAME,
        npm.as_deref(),
        Some(update_package_name),
    )
}

/// Explain an unavailable update using that same retained snapshot.
///
/// # Errors
/// Returns the typed error of a consumed package-manager preference.
pub fn get_self_update_unavailable_instruction_from_settings(
    context: &SelfUpdateContext<'_>,
    settings: &Settings,
    update_package_name: &str,
) -> io::Result<String> {
    let npm = npm_command(context, settings)?;
    super::get_self_update_unavailable_instruction(
        context,
        PACKAGE_NAME,
        npm.as_deref(),
        Some(update_package_name),
    )
}

/// Execute each selected step in order with inherited I/O and no deadline.
///
/// # Errors
/// Returns the first step's spawn or unsuccessful-status failure; later steps do not run.
pub async fn run_self_update(
    command: &SelfUpdateCommand,
    operations: &dyn SelfUpdateOperations,
) -> io::Result<()> {
    match &command.steps {
        Some(steps) => {
            for step in steps {
                run_step(step, operations).await?;
            }
            Ok(())
        }
        None => check_status(
            operations
                .run_command(&command.command, &command.args)
                .await?,
            &command.display,
        ),
    }
}

/// Run one selected step without reconstructing its operands.
async fn run_step(
    step: &SelfUpdateCommandStep,
    operations: &dyn SelfUpdateOperations,
) -> io::Result<()> {
    check_status(
        operations.run_command(&step.command, &step.args).await?,
        &step.display,
    )
}

/// Preserve native child outcome categories in the application error.
fn check_status(status: ExitStatus, display: &str) -> io::Result<()> {
    if status.success() {
        return Ok(());
    }
    let reason = maestro_process::exit_signal_name(status).map_or_else(
        || {
            format!(
                "exited with code {}",
                status
                    .code()
                    .map_or_else(|| "unknown".into(), |code| code.to_string())
            )
        },
        |name| format!("terminated by signal {name}"),
    );
    Err(io::Error::other(format!("{display} {reason}")))
}

/// Build the inherited-I/O launch of one selected step.
///
/// The operands stay literal: native spawning already launches a Windows batch shim
/// through its shell, so joining them into command text would let an authored prefix
/// like `C:\R&D` split and would drop an empty operand.
pub(super) fn native_launch(command: &str, args: &[String]) -> maestro_process::Launch {
    maestro_process::Launch {
        executable: command.into(),
        args: args.to_vec(),
        verbatim: false,
    }
}

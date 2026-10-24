//! Complete help documents of the package commands.

use super::PackageCommandContext;
use super::parse::PackageCommand;
use maestro_app::{APP_NAME, CONFIG_DIR_NAME};
use maestro_theme::Theme;
use std::io;

/// The usage line of `command`.
pub(super) fn usage(command: PackageCommand) -> String {
    match command {
        PackageCommand::Install | PackageCommand::Remove => {
            format!("{APP_NAME} {} <source> [-l]", command.name())
        }
        PackageCommand::Update => format!(
            "{APP_NAME} update [source|self|maestro] [--self] [--extensions] [--extension <source>] [--force]"
        ),
        PackageCommand::List => format!("{APP_NAME} list"),
    }
}

/// The complete install document.
fn install(styles: bool) -> String {
    format!(
        "{}
  {}

Install a package and add it to settings.

Options:
  -l, --local    Install project-locally ({CONFIG_DIR_NAME}/settings.json)

Examples:
  {APP_NAME} install npm:@foo/bar
  {APP_NAME} install git:github.com/user/repo
  {APP_NAME} install git:git@github.com:user/repo
  {APP_NAME} install https://github.com/user/repo
  {APP_NAME} install ssh://git@github.com/user/repo
  {APP_NAME} install ./local/path
",
        Theme::bold("Usage:", styles),
        usage(PackageCommand::Install)
    )
}

/// The complete remove document.
fn remove(styles: bool) -> String {
    format!(
        "{}
  {}

Remove a package and its source from settings.
Alias: {APP_NAME} uninstall <source> [-l]

Options:
  -l, --local    Remove from project settings ({CONFIG_DIR_NAME}/settings.json)

Examples:
  {APP_NAME} remove npm:@foo/bar
  {APP_NAME} uninstall npm:@foo/bar
",
        Theme::bold("Usage:", styles),
        usage(PackageCommand::Remove)
    )
}

/// The complete update document.
fn update(styles: bool) -> String {
    format!(
        "{}
  {}

Update {APP_NAME} and installed packages.

Options:
  --self                  Update {APP_NAME} only
  --extensions            Update installed packages only
  --extension <source>    Update one package only
  --force                 Reinstall {APP_NAME} even if the current version is latest

Short forms:
  {APP_NAME} update                Update {APP_NAME} and all extensions
  {APP_NAME} update <source>       Update one package
  {APP_NAME} update maestro        Update {APP_NAME} only (self works as alias to maestro)
",
        Theme::bold("Usage:", styles),
        usage(PackageCommand::Update)
    )
}

/// The complete list document.
fn list(styles: bool) -> String {
    format!(
        "{}
  {}

List installed packages from user and project settings.
",
        Theme::bold("Usage:", styles),
        usage(PackageCommand::List)
    )
}

/// Write the complete help document of `command`.
pub(super) fn print_help(
    command: PackageCommand,
    context: &PackageCommandContext<'_>,
) -> io::Result<()> {
    let document = match command {
        PackageCommand::Install => install(context.styles_enabled),
        PackageCommand::Remove => remove(context.styles_enabled),
        PackageCommand::Update => update(context.styles_enabled),
        PackageCommand::List => list(context.styles_enabled),
    };
    writeln!(context.stdout.borrow_mut(), "{document}")
}

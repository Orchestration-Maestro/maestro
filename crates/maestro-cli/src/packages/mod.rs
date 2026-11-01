//! Package management commands presented over the application operations.

pub(crate) mod package_manager_cli;

pub use package_manager_cli::{
    PackageCommand, PackageCommandContext, handle_config_command, handle_package_command,
};

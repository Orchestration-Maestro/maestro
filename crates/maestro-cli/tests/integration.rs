//! Shared integration-test executable.
#![cfg(test)]

mod file_inputs;
mod initial_inputs;
mod input_arguments;
mod input_help;
mod input_presenters;
mod model_account_checks;
mod model_tables;
mod package_command_checks;
mod print_output;
mod startup_bootstrap;
mod startup_config;
mod startup_dispatch;
mod startup_entry;
mod startup_options;
mod startup_runtime;
mod startup_sessions;
mod startup_stdout;
mod support;

mod startup_mode_dispatch;
mod workspace_settings;

mod workspace_discovery;

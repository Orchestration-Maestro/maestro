//! The doctest phase of `just test`, skipped for a selection with no library to document.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use cargo_metadata::{Metadata, MetadataCommand};

use crate::exit_code;

#[cfg(test)]
mod tests;

/// Run the Cargo doctests for the caller's arguments, or the whole workspace when no package is named.
///
/// Cargo refuses `--doc` for a selection without a library target, so a selection of packages
/// that have none passes without running Cargo's doctests.
///
/// # Errors
/// Returns a metadata read or command failure.
pub(crate) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let names = package_names(args);
    if !names.is_empty() {
        let metadata = MetadataCommand::new()
            .cargo_path(cargo)
            .current_dir(checkout)
            .no_deps()
            .exec()
            .map_err(io::Error::other)?;
        if !runs_doctests(&metadata, &names) {
            println!("no library to document in the selection");
            return Ok(ExitCode::SUCCESS);
        }
    }
    let mut child = Command::new(cargo);
    child
        .current_dir(checkout)
        .args(["test", "--locked", "--doc"]);
    if names.is_empty() {
        child.arg("--workspace");
    }
    Ok(exit_code(child.args(args).status()?))
}

/// Whether Cargo has a doctest to run for `names`.
///
/// A name that is no member goes to Cargo, which reports it.
fn runs_doctests(metadata: &Metadata, names: &[String]) -> bool {
    names.iter().any(|name| {
        metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == name)
            .is_none_or(|package| package.targets.iter().any(|target| target.doctest))
    })
}

/// The packages `-p NAME`, `-pNAME`, `--package NAME` and `--package=NAME` name before `--`.
fn package_names(args: &[OsString]) -> Vec<String> {
    let mut names = Vec::new();
    let mut remaining = args.iter().filter_map(|argument| argument.to_str());
    while let Some(argument) = remaining.next() {
        if argument == "--" {
            break;
        }
        if let Some(name) = argument.strip_prefix("--package=") {
            names.push(name.to_owned());
        } else if argument == "-p" || argument == "--package" {
            names.extend(remaining.next().map(str::to_owned));
        } else if let Some(name) = argument.strip_prefix("-p") {
            names.push(name.to_owned());
        }
    }
    names
}

//! Lint the declared browser crates for the browser target.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::exit_code;

#[cfg(test)]
mod tests;

/// The caller's declaration of the crates that build for the browser target.
#[derive(serde::Deserialize)]
struct Declaration {
    /// Every workspace member the browser build compiles.
    wasm_crates: Vec<String>,
}

/// Lint the declared browser crates the package selection names, for the browser target.
///
/// Only library targets are linted: an integration test may name a crate that
/// exists on native targets alone.
///
/// # Errors
/// Returns a configuration, command or filesystem failure.
pub(crate) fn run(args: &[OsString], cargo: &Path, checkout: &Path) -> io::Result<ExitCode> {
    let selection = selection(args)?;
    let declared = declaration(checkout)?;
    let packages = linted(&declared, selection.as_deref());
    if packages.is_empty() {
        println!("no declared browser crate in the selection");
        return Ok(ExitCode::SUCCESS);
    }
    let mut child = Command::new(cargo);
    child.current_dir(checkout).args(arguments(&packages));
    Ok(exit_code(child.status()?))
}

/// The shared CI lint command over `packages`, one package argument per crate.
fn arguments(packages: &[&str]) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["clippy", "--locked", "--target", "wasm32-unknown-unknown"]
        .map(OsString::from)
        .into();
    for package in packages {
        arguments.push(OsString::from("-p"));
        arguments.push(OsString::from(*package));
    }
    arguments.extend(["--", "-D", "warnings"].map(OsString::from));
    arguments
}

/// The browser crates the caller declares, in declaration order.
fn declaration(checkout: &Path) -> io::Result<Vec<String>> {
    let path = checkout.join(".github/ci.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
    let declared: Declaration = toml::from_str(&text)
        .map_err(|error| io::Error::other(format!("{}: {error}", path.display())))?;
    Ok(declared.wasm_crates)
}

/// The declared crates to lint, in declaration order.
///
/// No selection lints the whole workspace, as Cargo does without a package argument.
fn linted<'a>(declared: &'a [String], selection: Option<&[String]>) -> Vec<&'a str> {
    declared
        .iter()
        .filter(|package| selection.is_none_or(|names| names.contains(package)))
        .map(String::as_str)
        .collect()
}

/// The package names the Cargo arguments select, or none for the whole workspace.
fn selection(args: &[OsString]) -> io::Result<Option<Vec<String>>> {
    let mut names = Vec::new();
    let mut whole = args.is_empty();
    let mut remaining = args.iter();
    while let Some(argument) = remaining.next() {
        let argument = text(argument)?;
        if argument == "--workspace" {
            whole = true;
        } else if let Some(name) = argument.strip_prefix("--package=") {
            names.push(package(name)?.to_owned());
        } else if argument == "-p" || argument == "--package" {
            let name = remaining
                .next()
                .ok_or_else(|| io::Error::other(format!("{argument} names no package")))?;
            names.push(package(text(name)?)?.to_owned());
        } else if let Some(name) = argument.strip_prefix("-p") {
            names.push(package(name)?.to_owned());
        } else {
            return Err(io::Error::other(format!(
                "the browser lint selects on no argument {argument}"
            )));
        }
    }
    Ok((!whole).then_some(names))
}

/// One package name, refusing the Cargo patterns and versions it cannot match to a declared crate.
fn package(name: &str) -> io::Result<&str> {
    if name.contains(['*', '?', '@', '[']) {
        return Err(io::Error::other(format!(
            "the browser lint cannot narrow the package name {name}"
        )));
    }
    Ok(name)
}

/// Read one Cargo argument as text.
fn text(argument: &OsStr) -> io::Result<&str> {
    argument
        .to_str()
        .ok_or_else(|| io::Error::other("the package selection is not text"))
}

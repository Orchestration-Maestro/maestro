//! The setup commands nextest runs before the tests that need them.
//!
//! Each setup exports its variables by appending `NAME=value` lines to the file that
//! `NEXTEST_ENV` names, so the tests it serves inherit them.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use maestro_path::normalize_native;

use super::argument;
use crate::test_setup::{MANIFEST, cargo_checks, executable, native_fixture};
use crate::workspace::Workspace;

/// The bundle directory a run reads when it names none.
const BUNDLE: &str = "target/test-setup";

/// Prepare and launch the component-discovery example.
mod author_component;
/// Prepare the browser build the browser author tests share.
pub(crate) mod browser_graph;
/// Vendor locked dependencies for tooling fixtures or the complete workspace.
mod fixture_sources;
/// Align the modification times of the executables the example warms code for.
mod normalize;

/// Run the setup named by the first argument in the current directory, where nextest starts
/// its setup scripts: the workspace root.
///
/// # Errors
/// Returns an unknown or incomplete setup name, or the failure of the setup.
pub(crate) fn run(args: &[OsString]) -> io::Result<ExitCode> {
    let checkout = normalize_native(&std::env::current_dir()?);
    match args.first().and_then(|name| name.to_str()) {
        Some("author-component") => author_component::run(Path::new(&cargo()), &checkout),
        Some("fixture-sources") => {
            let scope = match args.get(1).and_then(|scope| scope.to_str()) {
                None if args.len() == 1 => fixture_sources::Scope::Fixtures,
                Some("workspace") if args.len() == 2 => fixture_sources::Scope::Workspace,
                _ => return Err(io::Error::other("unknown fixture source scope")),
            };
            fixture_sources::run(&checkout, scope)
        }
        Some("native-fixture") => native(&checkout),
        Some("cargo-checks") => checks(&checkout),
        Some("executable") => executable::run(Path::new(&cargo()), &checkout),
        Some("browser-author") => browser_graph::run(&checkout),
        Some("normalize-component-cache") => normalize::run(
            &checkout.join(argument(args, 1, "directory")?),
            normalize::archive_signalled(),
        ),
        _ => Err(io::Error::other("unknown nextest setup")),
    }
}

/// Export the validated host fixtures, building only during local repository setup.
fn native(checkout: &Path) -> io::Result<ExitCode> {
    let paths = if let Some(bundle) = prepared_bundle(checkout) {
        native_fixture::prepared(checkout, &bundle)?
    } else {
        let cargo = Path::new(&cargo()).to_path_buf();
        native_fixture::prepare(
            &cargo,
            checkout,
            &Workspace::read(&cargo, checkout)?,
            &checkout.join(BUNDLE),
            &checkout.join("target/native-fixture"),
        )?
    };
    for (variable, path) in paths {
        let mut entry = OsString::from(format!("{variable}="));
        entry.push(path);
        export(&entry)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Export the prepared build checks after strict bundle validation or local preparation.
fn checks(checkout: &Path) -> io::Result<ExitCode> {
    let paths = if let Some(bundle) = prepared_bundle(checkout) {
        cargo_checks::prepared(checkout, &bundle)?
    } else {
        let cargo = Path::new(&cargo()).to_path_buf();
        cargo_checks::prepare(
            &cargo,
            checkout,
            &Workspace::read(&cargo, checkout)?,
            &checkout.join(BUNDLE),
            &checkout.join("target/cargo-checks"),
        )?
    };
    for (variable, path) in paths {
        let mut entry = OsString::from(format!("{variable}="));
        entry.push(path);
        export(&entry)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The bundle directory a prepared run reads: the one `TEST_PREPARE_OUTPUT` names while it holds
/// a manifest, else the default of an archived run. A run without a bundle has none.
pub(crate) fn prepared_bundle(checkout: &Path) -> Option<PathBuf> {
    let named = nonempty("TEST_PREPARE_OUTPUT").map(PathBuf::from);
    let archived = std::env::var_os("TEST_ARCHIVE").is_some_and(|value| value == "1");
    let prepared = archived
        || named
            .as_ref()
            .is_some_and(|output| checkout.join(output).join(MANIFEST).is_file());
    prepared.then(|| checkout.join(named.unwrap_or_else(|| BUNDLE.into())))
}

/// The Cargo executable the setup was given, else the one on the search path.
fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The value of an environment variable that is set and not empty.
fn nonempty(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// Export one `NAME=value` entry to the tests, when nextest supplied a file for it.
pub(crate) fn export(entry: &OsStr) -> io::Result<()> {
    let Some(file) = std::env::var_os("NEXTEST_ENV") else {
        return Ok(());
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)?;
    file.write_all(entry.as_encoded_bytes())?;
    file.write_all(b"\n")
}

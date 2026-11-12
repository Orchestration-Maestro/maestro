//! Fetch the package repository's example tree into the checkout.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::test_setup::{at, remove_directory};

/// The package repository that hosts the examples.
const SOURCE: &str = "https://github.com/Orchestration-Maestro/maestro-packages.git";
/// The folder the tree is fetched into and read from, relative to the checkout.
const TREE: &str = "examples";
/// The scratch repository a fetch builds the new tree in, relative to the checkout.
const STAGING: &str = ".examples-fetch";
/// The previous tree while a new one takes its place, relative to the checkout.
const PREVIOUS: &str = ".examples-previous";
/// The usage line printed for malformed arguments.
const USAGE: &str = "usage: examples [<ref>] [--source <repository>]";

/// What to fetch and where from.
struct Request {
    /// The branch, tag or commit to fetch; the source's default branch when absent.
    reference: Option<OsString>,
    /// The repository location handed to `git`.
    source: OsString,
}

impl Request {
    /// Read the optional reference and the optional source override.
    fn parse(args: &[OsString]) -> io::Result<Self> {
        let mut reference = None;
        let mut source = None;
        let mut args = args.iter();
        while let Some(argument) = args.next() {
            let slot = if argument == "--source" {
                args.next().map(|value| (&mut source, value))
            } else {
                Some((&mut reference, argument))
            };
            let (slot, value) = slot.ok_or_else(|| io::Error::other(USAGE))?;
            if value.as_encoded_bytes().starts_with(b"-") {
                return Err(io::Error::other(USAGE));
            }
            if slot.replace(value.clone()).is_some() {
                return Err(io::Error::other(USAGE));
            }
        }
        Ok(Self {
            reference,
            source: source.unwrap_or_else(|| SOURCE.into()),
        })
    }
}

/// Replace the checkout's example tree with the source's, keeping the old tree on any failure.
pub(super) fn run(args: &[OsString], checkout: &Path) -> io::Result<ExitCode> {
    let request = Request::parse(args)?;
    let staging = checkout.join(STAGING);
    remove_directory(&staging)?;
    std::fs::create_dir_all(&staging)?;
    let fetched = fetch(&request, &staging).and_then(|()| install(checkout, &staging));
    let cleaned = remove_directory(&staging);
    fetched?;
    cleaned?;
    Ok(ExitCode::SUCCESS)
}

/// Fetch the revision into `staging` and check out only its example folder.
fn fetch(request: &Request, staging: &Path) -> io::Result<()> {
    git(staging, ["init".as_ref(), "--quiet".as_ref()])?;
    let reference = request.reference.as_deref().unwrap_or(OsStr::new("HEAD"));
    git(
        staging,
        [
            "fetch".as_ref(),
            "--quiet".as_ref(),
            "--depth=1".as_ref(),
            request.source.as_os_str(),
            reference,
        ],
    )?;
    let listed = git(
        staging,
        ["ls-tree".as_ref(), "FETCH_HEAD".as_ref(), TREE.as_ref()],
    )?;
    if listed.stdout.is_empty() {
        return Err(io::Error::other(format!(
            "the source holds no {TREE}/ folder"
        )));
    }
    git(
        staging,
        [
            "checkout".as_ref(),
            "--quiet".as_ref(),
            "FETCH_HEAD".as_ref(),
            "--".as_ref(),
            TREE.as_ref(),
        ],
    )?;
    Ok(())
}

/// Move the fetched tree into place, restoring the previous tree if the move fails.
fn install(checkout: &Path, staging: &Path) -> io::Result<()> {
    let tree = checkout.join(TREE);
    let previous = checkout.join(PREVIOUS);
    remove_directory(&previous)?;
    let replaced = tree.exists();
    if replaced {
        std::fs::rename(&tree, &previous).map_err(|error| at(&tree, error))?;
    }
    if let Err(error) = std::fs::rename(staging.join(TREE), &tree) {
        if replaced {
            std::fs::rename(&previous, &tree)?;
        }
        return Err(at(&tree, error));
    }
    remove_directory(&previous)
}

/// Run `git` in `directory`, returning its output or its own error text.
fn git<const N: usize>(directory: &Path, args: [&OsStr; N]) -> io::Result<std::process::Output> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()?;
    if output.status.success() {
        return Ok(output);
    }
    Err(io::Error::other(format!(
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

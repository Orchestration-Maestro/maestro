//! Where each program preparation runs comes from: this workspace, or its locked source.
//!
//! A workspace that builds on Maestro holds the programs it prepares in its own tree. One that
//! only depends on them reaches the package through a git dependency, whose checkout Cargo owns,
//! so the program is installed from the URL and commit the lock names.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cargo_lock::Lock;
use crate::workspace::Workspace;

/// One program preparation runs: the package that holds it and one target within that package.
pub(super) struct Program<'a> {
    /// The package that holds the program.
    pub(super) package: &'a str,
    /// The Cargo target the program is.
    pub(super) target: Target<'a>,
}

/// The Cargo target a program is.
pub(super) enum Target<'a> {
    /// An example target.
    Example(&'a str),
    /// A binary target.
    Binary(&'a str),
}

impl Target<'_> {
    /// The Cargo flag that selects this kind of target.
    fn flag(&self) -> &'static str {
        match self {
            Self::Example(_) => "--example",
            Self::Binary(_) => "--bin",
        }
    }

    /// The target's name, which is also its executable's name.
    pub(super) fn name(&self) -> &str {
        match self {
            Self::Example(name) | Self::Binary(name) => name,
        }
    }
}

/// Where a program is built from.
#[derive(Debug)]
pub(super) enum Source {
    /// The workspace holds the program's package, so Cargo builds it in the checkout.
    Member,
    /// The workspace depends on the program's package at one locked git revision.
    Locked {
        /// The repository the package comes from.
        url: String,
        /// The exact commit the lock pins.
        rev: String,
    },
}

/// Resolve where `package` is built from, when this workspace has it at all.
///
/// A package the workspace neither holds nor depends on is absent, so the step that needs it
/// is skipped.
///
/// # Errors
/// Returns a locked package without a git source, or a failed metadata read.
pub(super) fn source(
    cargo: &Path,
    checkout: &Path,
    workspace: &Workspace,
    package: &str,
) -> io::Result<Option<Source>> {
    if workspace.package(package).is_some() {
        return Ok(Some(Source::Member));
    }
    // A package the lock does not name cannot be built, so no resolution is needed to skip it.
    if !Lock::read(checkout)?
        .packages()
        .iter()
        .any(|entry| entry.name == package)
    {
        return Ok(None);
    }
    let metadata = crate::locked_dependencies(cargo, checkout)?;
    let Some(found) = metadata.packages.iter().find(|found| found.name == package) else {
        return Ok(None);
    };
    let locked = found
        .source
        .as_ref()
        .ok_or_else(|| io::Error::other(format!("{package} has no locked source")))?;
    locked_source(locked.repr.as_str()).map(Some)
}

/// Split a locked git source into the repository and the exact commit it pins.
///
/// Cargo spells the source of a git dependency `git+<url>?<query>#<commit>`, where the query
/// names the revision the manifest asked for and the fragment is the commit it resolved to.
fn locked_source(source: &str) -> io::Result<Source> {
    let not_git = || io::Error::other(format!("{source} is not a locked git source"));
    let rest = source.strip_prefix("git+").ok_or_else(not_git)?;
    let (rest, rev) = rest.rsplit_once('#').ok_or_else(not_git)?;
    let (url, _) = rest.split_once('?').unwrap_or((rest, ""));
    if url.is_empty() || rev.is_empty() {
        return Err(not_git());
    }
    Ok(Source::Locked {
        url: url.to_owned(),
        rev: rev.to_owned(),
    })
}

/// Install `program` from its locked source into `root` and return the installed executable.
///
/// # Errors
/// Returns a member source, a failed install, or an install that put no executable in `root`.
pub(super) fn install(
    cargo: &Path,
    checkout: &Path,
    source: &Source,
    program: &Program<'_>,
    root: &Path,
) -> io::Result<PathBuf> {
    let Source::Locked { url, rev } = source else {
        return Err(io::Error::other(format!(
            "{} is a workspace crate and cannot be installed from a locked source",
            program.package
        )));
    };
    let status = Command::new(cargo)
        .current_dir(checkout)
        .args(["install", "--locked", "--git", url, "--rev", rev])
        .arg(program.package)
        .arg(program.target.flag())
        .arg(program.target.name())
        .arg("--root")
        .arg(root)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "cannot install {} {} from {url} at {rev}: {status}",
            program.package,
            program.target.name()
        )));
    }
    let installed = root.join("bin").join(super::executable(
        program.target.name(),
        std::env::consts::EXE_EXTENSION,
    ));
    if installed.is_file() {
        return Ok(installed);
    }
    Err(io::Error::other(format!(
        "the install of {} put no executable at {}",
        program.target.name(),
        installed.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A locked git dependency names its repository and the commit the lock resolved.
    #[test]
    fn a_locked_git_source_names_its_repository_and_commit() {
        let Source::Locked { url, rev } =
            locked_source("git+https://github.com/Orchestration-Maestro/maestro?rev=1a2b3c#4d5e6f")
                .unwrap()
        else {
            panic!("a git source is locked");
        };
        assert_eq!(url, "https://github.com/Orchestration-Maestro/maestro");
        assert_eq!(rev, "4d5e6f");
    }

    /// A dependency without a revision query still pins the commit the lock resolved.
    #[test]
    fn a_locked_git_source_without_a_query_pins_the_resolved_commit() {
        let Source::Locked { url, rev } = locked_source("git+file:///srv/maestro#abc123").unwrap()
        else {
            panic!("a git source is locked");
        };
        assert_eq!(url, "file:///srv/maestro");
        assert_eq!(rev, "abc123");
    }

    /// A program the workspace only depends on is installed from the repository and commit the
    /// lock pins, and the installed executable is the one preparation exports.
    #[cfg(unix)]
    #[test]
    fn installation_uses_the_locked_source_and_exports_the_installed_executable() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let root = scratch.path();
        let destination = root.join("installed");
        let log = root.join("argv");
        let cargo = root.join("cargo");
        std::fs::write(
            &cargo,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {log}\nmkdir -p {bin}\ntouch {bin}/maestro\n",
                log = log.display(),
                bin = destination.join("bin").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let source = Source::Locked {
            url: "https://example.invalid/maestro".to_owned(),
            rev: "4d5e6f".to_owned(),
        };
        let program = Program {
            package: "maestro",
            target: Target::Binary("maestro"),
        };

        let executable = install(&cargo, root, &source, &program, &destination).unwrap();

        assert_eq!(executable, destination.join("bin/maestro"));
        let argv = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            argv.lines().collect::<Vec<_>>(),
            [
                "install",
                "--locked",
                "--git",
                "https://example.invalid/maestro",
                "--rev",
                "4d5e6f",
                "maestro",
                "--bin",
                "maestro",
                "--root",
                destination.to_str().unwrap(),
            ]
        );
    }

    /// A workspace crate is built by Cargo, so it cannot be installed from a locked source.
    #[test]
    fn installation_refuses_a_workspace_crate() {
        let program = Program {
            package: "maestro",
            target: Target::Binary("maestro"),
        };
        let error = install(
            Path::new("/missing"),
            Path::new("/missing"),
            &Source::Member,
            &program,
            Path::new("/missing"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("workspace crate"), "{error}");
    }

    /// A registry or path dependency has no git source to install from.
    #[test]
    fn a_source_that_is_not_git_is_refused() {
        for source in [
            "registry+https://github.com/rust-lang/crates.io-index",
            "git+https://github.com/example/maestro",
            "git+file:///srv/maestro#",
        ] {
            let error = locked_source(source).unwrap_err().to_string();
            assert!(error.contains("git source"), "{source}: {error}");
        }
    }
}

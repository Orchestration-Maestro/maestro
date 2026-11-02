//! Typed entry selection and the private executable-copy operation.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod browser_launch;
mod cargo;
#[cfg(unix)]
mod editor;
mod observe;
#[cfg(unix)]
mod rpc;
#[cfg(unix)]
mod search;
#[cfg(unix)]
mod trash;

/// A fully parsed fixture request, before any observable effect.
enum Request {
    /// Copy this artifact to a caller-owned path.
    Install(PathBuf),
    /// Report the native launch and exchange raw streams.
    Observe(observe::Record, Vec<OsString>),
    /// Stand in for the preparation fixture's native Cargo boundary.
    Cargo(cargo::Record, Vec<OsString>),
    /// Announce a desktop viewer launch and wait for its release.
    #[cfg(unix)]
    BrowserLaunch(browser_launch::Record, Vec<OsString>),
    /// Edit a caller-owned file through the recorded effects.
    #[cfg(unix)]
    Editor(editor::Record, Vec<OsString>),
    /// Exchange raw application RPC records.
    #[cfg(unix)]
    Rpc(rpc::Record),
    /// Produce controlled search bytes without interpreting native operands.
    #[cfg(unix)]
    Search(search::Record, Vec<OsString>),
    /// Move or remove caller-owned files on the trash program's behalf.
    #[cfg(unix)]
    Trash(trash::Record, Vec<OsString>),
}

impl Request {
    /// Parse a private operation or the installed alias's sibling record.
    fn read(args: Vec<OsString>) -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        if executable
            .file_stem()
            .is_none_or(|name| name != "native_fixture")
        {
            let mut record = executable.into_os_string();
            record.push(".fixture");
            return Self::record(Path::new(&record), args);
        }
        match args.as_slice() {
            [operation, destination] if operation == "--maestro-install" => {
                Ok(Self::Install(destination.into()))
            }
            [operation, record, operands @ ..] if operation == "--maestro-observe" => {
                Self::record(Path::new(record), operands.to_vec())
            }
            _ => Err(io::Error::other(
                "unknown or incomplete native fixture operation",
            )),
        }
    }

    /// Select a finite family from a complete sibling record.
    fn record(path: &Path, operands: Vec<OsString>) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        match text.lines().next() {
            Some("observe") => Ok(Self::Observe(observe::Record::parse(&text)?, operands)),
            Some("tooling-cargo") => Ok(Self::Cargo(cargo::Record::parse(&text)?, operands)),
            #[cfg(unix)]
            Some("browser_launch") => Ok(Self::BrowserLaunch(
                browser_launch::Record::parse(&text)?,
                operands,
            )),
            #[cfg(unix)]
            Some("editor") => Ok(Self::Editor(editor::Record::parse(&text)?, operands)),
            #[cfg(unix)]
            Some("rpc-peer") => Ok(Self::Rpc(rpc::Record::parse(&text)?)),
            #[cfg(unix)]
            Some("search") => Ok(Self::Search(search::Record::parse(&text)?, operands)),
            #[cfg(unix)]
            Some("trash") => Ok(Self::Trash(trash::Record::parse(&text)?, operands)),
            _ => Err(io::Error::other("unknown native fixture record")),
        }
    }
}

/// Parse before effects, then execute the requested native operation.
pub(super) fn run() -> io::Result<()> {
    match Request::read(std::env::args_os().skip(1).collect())? {
        Request::Install(destination) => install(&destination),
        Request::Observe(record, operands) => observe::run(&record, &operands),
        Request::Cargo(record, operands) => cargo::run(&record, &operands),
        #[cfg(unix)]
        Request::BrowserLaunch(record, operands) => browser_launch::run(&record, &operands),
        #[cfg(unix)]
        Request::Editor(record, operands) => editor::run(&record, &operands),
        #[cfg(unix)]
        Request::Rpc(record) => rpc::run(&record),
        #[cfg(unix)]
        Request::Search(record, operands) => search::run(&record, &operands),
        #[cfg(unix)]
        Request::Trash(record, operands) => trash::run(&record, &operands),
    }
}

/// Copy bytes in this isolated process; `fs::copy` closes its writer before returning.
fn install(destination: &Path) -> io::Result<()> {
    std::fs::copy(std::env::current_exe()?, destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

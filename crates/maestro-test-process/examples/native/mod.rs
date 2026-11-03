//! Typed entry selection and the private executable-copy operation.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

mod author;
mod cargo;
#[cfg(unix)]
mod child;
mod clipboard;
#[cfg(unix)]
mod command;
mod credentials;
mod echo;
mod editor;
#[cfg(unix)]
mod exec;
mod git;
mod notification;
mod observe;
mod rpc;
#[cfg(unix)]
mod search;
#[cfg(unix)]
mod termination;
mod tooling_command;
mod tooling_wrapper;

/// A fully parsed fixture request, before any observable effect.
enum Request {
    /// Copy this artifact to a caller-owned path.
    Install(PathBuf),
    /// Report the native launch and exchange raw streams.
    Observe(observe::Record, Vec<OsString>),
    /// Stand in for a controlled native program.
    #[cfg(unix)]
    Command(command::Record, Vec<OsString>),
    /// Act as the native child a byte-level operation launched.
    #[cfg(unix)]
    Child(child::Record, Vec<OsString>),
    /// Stand in for the preparation fixture's native Cargo boundary.
    Cargo(cargo::Record, Vec<OsString>),
    /// Produce controlled clipboard bytes for the application's native operations.
    Clipboard(clipboard::Record, Vec<OsString>),
    /// Answer native Git queries for footer observations or repository discovery.
    Git(git::Record, Vec<OsString>),
    /// Record notification operands and return a chosen status.
    Notification(notification::Record, Vec<OsString>),
    /// Report the file a controlled editor was asked to edit.
    Editor(editor::Record, Vec<OsString>),
    /// Print one authored line and end.
    Echo(echo::Record),
    /// Run a copied preparation example against its checkout-local declaration.
    Author(author::Record, Vec<OsString>),
    /// Answer as an AWS credential process or a Google executable source.
    Credentials(credentials::Record, Vec<OsString>),
    /// Exchange controlled command payload bytes.
    #[cfg(unix)]
    Exec(exec::Record, Vec<OsString>),
    /// Exchange raw application RPC records.
    Rpc(rpc::Record),
    /// Produce controlled search bytes without interpreting native operands.
    #[cfg(unix)]
    Search(search::Record, Vec<OsString>),
    /// Stand in for a repository command a recipe resolves by name.
    ToolingCommand(tooling_command::Record, Vec<OsString>),
    /// Stand in for a compiler wrapper a fixture build runs.
    ToolingWrapper(tooling_wrapper::Record, Vec<OsString>),
}

impl Request {
    /// Parse a private operation or the installed alias's sibling record.
    fn read(args: Vec<OsString>) -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        // Copied preparation examples receive their record through an explicit caller selector.
        if executable
            .file_stem()
            .is_some_and(|name| name == "prepare_author_component")
            && let Some(record) = std::env::var_os("MAESTRO_NATIVE_RECORD")
        {
            return Self::record(Path::new(&record), args);
        }
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
            #[cfg(unix)]
            Some("command") => Ok(Self::Command(command::Record::parse(&text)?, operands)),
            #[cfg(unix)]
            Some("child") => Ok(Self::Child(child::Record::parse(&text)?, operands)),
            Some("tooling-cargo") => Ok(Self::Cargo(cargo::Record::parse(&text)?, operands)),
            Some("clipboard") => Ok(Self::Clipboard(clipboard::Record::parse(&text)?, operands)),
            Some("git") => Ok(Self::Git(git::Record::parse(&text)?, operands)),
            Some("notify-send") => Ok(Self::Notification(
                notification::Record::parse(&text)?,
                operands,
            )),
            Some("editor") => Ok(Self::Editor(editor::Record::parse(&text)?, operands)),
            Some("echo") => Ok(Self::Echo(echo::Record::parse(&text)?)),
            Some("tooling-author") => Ok(Self::Author(author::Record::parse(&text)?, operands)),
            Some("credentials") => Ok(Self::Credentials(
                credentials::Record::parse(&text)?,
                operands,
            )),
            #[cfg(unix)]
            Some("exec") => Ok(Self::Exec(exec::Record::parse(&text)?, operands)),
            Some("rpc-peer") => Ok(Self::Rpc(rpc::Record::parse(&text)?)),
            #[cfg(unix)]
            Some("search") => Ok(Self::Search(search::Record::parse(&text)?, operands)),
            Some("tooling-command") => Ok(Self::ToolingCommand(
                tooling_command::Record::parse(&text)?,
                operands,
            )),
            Some("tooling-wrapper") => Ok(Self::ToolingWrapper(
                tooling_wrapper::Record::parse(&text)?,
                operands,
            )),
            _ => Err(io::Error::other("unknown native fixture record")),
        }
    }
}

/// Parse before effects, then execute the requested native operation.
pub(super) fn run() -> io::Result<()> {
    match Request::read(std::env::args_os().skip(1).collect())? {
        Request::Install(destination) => install(&destination),
        Request::Observe(record, operands) => observe::run(&record, &operands),
        #[cfg(unix)]
        Request::Command(record, operands) => command::run(&record, &operands),
        #[cfg(unix)]
        Request::Child(record, operands) => child::run(&record, &operands),
        Request::Cargo(record, operands) => cargo::run(&record, &operands),
        Request::Clipboard(record, operands) => clipboard::run(&record, &operands),
        Request::Git(record, operands) => git::run(&record, &operands),
        Request::Notification(record, operands) => notification::run(&record, &operands),
        Request::Editor(record, operands) => editor::run(&record, &operands),
        Request::Echo(record) => echo::run(&record, &[]),
        Request::Author(record, operands) => author::run(&record, &operands),
        Request::Credentials(record, operands) => credentials::run(&record, &operands),
        #[cfg(unix)]
        Request::Exec(record, operands) => exec::run(&record, &operands),
        Request::Rpc(record) => rpc::run(&record),
        #[cfg(unix)]
        Request::Search(record, operands) => search::run(&record, &operands),
        Request::ToolingCommand(record, operands) => tooling_command::run(&record, &operands),
        Request::ToolingWrapper(record, operands) => tooling_wrapper::run(&record, &operands),
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

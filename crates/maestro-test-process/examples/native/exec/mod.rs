//! Controlled command payloads selected by a sibling JSON record.

use std::{ffi::OsString, io};

use serde::Deserialize;

mod payloads;

/// One finite command payload, parsed before any stream, socket or directory changes.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Report the operands, directory and home after requiring closed standard input.
    Operands {},
    /// Write one large standard output and one large standard error stream.
    Streams {},
    /// Split one scalar across the readiness exchange.
    Utf8 {
        /// Parent-owned readiness and release channel.
        port: u16,
        /// Bytes of the scalar written before readiness.
        split: usize,
    },
    /// Write bytes that never form a complete scalar.
    Malformed {},
    /// Report the directory this process inherited.
    Cwd {},
    /// Write the partial streams, announce readiness, then finish after one release byte.
    Holder {
        /// Parent-owned readiness and release channel.
        port: u16,
        /// Bytes written to standard output before readiness.
        #[serde(default)]
        stdout: Vec<u8>,
        /// Bytes written to standard error before readiness.
        #[serde(default)]
        stderr: Vec<u8>,
    },
    #[cfg(unix)]
    /// Write the partial streams, then answer every TERM until killed, released by one byte
    /// or disconnected.
    Stubborn {
        /// Parent-owned readiness and question channel.
        port: u16,
        /// Bytes written to standard output before readiness.
        #[serde(default)]
        stdout: Vec<u8>,
        /// Bytes written to standard error before readiness.
        #[serde(default)]
        stderr: Vec<u8>,
    },
    /// Leave a linked descendant holding both pipes after this process exits.
    Descendant {
        /// Parent-owned readiness and question channel.
        port: u16,
    },
    /// Require two release bytes while writing to each output in turn.
    Exchange {
        /// Parent-owned release channel.
        port: u16,
    },
    /// Write raw bytes before release, then standard error after release.
    Raw {
        /// Parent-owned release channel.
        port: u16,
    },
    /// Interleave a split scalar and standard error before a large raw output.
    Interleaved {
        /// Parent-owned release channel.
        port: u16,
    },
    /// Report this process's PID, then acknowledge two releases before writing a marker.
    Termination {
        /// Parent-owned release channel.
        port: u16,
    },
    #[cfg(unix)]
    /// Report this process's session and process group.
    Identity {},
    /// Answer each question under the default termination action.
    Yielding {
        /// Parent-owned readiness and question channel.
        port: u16,
    },
    /// Wait for a linked descendant, then one control byte or EOF.
    TreeRoot {
        /// Parent-owned control channel.
        port: u16,
    },
    /// Hold one control channel until the owning tree is killed.
    TreeLeaf {
        /// Parent-owned control channel.
        port: u16,
    },
    #[cfg(unix)]
    /// Finish after one termination signal, without polling or a deadline.
    Blocked {
        /// Exit code to report once the termination signal was handled.
        code: Option<i32>,
        /// Optional parent-owned channel announcing that TERM is registered.
        port: Option<u16>,
    },
    /// Write grace bytes into another process's output descriptor.
    Grace {
        /// Process whose standard output the caller observes.
        pid: u32,
    },
}

impl Record {
    /// Reject unknown scenarios, fields and values before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("exec\n")
            .ok_or_else(|| io::Error::other("missing exec family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }

    /// The parent channel a linked descendant keeps using.
    fn port(&self) -> io::Result<u16> {
        match self {
            Self::Descendant { port } | Self::TreeRoot { port } => Ok(*port),
            _ => Err(io::Error::other("role without a parent channel")),
        }
    }
}

/// The role this process was started as on behalf of another fixture process.
#[derive(Clone, Copy)]
enum Role {
    /// Answer the questions of a direct child that already exited.
    Descendant,
    /// Hold a control channel beside the process that started this one.
    TreeLeaf,
}

impl Role {
    /// Read the role from the environment, rejecting an unknown name.
    fn read() -> io::Result<Option<Self>> {
        match std::env::var("MTP_ROLE") {
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(io::Error::other(error)),
            Ok(role) => match role.as_str() {
                "descendant" => Ok(Some(Self::Descendant)),
                "tree-leaf" => Ok(Some(Self::TreeLeaf)),
                _ => Err(io::Error::other("unknown fixture role")),
            },
        }
    }

    /// The environment value that selects this role.
    fn name(self) -> &'static str {
        match self {
            Self::Descendant => "descendant",
            Self::TreeLeaf => "tree-leaf",
        }
    }
}

/// Start this fixture again as `role` in the current process group.
fn spawn(role: Role) -> io::Result<std::process::Child> {
    std::process::Command::new(std::env::current_exe()?)
        .env("MTP_ROLE", role.name())
        .spawn()
}

/// Parse the requested role or scenario, then run exactly that payload.
///
/// # Errors
///
/// Returns an unknown or malformed record, a failed link, stream or socket operation,
/// or a descendant that could not start.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    if let Some(role) = Role::read()? {
        let port = record.port()?;
        return match role {
            Role::Descendant => payloads::descendant_holder(port),
            Role::TreeLeaf => payloads::tree_leaf(port),
        };
    }
    match record {
        Record::Operands {} => payloads::operands(operands),
        Record::Streams {} => payloads::streams(),
        Record::Utf8 { port, split } => payloads::utf8(*port, *split),
        Record::Malformed {} => payloads::malformed(),
        Record::Cwd {} => payloads::cwd(),
        Record::Holder {
            port,
            stdout,
            stderr,
        } => payloads::holder(*port, stdout, stderr),
        #[cfg(unix)]
        Record::Stubborn {
            port,
            stdout,
            stderr,
        } => payloads::stubborn(*port, stdout, stderr),
        Record::Descendant { .. } => payloads::descendant(),
        Record::Exchange { port } => payloads::exchange(*port),
        Record::Raw { port } => payloads::raw(*port),
        Record::Interleaved { port } => payloads::interleaved(*port),
        Record::Termination { port } => payloads::termination(*port),
        #[cfg(unix)]
        Record::Identity {} => payloads::identity(),
        Record::Yielding { port } => payloads::yielding(*port),
        Record::TreeRoot { port } => payloads::tree_root(*port),
        Record::TreeLeaf { port } => payloads::tree_leaf(*port),
        #[cfg(unix)]
        Record::Blocked { code, port } => payloads::blocked(*code, *port),
        Record::Grace { pid } => payloads::grace(*pid),
    }
}

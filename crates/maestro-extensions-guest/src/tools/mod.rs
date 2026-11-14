//! Tool argument and observation records.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod bash;
pub(crate) mod file_mutation_queue;
pub use bash::*;
mod read;
pub use read::*;
mod edit;
pub use edit::*;
mod write;
pub use write::*;
mod grep;
pub use grep::*;
mod find;
pub use find::*;
mod ls;
pub use ls::*;
pub mod limits;
pub use limits::*;
mod options;
pub use options::*;

/// Native authored-path operations.
pub mod path_utils;

/// A definition returned by a built-in factory.
pub type ToolDef = crate::ToolDefinition;

/// Built-in tool identities; custom registered names remain ordinary strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolName {
    /// Native read identity.
    Read,
    /// Native shell identity.
    Bash,
    /// Native edit identity.
    Edit,
    /// Native write identity.
    Write,
    /// Native content-search identity.
    Grep,
    /// Native file-search identity.
    Find,
    /// Native listing identity.
    Ls,
}

impl From<crate::bindings::maestro::extension::host::ToolName> for ToolName {
    fn from(name: crate::bindings::maestro::extension::host::ToolName) -> Self {
        use crate::bindings::maestro::extension::host::ToolName as Native;
        match name {
            Native::Read => Self::Read,
            Native::Bash => Self::Bash,
            Native::Edit => Self::Edit,
            Native::Write => Self::Write,
            Native::Grep => Self::Grep,
            Native::Find => Self::Find,
            Native::Ls => Self::Ls,
        }
    }
}

/// All built-in identities in native factory insertion order.
pub const ALL_TOOL_NAMES: [ToolName; 7] = [
    ToolName::Read,
    ToolName::Bash,
    ToolName::Edit,
    ToolName::Write,
    ToolName::Grep,
    ToolName::Find,
    ToolName::Ls,
];
